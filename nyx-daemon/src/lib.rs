#![forbid(unsafe_code)]

// Public module_s for daemon runtime (pure Rust only; no ring/openssl).
pub mod config_manager;
pub mod event_system;
#[cfg(feature = "low_power")]
pub mod low_power;
pub mod metrics;
pub mod prometheus_exporter;

// Re-export with shorter prefixe_s used in main.r_s
pub use config_manager as nyx_daemon_config;
pub use event_system as nyx_daemon_event_s;
#[cfg(feature = "low_power")]
pub use low_power as nyx_daemon_low_power;
