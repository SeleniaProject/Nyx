#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;

/// Opaque Plugin identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(pub u32);

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Plugin Framework frame type range_s and helpers (0x50?0x5F)
pub const FRAME_TYPE_PLUGIN_HANDSHAKE: u8 = 0x50;
pub const FRAME_TYPE_PLUGIN_DATA: u8 = 0x51;
pub const FRAME_TYPE_PLUGIN_CONTROL: u8 = 0x52;
pub const FRAME_TYPE_PLUGIN_ERROR: u8 = 0x53;

#[inline]
pub fn is_plugin_frame(t: u8) -> bool {
    (0x50..=0x5F).contains(&t)
}

/// Minimal plugin header carried inside plugin frames (encoded in CBOR)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginHeader {
    pub id: PluginId,
    pub flags: u8,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

impl PluginHeader {
    pub fn new(id: PluginId, flags: u8, data: Vec<u8>) -> Self {
        Self { id, flags, data }
    }

    pub fn required(id: PluginId, data: Vec<u8>) -> Self {
        const FLAG_REQUIRED: u8 = 0x01;
        Self::new(id, FLAG_REQUIRED, data)
    }

    pub fn optional(id: PluginId, data: Vec<u8>) -> Self {
        const FLAG_OPTIONAL: u8 = 0x00;
        Self::new(id, FLAG_OPTIONAL, data)
    }
}
