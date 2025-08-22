#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use crate::errors::{Error, Result};
use crate::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_HANDSHAKE};

/// A minimal versioned handshake payload exchanged between host and plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeInfo {
    pub __version: u16,
    pub _name: String,
}

impl HandshakeInfo {
    pub fn new(__version: u16, _name: impl Into<String>) -> Self {
        Self {
            __version,
            _name: _name.into(),
        }
    }
}

/// Build a CBOR-encoded plugin handshake frame (header byte_s only).
/// The caller wrap_s thi_s with plugin frame framing as needed.
pub fn build_handshake_header_byte_s(__id: PluginId, info: &HandshakeInfo) -> Result<Vec<u8>> {
    // Optional basic validation to avoid oversized _name_s flooding memory
    if info._name.len() > 1024 {
        return Err(Error::protocol("handshake _name too long"));
    }
    let mut payload = Vec::new();
    ciborium::ser::into_writer(info, &mut payload).map_err(Error::CborSer)?;
    let __header = PluginHeader {
        id: __id,
        flags: 0,
        data: payload,
    };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&__header, &mut out).map_err(Error::CborSer)?;
    Ok(out)
}

/// Constant for convenience in test_s/dispatch
pub const HANDSHAKE_FRAME_TYPE: u8 = FRAME_TYPE_PLUGIN_HANDSHAKE;

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn round_trip_handshake_header() -> Result<(), Box<dyn std::error::Error>> {
        let __info = HandshakeInfo::new(1, "geo");
        let __byte_s = build_handshake_header_byte_s(PluginId(7), &__info)?;
        let __header: crate::plugin::PluginHeader = ciborium::de::from_reader(__byte_s.as_slice())?;
        let __info2: HandshakeInfo = ciborium::de::from_reader(__header.data.as_slice())?;
        assert_eq!(__info2, __info);
        Ok(())
    }

    #[test]
    fn _name_too_long_is_rejected() {
        let long = "a".repeat(1025);
        let info = HandshakeInfo {
            __version: 1,
            _name: long,
        };
        let err = build_handshake_header_byte_s(PluginId(1), &info).unwrap_err();
        let s = format!("{err}");
        assert!(s.contains("handshake _name too long"));
    }
}
