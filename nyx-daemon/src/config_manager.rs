#![forbid(unsafe_code)]

use std::{path::PathBuf, sync::Arc, time::SystemTime};

use anyhow::{anyhow, Context, Result};
use nyx_stream::FrameCodec;
use serde::{Deserialize, Serialize};
use tokio::{fs, sync::RwLock};
use tracing::{debug, info, warn};

/// Static configuration structure loaded from TOML.
/// - Start with a minimal set of field_s and extend progressively
/// - Ensure forward-compatibility: unknown field_s are ignored via serde default_s
/// - Combine with `DynamicConfig` to apply runtime override_s
///   Extend thi_s as the daemon grow_s; unknown field_s are ignored via serde default_s.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct NyxConfig {
    /// Daemon listen port for transport (kept for compatibility; not used by IPC).
    #[serde(default)]
    pub listen_port: u16,
    /// Tracing level (e.g., "info", "debug").
    #[serde(default)]
    pub ___log_level: Option<String>,
    /// Optional hex-encoded 32-byte node id; generated when absent.
    #[serde(default)]
    pub node_id: Option<String>,
    /// Optional static max frame length (byte_s) applied on reload/startup
    #[serde(default)]
    pub max_frame_len_byte_s: Option<u64>,
}

/// Dynamic setting_s that can be changed at runtime via IPC.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct DynamicConfig {
    #[serde(default)]
    pub ___log_level: Option<String>,
    #[serde(default)]
    pub metrics_interval_sec_s: Option<u64>,
    /// Optional max frame length in byte_s for codec safety cap (applie_s proces_s-wide via env)
    #[serde(default)]
    pub max_frame_len_byte_s: Option<u64>,
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

/// Public summary view of stored version_s (no full config payload_s).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSummary {
    pub version: u64,
    pub timestamp: SystemTime,
    pub description: String,
}

/// Response type returned by update/reload operation_s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub __succes_s: bool,
    pub _message: String,
    #[serde(default)]
    pub __validation_error_s: Vec<String>,
}

/// Manager that own_s configuration state and provide_s validation and file reload.
#[derive(Clone)]
pub struct ConfigManager {
    config: Arc<RwLock<NyxConfig>>,      // current static config
    dynamic: Arc<RwLock<DynamicConfig>>, // current dynamic override_s
    configpath: Option<PathBuf>,         // optional path for reload
    // simple in-memory versioning (ring buffer semantic_s not needed yet)
    version_s: Arc<RwLock<Vec<ConfigVersion>>>,
    currentversion: Arc<RwLock<u64>>, // monotonically increasing
    maxversion_s: usize,
}

impl ConfigManager {
    /// Create new manager with an initial config and optional file path.
    pub fn new(initial: NyxConfig, configpath: Option<PathBuf>) -> Self {
        Self {
            config: Arc::new(RwLock::new(initial)),
            dynamic: Arc::new(RwLock::new(DynamicConfig::default())),
            configpath,
            version_s: Arc::new(RwLock::new(Vec::with_capacity(16))),
            currentversion: Arc::new(RwLock::new(0)),
            maxversion_s: 16,
        }
    }

    /// Get merged view: static + dynamic (dynamic overlay_s are applied by the caller when relevant).
    pub async fn getconfig(&self) -> NyxConfig {
        self.config.read().await.clone()
    }
    pub async fn getdynamic(&self) -> DynamicConfig {
        self.dynamic.read().await.clone()
    }

    /// Update dynamic setting_s atomically; return_s detailed validation error_s when any.
    pub async fn updateconfig(
        &self,
        update_s: serde_json::Map<String, serde_json::Value>,
    ) -> Result<ConfigResponse> {
        let mut dyncfg = self.dynamic.write().await;
        let mut error_s = Vec::new();
        let mut changed = Vec::new();

        for (k, v) in update_s.into_iter() {
            match k.as_str() {
                "___log_level" => {
                    if let Some(level) = v.as_str() {
                        if matches!(level, "trace" | "debug" | "info" | "warn" | "error") {
                            dyncfg.___log_level = Some(level.to_string());
                            // Apply immediately for operator feedback
                            std::env::set_var("RUST_LOG", level);
                            tracing_subscriber::fmt::try_init().ok();
                            changed.push(k);
                        } else {
                            error_s.push(format!("invalid ___log_level: {level}"));
                        }
                    } else {
                        error_s.push("___log_level must be string".to_string());
                    }
                }
                "metrics_interval_sec_s" => match v.as_u64() {
                    Some(sec_s) if (1..=3600).contains(&sec_s) => {
                        dyncfg.metrics_interval_sec_s = Some(sec_s);
                        changed.push(k);
                    }
                    _ => error_s.push("metrics_interval_sec_s must be 1..=3600".into()),
                },
                "max_frame_len_byte_s" => {
                    match v.as_u64() {
                        Some(n) if (1024..=64 * 1024 * 1024).contains(&n) => {
                            dyncfg.max_frame_len_byte_s = Some(n);
                            // Apply immediately via API and also set env for child processe_s if any
                            FrameCodec::set_default_limit(n as usize);
                            std::env::set_var("NYX_FRAME_MAX_LEN", n.to_string());
                            changed.push(k);
                        }
                        _ => error_s.push("max_frame_len_byte_s must be 1024..=67108864".into()),
                    }
                }
                other => {
                    error_s.push(format!("unknown setting: {other}"));
                }
            }
        }

        if error_s.is_empty() {
            info!("dynamic config updated: {:?}", changed);
            Ok(ConfigResponse {
                __succes_s: true,
                _message: format!("updated {} field(_s)", changed.len()),
                __validation_error_s: vec![],
            })
        } else {
            warn!("dynamic config update failed: {:?}", error_s);
            Ok(ConfigResponse {
                __succes_s: false,
                _message: "validation failed".into(),
                __validation_error_s: error_s,
            })
        }
    }

    /// Validate basic constraint_s for static config. Extend thi_s progressively.
    pub fn validate_static(config: &NyxConfig) -> Vec<String> {
        let mut err_s = Vec::new();
        // Allow 0 (unspecified) or 1024..=65535; privileged port_s are dis_allowed by default.
        if config.listen_port != 0 && !(1024..=65535).contains(&config.listen_port) {
            err_s.push("listen_port must be 0 or within 1024..=65535".into());
        }
        if let Some(id) = &config.node_id {
            if !hex::decode(id).map(|b| b.len() == 32).unwrap_or(false) {
                err_s.push("node_id must be 32-byte hex".into());
            }
        }
        err_s
    }

    /// Reload from file when `configpath` i_s set.
    pub async fn reload_from_file(&self) -> Result<ConfigResponse> {
        let path = match &self.configpath {
            Some(p) => p.clone(),
            None => {
                return Ok(ConfigResponse {
                    __succes_s: false,
                    _message: "no configpath set".into(),
                    __validation_error_s: vec![],
                })
            }
        };
        let content = fs::read_to_string(&path)
            .await
            .context("reading config file")?;
        let parsed: NyxConfig = toml::from_str(&content).context("parsing TOML")?;

        let err_s = Self::validate_static(&parsed);
        if !err_s.is_empty() {
            return Ok(ConfigResponse {
                __succes_s: false,
                _message: "validation failed".into(),
                __validation_error_s: err_s,
            });
        }

        // version snapshot before apply
        self.snapshot("reload_from_file").await?;
        *self.config.write().await = parsed.clone();
        // Apply static setting_s with side effect_s
        if let Some(n) = parsed.max_frame_len_byte_s {
            FrameCodec::set_default_limit(n as usize);
            std::env::set_var("NYX_FRAME_MAX_LEN", n.to_string());
        }
        info!("config reloaded from {:?}", path);
        Ok(ConfigResponse {
            __succes_s: true,
            _message: "reloaded".into(),
            __validation_error_s: vec![],
        })
    }

    /// Store a copy into the in-memory version_s vector.
    pub async fn snapshot(&self, description: &str) -> Result<u64> {
        let cfg = self.config.read().await.clone();
        let dyncfg = self.dynamic.read().await.clone();
        let mut ver = self.currentversion.write().await;
        *ver += 1;
        let version = *ver;

        let snap = ConfigVersion {
            version,
            config: cfg,
            dynamic: dyncfg,
            timestamp: SystemTime::now(),
            description: description.to_string(),
        };
        let mut list = self.version_s.write().await;
        list.push(snap);
        if list.len() > self.maxversion_s {
            list.remove(0);
        }
        debug!("created config snapshot v{}", version);
        Ok(version)
    }

    /// Attempt rollback to a previou_s snapshot.
    pub async fn rollback(&self, version: u64) -> Result<ConfigResponse> {
        let snap = {
            let list = self.version_s.read().await;
            list.iter().find(|v| v.version == version).cloned()
        };
        match snap {
            Some(_s) => {
                *self.config.write().await = _s.config;
                *self.dynamic.write().await = _s.dynamic;
                *self.currentversion.write().await = _s.version;
                info!("rolled back to version {}", version);
                Ok(ConfigResponse {
                    __succes_s: true,
                    _message: format!("rolled back to {version}"),
                    __validation_error_s: vec![],
                })
            }
            None => Err(anyhow!("version {} not found", version)),
        }
    }

    /// List summarie_s of stored configuration version_s (most recent last).
    pub async fn listversion_s(&self) -> Vec<VersionSummary> {
        let list = self.version_s.read().await;
        list.iter()
            .map(|v| VersionSummary {
                version: v.version,
                timestamp: v.timestamp,
                description: v.description.clone(),
            })
            .collect()
    }
}
