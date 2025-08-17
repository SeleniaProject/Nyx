#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginSettings {
	#[serde(default = "default_queue_size")] 
	pub queue_size: usize,
	#[serde(default = "default_max_errors")] 
	pub max_errors: u32,
}

const fn default_queue_size() -> usize { 1024 }
const fn default_max_errors() -> u32 { 100 }

impl Default for PluginSettings {
	fn default() -> Self { Self { queue_size: default_queue_size(), max_errors: default_max_errors() } }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn defaults_are_sane() {
		let s = PluginSettings::default();
		assert_eq!(s.queue_size, 1024);
		assert_eq!(s.max_errors, 100);
	}
}
