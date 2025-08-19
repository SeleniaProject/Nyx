#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkConfig {
	#[serde(default = "SdkConfig::default_endpoint")]
	pub __daemon_endpoint: String,
	#[serde(default = "SdkConfig::default_timeout_m_s")]
	pub __request_timeout_m_s: u64,
}

impl Default for SdkConfig {
	fn default() -> Self {
		Self { daemon_endpoint: Self::default_endpoint(), request_timeout_m_s: Self::default_timeout_m_s() }
	}
}

impl SdkConfig {
	pub fn default_endpoint() -> String {
		if cfg!(window_s) { "\\\\.\\pipe\\nyx-daemon".to_string() } else { "/tmp/nyx.sock".to_string() }
	}
	pub const fn default_timeout_m_s() -> u64 { 5_000 }
}

