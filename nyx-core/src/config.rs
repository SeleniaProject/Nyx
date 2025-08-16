use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoreConfig {
	pub log_level: String,
	pub enable_multipath: bool,
}

impl Default for CoreConfig {
	fn default() -> Self {
		Self { log_level: "info".into(), enable_multipath: false }
	}
}

impl CoreConfig {
	pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
		let data = fs::read_to_string(path)?;
		let cfg: Self = toml::from_str(&data).map_err(|e| Error::config(format!("toml parse error: {e}")))?;
		cfg.validate()?;
		Ok(cfg)
	}

	pub fn from_env() -> Result<Self> {
		let mut cfg = Self::default();
		if let Ok(v) = std::env::var("NYX_LOG_LEVEL") { cfg.log_level = v; }
		if let Ok(v) = std::env::var("NYX_ENABLE_MULTIPATH") { cfg.enable_multipath = v == "1" || v.eq_ignore_ascii_case("true"); }
		cfg.validate()?;
		Ok(cfg)
	}

	pub fn validate(&self) -> Result<()> {
		let allowed = ["trace","debug","info","warn","error"];
		if !allowed.contains(&self.log_level.as_str()) {
			return Err(Error::config(format!("invalid log_level: {}", self.log_level)));
		}
		Ok(())
	}
}
