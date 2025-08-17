#[cfg(feature = "plugin_framework")]
use cbor4ii::serde as cbor;
use serde::{Deserialize, Serialize};

/// Minimal plugin manifest and message schema (feature-gated for CBOR).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest { pub name: String, pub version: String }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMessage { pub plugin: String, pub payload: Vec<u8> }

#[cfg(feature = "plugin_framework")]
pub fn encode_msg(msg: &PluginMessage) -> Result<Vec<u8>, cbor::ser::Error<Vec<u8>>> {
	cbor::to_vec_simple(msg)
}

#[cfg(feature = "plugin_framework")]
pub fn decode_msg(bytes: &[u8]) -> Result<PluginMessage, cbor::de::Error<cbor4ii::serde::DecodeError>> {
	cbor::from_slice(bytes)
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn manifest_eq() {
		let m = PluginManifest { name: "x".into(), version: "1".into() };
		assert_eq!(m.name, "x");
	}
}
