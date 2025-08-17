#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("invalid config: {0}")] Invalid(String),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlConfig {
	#[serde(default = "default_true")] pub enable_http: bool,
	#[serde(default)] pub port: u16,
}

fn default_true() -> bool { true }

impl Default for ControlConfig {
	fn default() -> Self { Self { enable_http: true, port: 0 } }
}

/// nyx.toml をきちんとTOMLとして解釈し、欠落項目はデフォルト値で補完する。
pub fn parse_config(s: &str) -> Result<ControlConfig> {
	let s = s.trim();
	if s.is_empty() { return Ok(ControlConfig::default()); }
	toml::from_str::<ControlConfig>(s).map_err(|e| Error::Invalid(e.to_string()))
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

