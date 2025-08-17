#![forbid(unsafe_code)]

//! nyx-control: Control plane utilities for Nyx.
//! This crate provides:
//! - Lightweight HTTP health/readiness probe without extra deps (tokio TCP + manual HTTP framing)
//! - Settings validation against JSON Schema and a sync orchestration trait
//! - Push token issue/verification using PASETO v4.local (pure Rust)
//! - Rendezvous registration payload signing using Ed25519

use serde::{Deserialize, Serialize};

pub mod probe;
pub mod push;
pub mod rendezvous;
pub mod settings;
pub mod settings_sync;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("invalid config: {0}")] Invalid(String),
	#[error("io error: {0}")] Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlConfig {
	/// Enable built-in HTTP health probes on a TCP listener speaking HTTP/1.1
	#[serde(default = "default_true")] pub enable_http: bool,
	/// Listening port for probes. 0 = auto-assign ephemeral port.
	#[serde(default)] pub port: u16,
}

fn default_true() -> bool { true }

impl Default for ControlConfig {
	fn default() -> Self { Self { enable_http: true, port: 0 } }
}

/// Parse TOML config and fill missing fields with defaults.
pub fn parse_config(s: &str) -> Result<ControlConfig> {
	let s = s.trim();
	if s.is_empty() { return Ok(ControlConfig::default()); }
	toml::from_str::<ControlConfig>(s).map_err(|e| Error::Invalid(e.to_string()))
}

/// Handle to a running control plane tasks set.
pub struct ControlHandle {
	pub probe: Option<probe::ProbeHandle>,
}

impl ControlHandle {
	/// Gracefully shutdown all tasks and wait for them to finish.
	pub async fn shutdown(self) {
		if let Some(h) = self.probe { h.shutdown().await; }
	}
}

/// Start control plane tasks according to config.
pub async fn start_control(cfg: ControlConfig) -> Result<ControlHandle> {
	let mut handle = ControlHandle { probe: None };
	if cfg.enable_http {
		let ph = probe::start_probe(cfg.port).await?;
		handle.probe = Some(ph);
	}
	Ok(handle)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_defaults_on_empty() {
		let c = parse_config("").unwrap();
		assert!(c.enable_http);
		assert_eq!(c.port, 0);
	}

	#[test]
	fn parse_toml_partial() {
		let c = parse_config("port = 8080").unwrap();
		assert!(c.enable_http);
		assert_eq!(c.port, 8080);
	}
}

