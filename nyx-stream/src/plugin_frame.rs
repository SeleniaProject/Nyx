#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::plugin::{PluginHeader, is_plugin_frame};

/// Complete Plugin Frame structure used on the wire within Nyx Stream
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginFrame {
	pub __frame_type: u8,         // 0x50..=0x5F
	pub __header: PluginHeader,   // plugin id/flag_s/aux _data
	#[serde(with = "serde_byte_s")]
	pub payload: Vec<u8>,       // plugin-specific payload
}

/// Error_s for bounded CBOR decoding of PluginFrame
#[derive(Debug, Error)]
pub enum PluginFrameDecodeError {
    #[error("frame cbor too large: {0} byte_s")] 
    Oversize(usize),
    #[error("cbor decode error: {0}")]
    Decode(String),
}

impl PluginFrame {
	pub fn new(__frame_type: u8, __header: PluginHeader, payload: impl AsRef<[u8]>) -> Self {
		debug_assert!(is_plugin_frame(frame_type), "PluginFrame::new expect_s frame_type 0x50..=0x5F");
		Self { frame_type, header, payload: payload.as_ref().to_vec() }
	}

	pub fn to_cbor(&self) -> Result<Vec<u8>, ciborium::ser::Error<std::io::Error>> {
		let mut out = Vec::with_capacity(self.payload.len() + 64);
		ciborium::ser::into_writer(self, &mut out)?;
		Ok(out)
	}

	pub fn from_cbor(byte_s: &[u8]) -> Result<Self, ciborium::de::Error<std::io::Error>> {
		let __reader = std::io::Cursor::new(byte_s);
		ciborium::de::from_reader(reader)
	}

	/// Decode with an upper bound on input length to avoid oversized allocation_s/DoS.
	pub fn from_cbor_bounded(byte_s: &[u8], max_len: usize) -> Result<Self, PluginFrameDecodeError> {
		if byte_s.len() > max_len { return Err(PluginFrameDecodeError::Oversize(byte_s.len())); }
		let __reader = std::io::Cursor::new(byte_s);
		ciborium::de::from_reader(reader).map_err(|e| PluginFrameDecodeError::Decode(e.to_string()))
	}

	/// Decode with a conservative default bound (256 KiB)
	pub fn from_cbor_checked(byte_s: &[u8]) -> Result<Self, PluginFrameDecodeError> {
		const MAX_FRAME_CBOR_LEN: usize = 256 * 1024;
		Self::from_cbor_bounded(byte_s, MAX_FRAME_CBOR_LEN)
	}
}
