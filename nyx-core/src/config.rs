use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, net::SocketAddr};

/// Core configuration shared across Nyx components.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoreConfig {
	/// Global log level; one of: trace, debug, info, warn, error
	pub log_level: String,
	/// Whether multipath dataplane features are enabled.
	pub enable_multipath: bool,
}

impl Default for CoreConfig {
	fn default() -> Self {
		Self { log_level: "info".into(), enable_multipath: false }
	}
}

impl CoreConfig {
	/// Load configuration from TOML file and validate.
	pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
		let data = fs::read_to_string(path)?;
		let cfg: Self = toml::from_str(&data).map_err(|e| Error::config(format!("toml parse error: {e}")))?;
		cfg.validate()?;
		Ok(cfg)
	}

	/// Build a config from environment variables and validate.
	/// Recognized variables:
	/// - NYX_LOG_LEVEL
	/// - NYX_ENABLE_MULTIPATH (true/false/1/0)
	pub fn from_env() -> Result<Self> {
		let mut cfg = Self::default();
		if let Ok(v) = std::env::var("NYX_LOG_LEVEL") { cfg.log_level = v; }
		if let Ok(v) = std::env::var("NYX_ENABLE_MULTIPATH") { cfg.enable_multipath = matches!(v.as_str(), "1" | "true" | "TRUE" | "True"); }
		cfg.validate()?;
		Ok(cfg)
	}

	/// Validate logical consistency of fields.
	pub fn validate(&self) -> Result<()> {
		let allowed = ["trace","debug","info","warn","error"];
		if !allowed.contains(&self.log_level.as_str()) {
			return Err(Error::config(format!("invalid log_level: {}", self.log_level)));
		}
		Ok(())
	}

	/// Write this configuration to a TOML file.
	pub fn write_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
		let toml = toml::to_string_pretty(self).map_err(|e| Error::config(format!("toml serialize error: {e}")))?;
		fs::write(path, toml)?;
		Ok(())
	}

	/// Create a builder for programmatic construction.
	pub fn builder() -> CoreConfigBuilder { CoreConfigBuilder::default() }
}

/// Builder for `CoreConfig`.
#[derive(Debug, Default)]
pub struct CoreConfigBuilder {
	log_level: Option<String>,
	enable_multipath: Option<bool>,
}

impl CoreConfigBuilder {
	pub fn log_level(mut self, level: impl Into<String>) -> Self { self.log_level = Some(level.into()); self }
	pub fn enable_multipath(mut self, enabled: bool) -> Self { self.enable_multipath = Some(enabled); self }
	pub fn build(self) -> Result<CoreConfig> {
		let mut cfg = CoreConfig::default();
		if let Some(v) = self.log_level { cfg.log_level = v; }
		if let Some(v) = self.enable_multipath { cfg.enable_multipath = v; }
		cfg.validate()?;
		Ok(cfg)
	}
}

/// QUIC transport configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuicConfig {
    /// Local bind address for QUIC endpoint
    pub bind_addr: SocketAddr,
    /// Connection idle timeout in seconds
    pub idle_timeout_secs: u64,
    /// Keep-alive interval in seconds
    pub keep_alive_interval_secs: u64,
    /// Maximum concurrent streams per connection
    pub max_concurrent_streams: u64,
}

impl Default for QuicConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            idle_timeout_secs: 300,
            keep_alive_interval_secs: 30,
            max_concurrent_streams: 100,
        }
    }
}

impl QuicConfig {
    /// Validate QUIC configuration parameters
    pub fn validate(&self) -> Result<()> {
        if self.idle_timeout_secs == 0 {
            return Err(Error::config("idle_timeout_secs must be greater than 0".to_string()));
        }
        if self.keep_alive_interval_secs == 0 {
            return Err(Error::config("keep_alive_interval_secs must be greater than 0".to_string()));
        }
        if self.keep_alive_interval_secs >= self.idle_timeout_secs {
            return Err(Error::config("keep_alive_interval_secs must be less than idle_timeout_secs".to_string()));
        }
        if self.max_concurrent_streams == 0 {
            return Err(Error::config("max_concurrent_streams must be greater than 0".to_string()));
        }
        Ok(())
    }
}
