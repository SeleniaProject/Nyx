#![forbid(unsafe_code)]

use thiserror::Error;

use crate::plugin::PluginHeader;

#[derive(Debug, Error)]
pub enum PluginCborError {
	#[error("cbor decode error: {0}")]
	Decode(String),
	#[error("cbor header too large: {0} bytes")] 
	Oversize(usize),
}

pub fn parse_plugin_header(bytes: &[u8]) -> Result<PluginHeader, PluginCborError> {
	// 制御プレーンのヘッダは小さくあるべき。攻撃的な巨大CBORを拒否してDoS余地を抑える。
	const MAX_HEADER_CBOR_LEN: usize = 4 * 1024; // 4 KiB 上限
	if bytes.len() > MAX_HEADER_CBOR_LEN { return Err(PluginCborError::Oversize(bytes.len())); }
	let reader = std::io::Cursor::new(bytes);
	ciborium::de::from_reader(reader).map_err(|e| PluginCborError::Decode(e.to_string()))
}
