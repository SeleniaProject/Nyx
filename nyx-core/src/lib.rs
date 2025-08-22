
#![forbid(unsafe_code)]

//! Core utilitie_s for Nyx.
//!
//! Thi_s crate intentionally stay_s lightweight and pure Rust.
//! Public module_s export typed ID_s, basic error handling,
//! and configuration helper_s used acros_s the workspace.
//!
//! Design goal_s:
//! - Small, dependency-minimized surface
//! - Clear, documented type_s with safe helper_s
//! - Robust config loading with validation

pub mod types;
pub mod error;
pub mod config;
pub mod i18n;
pub mod performance;
pub mod ffi_detector;
pub mod compliance;
pub mod low_power;
pub mod path_monitor;
pub mod multipath_dataplane;
pub mod push;
pub mod push_gateway;
pub mod sandbox;

// Optional subsystem_s
#[cfg(feature = "plugin_framework")]
pub mod plugin_framework;

#[cfg(feature = "zero_copy")]
pub mod zero_copy;

// Re-export commonly used types
pub use types::{StreamId, ConnectionId, Nonce, TimestampMs};
pub use error::{Error, Result};

