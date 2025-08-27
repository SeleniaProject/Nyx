#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};
use crate::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_HANDSHAKE};

/// A minimal versioned handshake payload exchanged between host and plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeInfo {
    pub version: u16,
    pub name: String,
}

impl HandshakeInfo {
    pub fn new(version: u16, name: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
        }
    }
}

/// Build a CBOR-encoded plugin handshake frame (header bytes only).
/// The caller wraps this with plugin frame framing as needed.
pub fn build_handshake_header_bytes(id: PluginId, info: &HandshakeInfo) -> Result<Vec<u8>> {
    // Optional basic validation to avoid oversized names flooding memory
    if info.name.len() > 1024 {
        return Err(Error::protocol("handshake name too long"));
    }
    let mut payload = Vec::new();
    ciborium::ser::into_writer(info, &mut payload).map_err(Error::CborSer)?;
    let header = PluginHeader {
        id,
        flags: 0,
        data: payload,
    };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&header, &mut out).map_err(Error::CborSer)?;
    Ok(out)
}

/// Constant for convenience in tests/dispatch
pub const HANDSHAKE_FRAME_TYPE: u8 = FRAME_TYPE_PLUGIN_HANDSHAKE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_handshake_header() -> Result<(), Box<dyn std::error::Error>> {
        let info = HandshakeInfo::new(1, "geo");
        let bytes = build_handshake_header_bytes(PluginId(7), &info)?;
        let header: crate::plugin::PluginHeader = ciborium::de::from_reader(bytes.as_slice())?;
        let info2: HandshakeInfo = ciborium::de::from_reader(header.data.as_slice())?;
        assert_eq!(info2, info);
        Ok(())
    }

    #[test]
    fn name_too_long_is_rejected() {
        let long = "a".repeat(1025);
        let info = HandshakeInfo {
            version: 1,
            name: long,
        };
        let err = build_handshake_header_bytes(PluginId(1), &info).unwrap_err();
        let s = format!("{err}");
        assert!(s.contains("handshake name too long"));
    }
}
