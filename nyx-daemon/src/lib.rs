#![deny(unsafe_code)]
#![cfg_attr(feature = "low_power", allow(unsafe_code))]

// Public module_s for daemon runtime (pure Rust only; no ring/openssl).
pub mod config_manager;
pub mod errors; // Error types for daemon
pub mod event_system;
#[cfg(feature = "low_power")]
pub mod low_power;
pub mod metrics;
pub mod path_builder; // Path builder implementation
pub mod path_performance_test; // Performance testing for paths
pub mod path_recovery; // Path recovery and diagnostics
pub mod prometheus_exporter;
pub mod session_manager; // Session and handshake orchestration
pub mod session_api; // REST API for session management
pub mod connection_manager; // Connection-level state (congestion control, RTT, rate limiting)
pub mod connection_api; // REST API for connection management
pub mod stream_manager; // Stream multiplexing and management
pub mod multipath_integration; // Multipath scheduling integration
pub mod packet_processor; // Extended packet format processing
pub mod cmix_integration; // cMix batch processing integration
pub mod larmix_feedback; // LARMix++ feedback loop for dynamic hop adjustment

// Re-export with shorter prefixe_s used in main.r_s
pub use config_manager as nyx_daemon_config;
pub use event_system as nyx_daemon_event_s;
#[cfg(feature = "low_power")]
pub use low_power as nyx_daemon_low_power;
