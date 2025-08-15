#![forbid(unsafe_code)]

//! Nyx configuration handling. Parses a TOML file into a strongly-typed structure and supports
//! hot-reloading via the `notify` crate. All public APIs are `async`-ready but do not impose an
//! async runtime themselves.

use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher,
};
use serde::Deserialize;
use std::{fs, path::Path, sync::Arc};
use tokio::sync::watch;

use crate::types::{MAX_HOPS, MIN_HOPS};
use crate::NyxError;

/// Push notification provider configuration used for Low Power Mode wake-up.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PushProvider {
    /// Firebase Cloud Messaging – uses legacy server key authentication.
    Fcm { server_key: String },
    /// Apple Push Notification Service – uses JWT authentication token.
    Apns {
        team_id: String,
        key_id: String,
        /// Raw contents of the `.p8` private key (BEGIN PRIVATE KEY ...).
        key_p8: String,
    },
}

/// Multipath data plane configuration for Nyx Protocol v1.0
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MultipathConfig {
    /// Enable multipath data plane
    pub enabled: bool,

    /// Maximum number of concurrent paths (1-8)
    pub max_paths: usize,

    /// Minimum number of hops for dynamic routing (3-7)  
    pub min_hops: u8,

    /// Maximum number of hops for dynamic routing (3-7)
    pub max_hops: u8,

    /// Reordering buffer timeout in milliseconds
    pub reorder_timeout_ms: u32,

    /// Weight calculation method for round-robin scheduling
    pub weight_method: WeightMethod,
    /// Adaptive reorder buffer: target p95 delay factor vs RTT (e.g. 1.5 means target p95 <= 1.5*RTT)
    pub reorder_target_p95_factor: f64,
    /// Adaptive reorder PID proportional gain
    pub reorder_pid_kp: f64,
    /// Adaptive reorder PID integral gain
    pub reorder_pid_ki: f64,
    /// Adaptive reorder PID derivative gain
    pub reorder_pid_kd: f64,
    /// Minimum reorder buffer size for adaptation
    pub reorder_min_size: usize,
    /// Maximum reorder buffer size for adaptation
    pub reorder_max_size: usize,
    /// Fairness entropy floor (below this triggers weight smoothing boost)
    pub fairness_entropy_floor: f64,
}

/// Method for calculating path weights in multipath round-robin scheduling
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeightMethod {
    /// Weight = 1 / RTT (inverse RTT weighting)
    InverseRtt,
    /// Equal weight for all paths
    Equal,
    /// Custom weight values
    Custom(Vec<u8>),
}

/// Mix routing mode configuration for Nyx Protocol v1.0
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MixConfig {
    /// Mix routing mode: standard, cmix (default: standard)
    pub mode: MixMode,

    /// Batch size for cMix mode (default: 100)
    pub batch_size: usize,

    /// VDF delay in milliseconds for cMix mode (default: 100)
    pub vdf_delay_ms: u64,

    /// Cover traffic generation rate (packets per second)
    pub cover_traffic_rate: f64,

    /// Adaptive cover traffic (adjust based on utilization)
    pub adaptive_cover: bool,

    /// Target utilization for adaptive cover traffic (0.2-0.6)
    pub target_utilization: f64,
}

/// Mix routing modes supported by Nyx Protocol v1.0
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MixMode {
    /// Standard mix routing with fixed delay
    Standard,
    /// cMix with verifiable delay function (VDF)
    Cmix,
}

impl Default for MixConfig {
    fn default() -> Self {
        // Allow env overrides to enforce spec defaults or runtime tuning
        let batch = std::env::var("NYX_CMIX_BATCH")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100);
        let vdf_ms = std::env::var("NYX_CMIX_VDF_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(100);
        Self {
            mode: MixMode::Standard,
            batch_size: batch,
            vdf_delay_ms: vdf_ms,
            cover_traffic_rate: 10.0,
            adaptive_cover: true,
            target_utilization: 0.4,
        }
    }
}

impl Default for MultipathConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_paths: 4,
            min_hops: MIN_HOPS,
            max_hops: MAX_HOPS,
            reorder_timeout_ms: 100, // RTT diff + jitter * 2
            weight_method: WeightMethod::InverseRtt,
            reorder_target_p95_factor: 1.5,
            reorder_pid_kp: 0.4,
            reorder_pid_ki: 0.05,
            reorder_pid_kd: 0.1,
            reorder_min_size: 32,
            reorder_max_size: 4096,
            fairness_entropy_floor: 0.7,
        }
    }
}

impl MultipathConfig {
    /// Get health check interval as Duration (default 5 seconds)
    pub fn health_check_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }

    /// Get hop adjustment interval as Duration (default 30 seconds)
    pub fn hop_adjustment_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(30)
    }

    /// Get reorder timeout as Duration
    pub fn reorder_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.reorder_timeout_ms as u64)
    }

    /// Check if dynamic hop count is enabled (default true)
    pub fn dynamic_hop_count(&self) -> bool {
        true
    }

    /// Calculate weight based on RTT using the configured method
    pub fn calculate_weight(&self, rtt: std::time::Duration) -> u32 {
        match self.weight_method {
            WeightMethod::InverseRtt => {
                let rtt_ms = rtt.as_millis() as f64;
                if rtt_ms > 0.0 {
                    (1000.0 / rtt_ms) as u32
                } else {
                    1000 // Very high weight for very low RTT
                }
            }
            WeightMethod::Equal => 10, // Equal weight for all paths
            WeightMethod::Custom(ref weights) => weights.first().copied().unwrap_or(10) as u32,
        }
    }
}

/// Primary configuration structure shared across Nyx components.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NyxConfig {
    /// Optional node identifier. If omitted a random value will be generated at startup.
    pub node_id: Option<String>,

    /// Logging verbosity (`error`, `warn`, `info`, `debug`, `trace`).
    pub log_level: Option<String>,

    /// UDP listen port for incoming Nyx traffic.
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,

    /// Optional push notification provider (FCM / APNS). When `None`, push support is disabled.
    pub push: Option<PushProvider>,

    /// Multipath configuration
    pub multipath: MultipathConfig,

    /// Mix routing configuration
    pub mix: MixConfig,
}

impl Default for NyxConfig {
    fn default() -> Self {
        Self {
            node_id: None,
            log_level: Some("info".to_string()),
            listen_port: default_listen_port(),
            push: None,
            multipath: MultipathConfig::default(),
            mix: MixConfig::default(),
        }
    }
}

fn default_listen_port() -> u16 {
    43300
}

impl NyxConfig {
    /// Load a configuration file from the given path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> crate::NyxResult<Self> {
        let data = fs::read_to_string(&path).map_err(NyxError::from)?;
        let cfg = toml::from_str::<NyxConfig>(&data).map_err(NyxError::ConfigParse)?;
        Ok(cfg)
    }

    /// Load config alias version
    pub fn load<P: AsRef<Path>>(path: P) -> crate::NyxResult<Self> {
        Self::from_file(path)
    }

    /// Watch the configuration file for changes and receive updates through a watch channel.
    ///
    /// Returns the initial configuration and a [`watch::Receiver`] that yields a new [`NyxConfig`]
    /// wrapped in [`Arc`] every time the file is modified on disk.
    pub fn watch_file<P: AsRef<Path>>(
        path: P,
    ) -> crate::NyxResult<(Arc<NyxConfig>, watch::Receiver<Arc<NyxConfig>>)> {
        let path_buf = path.as_ref().to_path_buf();
        let initial_cfg = Arc::new(Self::from_file(&path_buf)?);
        // Clone for closure capture to avoid moving the original `path_buf`.
        let path_in_closure = path_buf.clone();
        let (tx, rx) = watch::channel::<Arc<NyxConfig>>(initial_cfg.clone());

        // `notify` requires the watcher to stay alive for as long as we want events. We therefore
        // spawn it on a background task and intentionally leak it so that it lives for the process
        // lifetime. This avoids polluting the public API with a guard type the caller must hold.
        let mut watcher: RecommendedWatcher =
            notify::recommended_watcher(move |res: NotifyResult<Event>| {
                if let Ok(event) = res {
                    // Interested only in content modifications.
                    if matches!(event.kind, EventKind::Modify(_)) {
                        if let Ok(updated) = Self::from_file(&path_in_closure) {
                            let _ = tx.send(Arc::new(updated));
                        }
                    }
                }
            })?;

        watcher.watch(&path_buf, RecursiveMode::NonRecursive)?;
        // Leak the watcher so it keeps running. Safe because it lives for the entire program.
        std::mem::forget(watcher);

        Ok((initial_cfg, rx))
    }
}
