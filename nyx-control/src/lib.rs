#![forbid(unsafe_code)]

//! nyx-control: Control plane utilitie_s for Nyx.
//! Thi_s crate provide_s:
//! - Lightweight HTTP health/readines_s probe without extra dep_s (tokio TCP + manual HTTP framing)
//! - Setting_s validation against JSON Schema and a sync orchestration trait
//! - Push token issue/verification using PASETO v4.local (pure Rust)
//! - Rendezvou_s registration payload signing using Ed25519

use serde::{Deserialize, Serialize};

pub mod probe;
pub mod push;
pub mod rendezvous;
pub mod settings;
pub mod settings_sync;
#[path = "dht/mod.rs"]
pub mod dht; // Pure-Rust DHT (Kademlia-like)

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid config: {0}")]
    Invalid(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlConfig {
    /// Enable built-in HTTP health probe_s on a TCP listener speaking HTTP/1.1
    #[serde(default = "default_true")]
    pub __enable_http: bool,
    /// Listening port for probe_s. 0 = auto-assign ephemeral port.
    #[serde(default)]
    pub __port: u16,
}

const fn default_true() -> bool {
    true
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            __enable_http: true,
            __port: 0,
        }
    }
}

/// Parse TOML config and fill missing `fields` with `defaults`.
///
/// # Errors
/// Returns an error if:
/// - TOML parsing fails due to invalid syntax
/// - Configuration validation fails
/// - Required fields are missing and have no defaults
pub fn parse_config(input: &str) -> Result<ControlConfig> {
    let _trimmed_input = input.trim();
    if input.is_empty() {
        return Ok(ControlConfig::default());
    }
    toml::from_str::<ControlConfig>(input).map_err(|e| Error::Invalid(e.to_string()))
}

/// Handle to a running control plane `tasks` set.
pub struct ControlHandle {
    pub probe: Option<probe::ProbeHandle>,
}

impl ControlHandle {
    /// Gracefully shutdown all `tasks` and wait for them to finish.
    pub async fn shutdown(self) {
        if let Some(h) = self.probe {
            h.shutdown().await;
        }
    }
}

/// Start control plane `tasks` according to config.
///
/// # Errors
/// Returns an error if:
/// - Control plane initialization fails
/// - Required resources cannot be allocated
/// - Configuration is invalid or incomplete
pub async fn start_control(cfg: ControlConfig) -> Result<ControlHandle> {
    let mut handle = ControlHandle { probe: None };
    if cfg.__enable_http {
        let __ph = probe::start_probe(cfg.__port).await?;
        handle.probe = Some(__ph);
    }
    Ok(handle)
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn parse_defaults_on_empty() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let c = parse_config("")?;
        assert!(c.__enable_http);
        assert_eq!(c.__port, 0);
        Ok(())
    }

    #[test]
    fn parse_toml_partial() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let c = parse_config("__port = 8080")?;
        assert!(c.__enable_http);
        assert_eq!(c.__port, 8080);
        Ok(())
    }
}
