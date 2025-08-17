#![forbid(unsafe_code)]

use std::{path::PathBuf, sync::Arc, time::SystemTime};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::{fs, sync::RwLock};
use tracing::{debug, info, warn};
use nyx_stream::FrameCodec;

/// Static configuration structure loaded from TOML.
/// - Start with a minimal set of fields and extend progressively
/// - Ensure forward-compatibility: unknown fields are ignored via serde defaults
/// - Combine with `DynamicConfig` to apply runtime overrides
///   Extend this as the daemon grows; unknown fields are ignored via serde defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NyxConfig {
    /// Daemon listen port for transport (kept for compatibility; not used by IPC).
    #[serde(default)]
    pub listen_port: u16,
    /// Tracing level (e.g., "info", "debug").
    #[serde(default)]
    pub log_level: Option<String>,
    /// Optional hex-encoded 32-byte node id; generated when absent.
    #[serde(default)]
    pub node_id: Option<String>,
    /// Optional static max frame length (bytes) applied on reload/startup
    #[serde(default)]
    pub max_frame_len_bytes: Option<u64>,
}

/// Dynamic settings that can be changed at runtime via IPC.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DynamicConfig {
    #[serde(default)]
    pub log_level: Option<String>,
    #[serde(default)]
    pub metrics_interval_secs: Option<u64>,
    /// Optional max frame length in bytes for codec safety cap (applies process-wide via env)
    #[serde(default)]
    pub max_frame_len_bytes: Option<u64>,
}

/// Single snapshot of configuration for rudimentary versioning and rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigVersion {
    pub version: u64,
    pub config: NyxConfig,
    pub dynamic: DynamicConfig,
    pub timestamp: SystemTime,
    pub description: String,
}

/// Public summary view of stored versions (no full config payloads).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSummary {
    pub version: u64,
    pub timestamp: SystemTime,
    pub description: String,
}

/// Response type returned by update/reload operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub validation_errors: Vec<String>,
}

/// Manager that owns configuration state and provides validation and file reload.
#[derive(Clone)]
pub struct ConfigManager {
    config: Arc<RwLock<NyxConfig>>,        // current static config
    dynamic: Arc<RwLock<DynamicConfig>>,   // current dynamic overrides
    config_path: Option<PathBuf>,          // optional path for reload
    // simple in-memory versioning (ring buffer semantics not needed yet)
    versions: Arc<RwLock<Vec<ConfigVersion>>>,
    current_version: Arc<RwLock<u64>>,     // monotonically increasing
    max_versions: usize,
}

impl ConfigManager {
    /// Create new manager with an initial config and optional file path.
    pub fn new(initial: NyxConfig, config_path: Option<PathBuf>) -> Self {
        Self {
            config: Arc::new(RwLock::new(initial)),
            dynamic: Arc::new(RwLock::new(DynamicConfig::default())),
            config_path,
            versions: Arc::new(RwLock::new(Vec::with_capacity(16))),
            current_version: Arc::new(RwLock::new(0)),
            max_versions: 16,
        }
    }

    /// Get merged view: static + dynamic (dynamic overlays are applied by the caller when relevant).
    pub async fn get_config(&self) -> NyxConfig { self.config.read().await.clone() }
    pub async fn get_dynamic(&self) -> DynamicConfig { self.dynamic.read().await.clone() }

    /// Update dynamic settings atomically; returns detailed validation errors when any.
    pub async fn update_config(&self, updates: serde_json::Map<String, serde_json::Value>) -> Result<ConfigResponse> {
        let mut dyn_cfg = self.dynamic.write().await;
        let mut errors = Vec::new();
        let mut changed = Vec::new();

        for (k, v) in updates.into_iter() {
            match k.as_str() {
                "log_level" => {
                    if let Some(level) = v.as_str() {
                        if matches!(level, "trace"|"debug"|"info"|"warn"|"error") {
                            dyn_cfg.log_level = Some(level.to_string());
                            // Apply immediately for operator feedback
                            std::env::set_var("RUST_LOG", level);
                            tracing_subscriber::fmt::try_init().ok();
                            changed.push(k);
                        } else {
                            errors.push(format!("invalid log_level: {level}"));
                        }
                    } else {
                        errors.push("log_level must be string".to_string());
                    }
                }
                "metrics_interval_secs" => {
                    match v.as_u64() {
                        Some(secs) if (1..=3600).contains(&secs) => {
                            dyn_cfg.metrics_interval_secs = Some(secs);
                            changed.push(k);
                        }
                        _ => errors.push("metrics_interval_secs must be 1..=3600".into()),
                    }
                }
                "max_frame_len_bytes" => {
                    match v.as_u64() {
                        Some(n) if (1024..=64 * 1024 * 1024).contains(&n) => {
                            dyn_cfg.max_frame_len_bytes = Some(n);
                            // Apply immediately via API and also set env for child processes if any
                            FrameCodec::set_default_limit(n as usize);
                            std::env::set_var("NYX_FRAME_MAX_LEN", n.to_string());
                            changed.push(k);
                        }
                        _ => errors.push("max_frame_len_bytes must be 1024..=67108864".into()),
                    }
                }
                other => {
                    errors.push(format!("unknown setting: {other}"));
                }
            }
        }

        if errors.is_empty() {
            info!("dynamic config updated: {:?}", changed);
            Ok(ConfigResponse { success: true, message: format!("updated {} field(s)", changed.len()), validation_errors: vec![] })
        } else {
            warn!("dynamic config update failed: {:?}", errors);
            Ok(ConfigResponse { success: false, message: "validation failed".into(), validation_errors: errors })
        }
    }

    /// Validate basic constraints for static config. Extend this progressively.
    pub fn validate_static(config: &NyxConfig) -> Vec<String> {
        let mut errs = Vec::new();
        // Allow 0 (unspecified) or 1024..=65535; privileged ports are disallowed by default.
        if config.listen_port != 0 && !(1024..=65535).contains(&config.listen_port) {
            errs.push("listen_port must be 0 or within 1024..=65535".into());
        }
        if let Some(id) = &config.node_id {
            if !hex::decode(id).map(|b| b.len() == 32).unwrap_or(false) {
                errs.push("node_id must be 32-byte hex".into());
            }
        }
        errs
    }

    /// Reload from file when `config_path` is set.
    pub async fn reload_from_file(&self) -> Result<ConfigResponse> {
        let path = match &self.config_path { Some(p) => p.clone(), None => return Ok(ConfigResponse { success: false, message: "no config_path set".into(), validation_errors: vec![] }) };
        let content = fs::read_to_string(&path).await.context("reading config file")?;
        let parsed: NyxConfig = toml::from_str(&content).context("parsing TOML")?;

        let errs = Self::validate_static(&parsed);
        if !errs.is_empty() {
            return Ok(ConfigResponse { success: false, message: "validation failed".into(), validation_errors: errs });
        }

        // version snapshot before apply
        self.snapshot("reload_from_file").await?;
        *self.config.write().await = parsed.clone();
        // Apply static settings with side effects
        if let Some(n) = parsed.max_frame_len_bytes {
            FrameCodec::set_default_limit(n as usize);
            std::env::set_var("NYX_FRAME_MAX_LEN", n.to_string());
        }
        info!("config reloaded from {:?}", path);
        Ok(ConfigResponse { success: true, message: "reloaded".into(), validation_errors: vec![] })
    }

    /// Store a copy into the in-memory versions vector.
    pub async fn snapshot(&self, description: &str) -> Result<u64> {
        let cfg = self.config.read().await.clone();
        let dyn_cfg = self.dynamic.read().await.clone();
        let mut ver = self.current_version.write().await;
        *ver += 1;
        let version = *ver;

        let snap = ConfigVersion {
            version,
            config: cfg,
            dynamic: dyn_cfg,
            timestamp: SystemTime::now(),
            description: description.to_string(),
        };
        let mut list = self.versions.write().await;
        list.push(snap);
        if list.len() > self.max_versions { list.remove(0); }
        debug!("created config snapshot v{}", version);
        Ok(version)
    }

    /// Attempt rollback to a previous snapshot.
    pub async fn rollback(&self, version: u64) -> Result<ConfigResponse> {
        let snap = {
            let list = self.versions.read().await;
            list.iter().find(|v| v.version == version).cloned()
        };
        match snap {
            Some(s) => {
                *self.config.write().await = s.config;
                *self.dynamic.write().await = s.dynamic;
                *self.current_version.write().await = s.version;
                info!("rolled back to version {}", version);
                Ok(ConfigResponse { success: true, message: format!("rolled back to {version}"), validation_errors: vec![] })
            }
            None => Err(anyhow!("version {} not found", version)),
        }
    }

    /// List summaries of stored configuration versions (most recent last).
    pub async fn list_versions(&self) -> Vec<VersionSummary> {
        let list = self.versions.read().await;
        list.iter()
            .map(|v| VersionSummary { version: v.version, timestamp: v.timestamp, description: v.description.clone() })
            .collect()
    }
}
