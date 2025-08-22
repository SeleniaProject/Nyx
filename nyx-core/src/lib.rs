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

pub mod compliance;
pub mod config;
pub mod error;
pub mod ffi_detector;
pub mod i18n;
pub mod low_power;
pub mod multipath_dataplane;
pub mod path_monitor;
pub mod performance;
pub mod push;
pub mod push_gateway;
pub mod sandbox;
pub mod types;

// Optional subsystem_s
#[cfg(feature = "plugin_framework")]
pub mod plugin_framework;

#[cfg(feature = "zero_copy")]
pub mod zero_copy;

// Re-export commonly used types
pub use error::{Error, Result};
pub use types::{ConnectionId, Nonce, StreamId, TimestampMs};
