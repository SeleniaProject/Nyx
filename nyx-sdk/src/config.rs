#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdkConfig {
    #[serde(default = "SdkConfig::default_endpoint")]
    pub daemon_endpoint: String,
    #[serde(default = "SdkConfig::default_timeout_ms")]
    pub request_timeout_ms: u64,
}

impl Default for SdkConfig {
    fn default() -> Self {
        Self {
            daemon_endpoint: Self::default_endpoint(),
            request_timeout_ms: Self::default_timeout_ms(),
        }
    }
}

impl SdkConfig {
    pub fn default_endpoint() -> String {
        if cfg!(windows) {
            "\\\\.\\pipe\\nyx-daemon".to_string()
        } else {
            "/tmp/nyx.sock".to_string()
        }
    }
    fn default_timeout_ms() -> u64 {
        10000
    }
}
