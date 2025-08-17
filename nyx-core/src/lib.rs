
#![forbid(unsafe_code)]

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

// Optional subsystems
#[cfg(feature = "plugin_framework")]
pub mod plugin_framework;

#[cfg(feature = "zero_copy")]
pub mod zero_copy;

