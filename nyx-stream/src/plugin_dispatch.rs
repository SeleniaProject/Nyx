#![forbid(unsafe_code)]

use crate::plugin::PluginHeader;
use crate::plugin_registry::{PluginInfo, PluginRegistry};
use crate::plugin_sandbox::SandboxPolicy;
use std::sync::Arc;

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

/// Plugin dispatcher for managing plugin execution
#[derive(Debug)]
pub struct PluginDispatcher {
    #[allow(dead_code)]
    registry: Arc<PluginRegistry>,
    #[allow(dead_code)]
    sandbox_policy: Option<SandboxPolicy>,
}

impl PluginDispatcher {
    /// Create a new plugin dispatcher
    pub fn new(registry: Arc<PluginRegistry>) -> Self {
        Self {
            registry,
            sandbox_policy: None,
        }
    }

    /// Create a new plugin dispatcher with sandbox policy
    pub fn new_with_sandbox(registry: Arc<PluginRegistry>, policy: SandboxPolicy) -> Self {
        Self {
            registry,
            sandbox_policy: Some(policy),
        }
    }

    /// Load a plugin
    pub async fn load_plugin(&self, _info: PluginInfo) -> Result<(), Box<dyn std::error::Error>> {
        // Implementation would go here
        Ok(())
    }
}
