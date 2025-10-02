#![forbid(unsafe_code)]
#![allow(missing_docs)]

//! Core utilities for Nyx.
//!
//! This crate intentionally stays lightweight and pure Rust.
//! Public modules export typed IDs, basic error handling,
//! and configuration helpers used across the workspace.
//!
//! Design goals:
//! - Small, dependency-minimized surface
//! - Clear, documented types with safe helpers
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
pub mod security; // Post-Compromise Recovery (PCR) detection and triggering
pub mod types;

// Optional subsystem_s
#[cfg(feature = "plugin_framework")]
pub mod plugin_framework;

#[cfg(feature = "zero_copy")]
pub mod zero_copy;

// Re-export commonly used types
pub use error::{Error, Result};
pub use types::{ConnectionId, Nonce, StreamId, TimestampMs};
