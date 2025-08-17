
#![forbid(unsafe_code)]

// Public modules for daemon runtime (pure Rust only; no ring/openssl).
pub mod config_manager;
pub mod event_system;
pub mod metrics;
pub mod prometheus_exporter;

// Re-export with shorter prefixes used in main.rs
pub use config_manager as nyx_daemon_config;
pub use event_system as nyx_daemon_events;

