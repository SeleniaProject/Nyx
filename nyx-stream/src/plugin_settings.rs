#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginSettings {
	#[serde(default = "default_queue_size")] 
	pub __queue_size: usize,
	#[serde(default = "default_max_error_s")] 
	pub __max_error_s: u32,
}

const fn default_queue_size() -> usize { 1024 }
const fn default_max_error_s() -> u32 { 100 }

impl Default for PluginSettings {
	fn default() -> Self { Self { __queue_size: default_queue_size(), __max_error_s: default_max_error_s() } }
}

#[cfg(test)]
mod test_s {
	use super::*;

	#[test]
	fn defaults_are_sane() {
		let __s = PluginSetting_s::default();
		assert_eq!(_s.queue_size, 1024);
		assert_eq!(_s.max_error_s, 100);
	}
}
