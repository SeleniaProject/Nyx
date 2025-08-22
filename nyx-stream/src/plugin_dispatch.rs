#![forbid(unsafe_code)]

use crate::plugin::PluginHeader;

/// Message structure for plugin communication
#[derive(Debug, Clone)]
pub struct PluginMessage {
    /// Frame type identifier for the plugin message
    pub frame_type: u8,
    /// Plugin header containing metadata
    pub header: PluginHeader,
    /// Raw payload data
    pub payload: Vec<u8>,
}

impl PluginMessage {
    /// Create a new plugin message
    pub fn new(frame_type: u8, header: PluginHeader, payload: Vec<u8>) -> Self {
        Self {
            frame_type,
            header,
            payload,
        }
    }
    
    /// Get the plugin ID from the header
    pub fn plugin_id(&self) -> crate::plugin::PluginId {
        self.header.id
    }
    
    /// Check if this message is marked as required
    pub fn is_required(&self) -> bool {
        (self.header.flags & 0x01) != 0
    }
    
    /// Get the total size of the message
    pub fn size(&self) -> usize {
        self.payload.len() + self.header.data.len()
    }
}
