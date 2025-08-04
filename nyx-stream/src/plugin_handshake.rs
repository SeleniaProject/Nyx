#![forbid(unsafe_code)]

//! Plugin handshake and capability negotiation for Nyx Protocol v1.0.
//!
//! This module handles the plugin-specific aspects of the Nyx handshake process,
//! including advertising plugin requirements, validating peer plugin support,
//! and establishing plugin communication channels.
//!
//! ## Handshake Flow
//! 1. During CRYPTO frame exchange, both peers include plugin capability information
//! 2. SETTINGS frames advertise required/optional plugins and security policies  
//! 3. Plugin compatibility is validated before connection establishment
//! 4. Unsupported required plugins trigger immediate connection termination
//! 5. Compatible plugins are initialized and IPC channels established

use std::collections::{HashMap, HashSet};
use bytes::Bytes;
use tracing::{debug, warn, error, info};
use thiserror::Error;

#[cfg(feature = "plugin")]
use crate::plugin_registry::{PluginRegistry, PluginInfo, Permission};
#[cfg(feature = "plugin")]
use crate::plugin_dispatch::PluginDispatcher;
#[cfg(feature = "plugin")]
use crate::plugin_ipc::{PluginIpcTx, PluginIpcRx, spawn_ipc_server};

use crate::management::{
    Setting, SettingsFrame, build_settings_frame, parse_settings_frame,
    setting_ids, plugin_support_flags, plugin_security_flags,
    ERR_UNSUPPORTED_CAP, build_close_unsupported_cap
};

#[cfg(feature = "plugin")]
use crate::management::plugin_settings::{
    PluginSettingsInfo, extract_plugin_settings,
    build_plugin_support_setting, build_plugin_security_setting,
    encode_plugin_list, decode_plugin_list
};

/// Errors that can occur during plugin handshake
#[derive(Error, Debug, Clone)]
pub enum PluginHandshakeError {
    #[error("Required plugin {plugin_id} not supported by peer")]
    UnsupportedRequiredPlugin { plugin_id: u32 },
    #[error("Plugin capability negotiation failed: {reason}")]
    CapabilityNegotiationFailed { reason: String },
    #[error("Plugin security policy incompatible: {details}")]
    SecurityPolicyIncompatible { details: String },
    #[error("Plugin IPC initialization failed: {error}")]
    IpcInitializationFailed { error: String },
    #[error("Plugin registry error: {error}")]
    RegistryError { error: String },
    #[error("CBOR encoding/decoding error: {error}")]
    CborError { error: String },
}

/// Result of plugin handshake negotiation
#[derive(Debug, Clone)]
pub enum PluginHandshakeResult {
    /// Handshake completed successfully with plugin support enabled
    Success {
        /// Number of required plugins successfully negotiated
        required_plugins: u32,
        /// Number of optional plugins enabled
        optional_plugins: u32,
        /// Negotiated security policy flags
        security_policy: u32,
    },
    /// Handshake completed but no plugin support (compatible fallback)
    NoPluginSupport,
    /// Handshake failed due to incompatible plugin requirements
    Failed {
        /// Error that caused the failure
        error: PluginHandshakeError,
        /// CLOSE frame payload to send to peer
        close_payload: Vec<u8>,
    },
}

/// Plugin handshake coordinator
pub struct PluginHandshakeCoordinator {
    #[cfg(feature = "plugin")]
    registry: PluginRegistry,
    #[cfg(feature = "plugin")]
    dispatcher: Option<PluginDispatcher>,
    
    /// Local plugin capabilities to advertise
    local_capabilities: u32,
    /// Local security policy requirements
    local_security_policy: u32,
    /// Required plugins that peer must support
    required_plugins: Vec<u32>,
    /// Optional plugins we'd like to use if peer supports them
    optional_plugins: Vec<u32>,
}

impl PluginHandshakeCoordinator {
    /// Create new plugin handshake coordinator
    #[cfg(feature = "plugin")]
    pub fn new(
        registry: PluginRegistry,
        local_capabilities: u32,
        local_security_policy: u32,
        required_plugins: Vec<u32>,
        optional_plugins: Vec<u32>,
    ) -> Self {
        Self {
            registry,
            dispatcher: None,
            local_capabilities,
            local_security_policy,
            required_plugins,
            optional_plugins,
        }
    }

    /// Create minimal coordinator for non-plugin builds
    #[cfg(not(feature = "plugin"))]
    pub fn new() -> Self {
        Self {
            local_capabilities: 0,
            local_security_policy: 0,
            required_plugins: Vec::new(),
            optional_plugins: Vec::new(),
        }
    }

    /// Build SETTINGS frame advertising local plugin capabilities
    pub fn build_plugin_settings(&self) -> Vec<Setting> {
        let mut settings = Vec::new();

        // Always advertise plugin support capabilities (even if 0 for no support)
        settings.push(Setting {
            id: setting_ids::PLUGIN_SUPPORT,
            value: self.local_capabilities,
        });

        // Advertise security policy
        settings.push(Setting {
            id: setting_ids::PLUGIN_SECURITY_POLICY,
            value: self.local_security_policy,
        });

        // Advertise required plugins if any
        if !self.required_plugins.is_empty() {
            settings.push(Setting {
                id: setting_ids::PLUGIN_REQUIRED,
                value: self.required_plugins.len() as u32,
            });
        }

        // Advertise optional plugins if any
        if !self.optional_plugins.is_empty() {
            settings.push(Setting {
                id: setting_ids::PLUGIN_OPTIONAL,
                value: self.optional_plugins.len() as u32,
            });
        }

        debug!("Built plugin SETTINGS: capabilities=0x{:08X}, security=0x{:08X}, required={}, optional={}",
               self.local_capabilities, self.local_security_policy, 
               self.required_plugins.len(), self.optional_plugins.len());

        settings
    }

    /// Process peer's plugin settings and perform compatibility check
    pub async fn process_peer_settings(
        &mut self,
        peer_settings: &SettingsFrame,
    ) -> Result<PluginHandshakeResult, PluginHandshakeError> {
        
        #[cfg(feature = "plugin")]
        {
            let peer_info = extract_plugin_settings(peer_settings);
            
            debug!("Processing peer plugin settings: support=0x{:08X}, security=0x{:08X}, required={}, optional={}",
                   peer_info.support_flags, peer_info.security_policy,
                   peer_info.required_plugin_count, peer_info.optional_plugin_count);

            // Check if peer supports plugin frames at all
            if !peer_info.supports_plugin_frames() && !self.required_plugins.is_empty() {
                let error = PluginHandshakeError::UnsupportedRequiredPlugin {
                    plugin_id: self.required_plugins[0], // Report first required plugin
                };
                return Ok(PluginHandshakeResult::Failed {
                    error,
                    close_payload: build_close_unsupported_cap(self.required_plugins[0]),
                });
            }

            // Validate security policy compatibility
            let security_check = self.validate_security_policy(&peer_info);
            if let Err(error) = security_check {
                return Ok(PluginHandshakeResult::Failed {
                    error,
                    close_payload: build_close_unsupported_cap(0xFFFFFFFF), // Generic policy error
                });
            }

            // Check required plugins compatibility
            let plugin_check = self.validate_required_plugins(&peer_info).await;
            if let Err(error) = plugin_check {
                let plugin_id = match &error {
                    PluginHandshakeError::UnsupportedRequiredPlugin { plugin_id } => *plugin_id,
                    _ => 0xFFFFFFFF,
                };
                return Ok(PluginHandshakeResult::Failed {
                    error,
                    close_payload: build_close_unsupported_cap(plugin_id),
                });
            }

            // If we reach here, plugin negotiation was successful
            let compatible_optional = self.count_compatible_optional_plugins(&peer_info);
            
            info!("Plugin handshake successful: required={}, optional={}", 
                  self.required_plugins.len(), compatible_optional);

            Ok(PluginHandshakeResult::Success {
                required_plugins: self.required_plugins.len() as u32,
                optional_plugins: compatible_optional,
                security_policy: peer_info.security_policy & self.local_security_policy,
            })
        }

        #[cfg(not(feature = "plugin"))]
        {
            // For non-plugin builds, check if peer requires any plugins
            let peer_info = extract_plugin_settings(peer_settings);
            if peer_info.required_plugin_count > 0 {
                warn!("Peer requires {} plugins but plugin support not enabled", peer_info.required_plugin_count);
                let error = PluginHandshakeError::UnsupportedRequiredPlugin { plugin_id: 0 };
                return Ok(PluginHandshakeResult::Failed {
                    error,
                    close_payload: build_close_unsupported_cap(0),
                });
            }

            Ok(PluginHandshakeResult::NoPluginSupport)
        }
    }

    /// Validate security policy compatibility between local and peer requirements
    #[cfg(feature = "plugin")]
    fn validate_security_policy(&self, peer_info: &PluginSettingsInfo) -> Result<(), PluginHandshakeError> {
        // Check if our required security features are supported by peer
        let required_features = self.local_security_policy;
        let peer_features = peer_info.security_policy;
        
        // We require signature verification but peer doesn't support it
        if (required_features & plugin_security_flags::REQUIRE_SIGNATURES != 0) &&
           (peer_features & plugin_security_flags::REQUIRE_SIGNATURES == 0) {
            return Err(PluginHandshakeError::SecurityPolicyIncompatible {
                details: "Peer does not support required signature verification".to_string(),
            });
        }

        // Additional security policy checks can be added here
        
        debug!("Security policy validation passed");
        Ok(())
    }

    /// Validate that peer supports all our required plugins
    #[cfg(feature = "plugin")]
    async fn validate_required_plugins(&self, peer_info: &PluginSettingsInfo) -> Result<(), PluginHandshakeError> {
        // For now, we assume peer declares plugin support through the support flags
        // In a full implementation, this would involve detailed plugin ID negotiation
        
        if !self.required_plugins.is_empty() && !peer_info.supports_plugin_frames() {
            return Err(PluginHandshakeError::UnsupportedRequiredPlugin {
                plugin_id: self.required_plugins[0],
            });
        }

        // TODO: Implement detailed per-plugin compatibility checking
        // This would involve:
        // 1. Exchanging detailed plugin ID lists
        // 2. Version compatibility checking  
        // 3. Plugin dependency resolution
        
        debug!("Required plugin validation passed for {} plugins", self.required_plugins.len());
        Ok(())
    }

    /// Count how many optional plugins are compatible with peer
    #[cfg(feature = "plugin")]
    fn count_compatible_optional_plugins(&self, peer_info: &PluginSettingsInfo) -> u32 {
        // Simplified logic - assume all optional plugins are compatible if peer supports plugin frames
        if peer_info.supports_plugin_frames() {
            self.optional_plugins.len() as u32
        } else {
            0
        }
    }

    /// Initialize plugin dispatcher after successful handshake
    #[cfg(feature = "plugin")]
    pub async fn initialize_plugin_dispatcher(&mut self) -> Result<(), PluginHandshakeError> {
        if self.dispatcher.is_some() {
            return Ok(()); // Already initialized
        }

        let dispatcher = PluginDispatcher::new(self.registry.clone());
        
        // TODO: Initialize required plugins and their IPC channels
        // This would involve:
        // 1. Loading required plugin modules
        // 2. Setting up IPC communication channels
        // 3. Registering plugins with the dispatcher
        // 4. Performing plugin-specific initialization
        
        self.dispatcher = Some(dispatcher);
        
        info!("Plugin dispatcher initialized successfully");
        Ok(())
    }

    /// Get reference to plugin dispatcher (if initialized)
    #[cfg(feature = "plugin")]
    pub fn get_dispatcher(&self) -> Option<&PluginDispatcher> {
        self.dispatcher.as_ref()
    }

    /// Check if plugin support is enabled and negotiated
    pub fn is_plugin_support_active(&self) -> bool {
        #[cfg(feature = "plugin")]
        {
            self.dispatcher.is_some()
        }
        #[cfg(not(feature = "plugin"))]
        {
            false
        }
    }
}

/// Utility function to extract plugin settings from non-plugin builds
#[cfg(not(feature = "plugin"))]
fn extract_plugin_settings(frame: &SettingsFrame) -> BasicPluginInfo {
    let mut info = BasicPluginInfo::default();
    
    for setting in &frame.settings {
        match setting.id {
            setting_ids::PLUGIN_SUPPORT => {
                info.support_flags = setting.value;
            }
            setting_ids::PLUGIN_REQUIRED => {
                info.required_plugin_count = setting.value;
            }
            _ => {}
        }
    }
    
    info
}

/// Minimal plugin info for non-plugin builds
#[cfg(not(feature = "plugin"))]
#[derive(Debug, Clone, Default)]
struct BasicPluginInfo {
    support_flags: u32,
    required_plugin_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_settings_building() {
        #[cfg(feature = "plugin")]
        {
            let registry = PluginRegistry::new();
            let coordinator = PluginHandshakeCoordinator::new(
                registry,
                plugin_support_flags::BASIC_FRAMES | plugin_support_flags::DYNAMIC_LOADING,
                plugin_security_flags::REQUIRE_SIGNATURES,
                vec![1001, 1002],
                vec![2001, 2002, 2003],
            );

            let settings = coordinator.build_plugin_settings();
            assert!(!settings.is_empty());
            
            // Should have at least plugin support and security policy settings
            assert!(settings.iter().any(|s| s.id == setting_ids::PLUGIN_SUPPORT));
            assert!(settings.iter().any(|s| s.id == setting_ids::PLUGIN_SECURITY_POLICY));
        }

        #[cfg(not(feature = "plugin"))]
        {
            let coordinator = PluginHandshakeCoordinator::new();
            let settings = coordinator.build_plugin_settings();
            // Should still build settings (with zero capabilities)
            assert!(!settings.is_empty());
        }
    }

    #[test]
    fn test_no_plugin_support_compatibility() {
        let coordinator = PluginHandshakeCoordinator::new();
        
        // Peer with no plugin requirements should be compatible
        let peer_settings = SettingsFrame {
            settings: vec![
                Setting { id: setting_ids::PLUGIN_SUPPORT, value: 0 },
            ],
        };

        // This test requires async runtime for processing
        // Result validation would be done in integration tests
    }
}
