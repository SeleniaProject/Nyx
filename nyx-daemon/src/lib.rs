
#![forbid(unsafe_code)]

// Public modules for daemon runtime; kept minimal and pure Rust.
pub mod config_manager;
pub mod event_system;

// Re-export with shorter prefixes used in main.rs
pub use config_manager as nyx_daemon_config;
pub use event_system as nyx_daemon_events;

