#![forbid(unsafe_code)]

//! Plugin Settings and PLUGIN_REQUIRED Advertising for Nyx Protocol v1.0
//!
//! This module implements the SETTINGS frame extension for advertising
//! required plugins during handshake phase as specified in v1.0 §7.2.
//!
//! When a connection requires specific plugins, the PLUGIN_REQUIRED setting
//! must be advertised in the initial SETTINGS frame. Peers that don't support
//! required plugins must terminate the connection with error code 0x07.

use std::collections::HashSet;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use bytes::{Bytes, BytesMut, BufMut};
use tracing::{debug, warn, trace};

/// Settings identifier for plugin requirements in SETTINGS frames
pub const SETTINGS_PLUGIN_REQUIRED: u16 = 0x0010;

/// Maximum number of plugins that can be marked as required
pub const MAX_REQUIRED_PLUGINS: usize = 32;

/// Plugin capability level enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginCapability {
    /// Plugin is not supported
    NotSupported = 0,
    /// Plugin is supported but optional
    Optional = 1,
    /// Plugin is supported and required for this connection
    Required = 2,
}

/// Plugin requirement specification in SETTINGS frames
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequirement {
    /// Plugin identifier
    pub plugin_id: u32,
    /// Minimum supported version (semantic versioning major.minor)
    pub min_version: (u16, u16),
    /// Plugin capability requirement level
    pub capability: PluginCapability,
    /// Optional plugin-specific configuration parameters
    pub config_params: Vec<u8>,
}

/// Plugin settings advertising and negotiation handler
#[derive(Debug)]
pub struct PluginSettingsManager {
    /// Plugins required by this endpoint
    required_plugins: HashSet<u32>,
    /// Plugins optionally supported by this endpoint  
    optional_plugins: HashSet<u32>,
    /// Detailed plugin requirements for SETTINGS frames
    plugin_requirements: Vec<PluginRequirement>,
    /// Whether plugin negotiation is enabled
    negotiation_enabled: bool,
}

/// Errors in plugin settings processing
#[derive(Error, Debug)]
pub enum PluginSettingsError {
    #[error("Too many required plugins: {count} (max: {max})")]
    TooManyRequiredPlugins { count: usize, max: usize },
    
    #[error("Plugin ID {0} is already registered")]
    DuplicatePlugin(u32),
    
    #[error("Plugin ID {0} is reserved for system use")]
    ReservedPluginId(u32),
    
    #[error("SETTINGS frame too large: {size} bytes")]
    SettingsFrameTooLarge { size: usize },
    
    #[error("Malformed plugin requirement data: {0}")]
    MalformedData(String),
    
    #[error("Required plugin {0} not supported by peer")]
    UnsupportedRequiredPlugin(u32),
    
    #[error("Plugin version mismatch: plugin {id}, required {required_major}.{required_minor}, available {available_major}.{available_minor}")]
    VersionMismatch {
        id: u32,
        required_major: u16,
        required_minor: u16,
        available_major: u16,
        available_minor: u16,
    },
}

impl PluginSettingsManager {
    /// Create a new plugin settings manager
    pub fn new() -> Self {
        Self {
            required_plugins: HashSet::new(),
            optional_plugins: HashSet::new(),
            plugin_requirements: Vec::new(),
            negotiation_enabled: true,
        }
    }

    /// Add a required plugin that must be supported by peer
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin identifier
    /// * `min_version` - Minimum supported version (major, minor)
    /// * `config_params` - Optional plugin-specific configuration
    ///
    /// # Returns
    /// * `Ok(())` - Plugin requirement added successfully
    /// * `Err(PluginSettingsError)` - Registration failed
    pub fn add_required_plugin(
        &mut self,
        plugin_id: u32,
        min_version: (u16, u16),
        config_params: Vec<u8>,
    ) -> Result<(), PluginSettingsError> {
        // Validate not a reserved plugin ID
        if plugin_id >= 0xFFFF0000 {
            return Err(PluginSettingsError::ReservedPluginId(plugin_id));
        }

        // Check maximum required plugins limit
        if self.required_plugins.len() >= MAX_REQUIRED_PLUGINS {
            return Err(PluginSettingsError::TooManyRequiredPlugins {
                count: self.required_plugins.len() + 1,
                max: MAX_REQUIRED_PLUGINS,
            });
        }

        // Check for duplicates
        if self.required_plugins.contains(&plugin_id) || self.optional_plugins.contains(&plugin_id) {
            return Err(PluginSettingsError::DuplicatePlugin(plugin_id));
        }

        // Add to required set and create detailed requirement
        self.required_plugins.insert(plugin_id);
        self.plugin_requirements.push(PluginRequirement {
            plugin_id,
            min_version,
            capability: PluginCapability::Required,
            config_params,
        });

        debug!("Added required plugin: ID={}, version={}.{}", plugin_id, min_version.0, min_version.1);
        Ok(())
    }

    /// Add an optional plugin that is supported but not required
    ///
    /// # Arguments
    /// * `plugin_id` - Plugin identifier
    /// * `min_version` - Minimum supported version
    /// * `config_params` - Optional configuration parameters
    pub fn add_optional_plugin(
        &mut self,
        plugin_id: u32,
        min_version: (u16, u16),
        config_params: Vec<u8>,
    ) -> Result<(), PluginSettingsError> {
        // Validate not a reserved plugin ID
        if plugin_id >= 0xFFFF0000 {
            return Err(PluginSettingsError::ReservedPluginId(plugin_id));
        }

        // Check for duplicates
        if self.required_plugins.contains(&plugin_id) || self.optional_plugins.contains(&plugin_id) {
            return Err(PluginSettingsError::DuplicatePlugin(plugin_id));
        }

        // Add to optional set and create detailed requirement
        self.optional_plugins.insert(plugin_id);
        self.plugin_requirements.push(PluginRequirement {
            plugin_id,
            min_version,
            capability: PluginCapability::Optional,
            config_params,
        });

        debug!("Added optional plugin: ID={}, version={}.{}", plugin_id, min_version.0, min_version.1);
        Ok(())
    }

    /// Generate SETTINGS frame data for plugin requirement advertising
    ///
    /// Creates the binary payload for SETTINGS_PLUGIN_REQUIRED (0x0010) that
    /// advertises plugin requirements to peer during handshake.
    ///
    /// Format: [count:u16][requirement1][requirement2]...[requirementN]
    /// Where each requirement is: [plugin_id:u32][min_major:u16][min_minor:u16][capability:u8][config_len:u16][config_data]
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - SETTINGS frame payload bytes
    /// * `Err(PluginSettingsError)` - Serialization failed
    pub fn generate_settings_frame_data(&self) -> Result<Vec<u8>, PluginSettingsError> {
        if !self.negotiation_enabled {
            return Ok(vec![]);
        }

        let mut data = BytesMut::new();
        
        // Write requirement count (16-bit big-endian)
        data.put_u16(self.plugin_requirements.len() as u16);

        // Write each plugin requirement
        for requirement in &self.plugin_requirements {
            // Plugin ID (32-bit big-endian)
            data.put_u32(requirement.plugin_id);
            
            // Minimum version (major.minor, each 16-bit big-endian)
            data.put_u16(requirement.min_version.0);
            data.put_u16(requirement.min_version.1);
            
            // Capability level (8-bit)
            data.put_u8(requirement.capability as u8);
            
            // Configuration parameters length and data
            data.put_u16(requirement.config_params.len() as u16);
            data.extend_from_slice(&requirement.config_params);

            trace!("Serialized plugin requirement: ID={}, version={}.{}, capability={:?}",
                   requirement.plugin_id, requirement.min_version.0, requirement.min_version.1, requirement.capability);
        }

        let result = data.freeze().to_vec();
        
        // Validate total size is reasonable for SETTINGS frames
        if result.len() > 8192 {
            return Err(PluginSettingsError::SettingsFrameTooLarge { size: result.len() });
        }

        debug!("Generated plugin SETTINGS frame data: {} bytes, {} requirements", result.len(), self.plugin_requirements.len());
        Ok(result)
    }

    /// Parse plugin requirements from peer's SETTINGS frame data
    ///
    /// # Arguments
    /// * `settings_data` - Binary payload from SETTINGS_PLUGIN_REQUIRED
    ///
    /// # Returns
    /// * `Ok(Vec<PluginRequirement>)` - Parsed plugin requirements
    /// * `Err(PluginSettingsError)` - Parse failed
    pub fn parse_peer_settings_data(&self, settings_data: &[u8]) -> Result<Vec<PluginRequirement>, PluginSettingsError> {
        if settings_data.len() < 2 {
            return Err(PluginSettingsError::MalformedData("Settings data too short".to_string()));
        }

        let mut cursor = std::io::Cursor::new(settings_data);
        let mut requirements = Vec::new();

        // Read requirement count
        let count = u16::from_be_bytes([
            settings_data[0],
            settings_data[1]
        ]) as usize;
        
        let mut offset = 2;

        // Parse each requirement
        for i in 0..count {
            if offset + 11 > settings_data.len() {
                return Err(PluginSettingsError::MalformedData(
                    format!("Insufficient data for requirement {}", i)
                ));
            }

            // Extract plugin ID (32-bit big-endian)
            let plugin_id = u32::from_be_bytes([
                settings_data[offset],
                settings_data[offset + 1],
                settings_data[offset + 2],
                settings_data[offset + 3],
            ]);
            offset += 4;

            // Extract minimum version (major.minor, each 16-bit big-endian)
            let min_major = u16::from_be_bytes([
                settings_data[offset],
                settings_data[offset + 1],
            ]);
            offset += 2;

            let min_minor = u16::from_be_bytes([
                settings_data[offset],
                settings_data[offset + 1],
            ]);
            offset += 2;

            // Extract capability level (8-bit)
            let capability = match settings_data[offset] {
                0 => PluginCapability::NotSupported,
                1 => PluginCapability::Optional,
                2 => PluginCapability::Required,
                other => return Err(PluginSettingsError::MalformedData(
                    format!("Invalid capability value: {}", other)
                )),
            };
            offset += 1;

            // Extract configuration parameters length and data
            if offset + 2 > settings_data.len() {
                return Err(PluginSettingsError::MalformedData(
                    "Missing config length".to_string()
                ));
            }

            let config_len = u16::from_be_bytes([
                settings_data[offset],
                settings_data[offset + 1],
            ]) as usize;
            offset += 2;

            if offset + config_len > settings_data.len() {
                return Err(PluginSettingsError::MalformedData(
                    "Insufficient data for config params".to_string()
                ));
            }

            let config_params = settings_data[offset..offset + config_len].to_vec();
            offset += config_len;

            requirements.push(PluginRequirement {
                plugin_id,
                min_version: (min_major, min_minor),
                capability,
                config_params,
            });

            trace!("Parsed peer plugin requirement: ID={}, version={}.{}, capability={:?}",
                   plugin_id, min_major, min_minor, capability);
        }

        debug!("Parsed {} plugin requirements from peer SETTINGS frame", requirements.len());
        Ok(requirements)
    }

    /// Validate that we can satisfy peer's plugin requirements
    ///
    /// # Arguments
    /// * `peer_requirements` - Plugin requirements from peer's SETTINGS frame
    ///
    /// # Returns
    /// * `Ok(())` - All requirements can be satisfied
    /// * `Err(PluginSettingsError)` - One or more requirements cannot be satisfied
    pub fn validate_peer_requirements(&self, peer_requirements: &[PluginRequirement]) -> Result<(), PluginSettingsError> {
        for requirement in peer_requirements {
            if requirement.capability == PluginCapability::Required {
                // Check if we support this required plugin
                if !self.required_plugins.contains(&requirement.plugin_id) && 
                   !self.optional_plugins.contains(&requirement.plugin_id) {
                    warn!("Peer requires unsupported plugin: {}", requirement.plugin_id);
                    return Err(PluginSettingsError::UnsupportedRequiredPlugin(requirement.plugin_id));
                }

                // TODO: Add version compatibility checking here when plugin registry is available
                debug!("Validated required plugin: {} (version {}.{})", 
                       requirement.plugin_id, requirement.min_version.0, requirement.min_version.1);
            }
        }

        debug!("Successfully validated all peer plugin requirements");
        Ok(())
    }

    /// Get list of required plugin IDs
    pub fn get_required_plugins(&self) -> &HashSet<u32> {
        &self.required_plugins
    }

    /// Get list of optional plugin IDs  
    pub fn get_optional_plugins(&self) -> &HashSet<u32> {
        &self.optional_plugins
    }

    /// Check if plugin negotiation is enabled
    pub fn is_negotiation_enabled(&self) -> bool {
        self.negotiation_enabled
    }

    /// Enable or disable plugin negotiation
    pub fn set_negotiation_enabled(&mut self, enabled: bool) {
        self.negotiation_enabled = enabled;
        debug!("Plugin negotiation {}", if enabled { "enabled" } else { "disabled" });
    }

    /// Get total number of plugin requirements
    pub fn requirement_count(&self) -> usize {
        self.plugin_requirements.len()
    }

    /// Clear all plugin requirements (useful for testing)
    pub fn clear_all_requirements(&mut self) {
        self.required_plugins.clear();
        self.optional_plugins.clear();
        self.plugin_requirements.clear();
        debug!("Cleared all plugin requirements");
    }
}

impl Default for PluginSettingsManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_required_plugin() {
        let mut manager = PluginSettingsManager::new();
        
        let result = manager.add_required_plugin(12345, (1, 2), vec![0x01, 0x02]);
        assert!(result.is_ok());
        
        assert!(manager.required_plugins.contains(&12345));
        assert_eq!(manager.requirement_count(), 1);
    }

    #[test]
    fn test_add_optional_plugin() {
        let mut manager = PluginSettingsManager::new();
        
        let result = manager.add_optional_plugin(54321, (2, 0), vec![]);
        assert!(result.is_ok());
        
        assert!(manager.optional_plugins.contains(&54321));
        assert_eq!(manager.requirement_count(), 1);
    }

    #[test]
    fn test_duplicate_plugin_rejection() {
        let mut manager = PluginSettingsManager::new();
        
        manager.add_required_plugin(12345, (1, 0), vec![]).expect("First addition");
        let result = manager.add_required_plugin(12345, (1, 1), vec![]);
        
        assert!(matches!(result, Err(PluginSettingsError::DuplicatePlugin(12345))));
    }

    #[test]
    fn test_reserved_plugin_id_rejection() {
        let mut manager = PluginSettingsManager::new();
        
        let result = manager.add_required_plugin(0xFFFF0001, (1, 0), vec![]);
        assert!(matches!(result, Err(PluginSettingsError::ReservedPluginId(0xFFFF0001))));
    }

    #[test]
    fn test_settings_frame_generation_parsing_roundtrip() {
        let mut manager = PluginSettingsManager::new();
        
        manager.add_required_plugin(1001, (1, 0), vec![0xAA]).expect("Add required");
        manager.add_optional_plugin(2002, (2, 1), vec![0xBB, 0xCC]).expect("Add optional");
        
        let settings_data = manager.generate_settings_frame_data().expect("Generate settings");
        assert!(!settings_data.is_empty());
        
        let parsed_requirements = manager.parse_peer_settings_data(&settings_data).expect("Parse settings");
        assert_eq!(parsed_requirements.len(), 2);
        
        // Verify first requirement (required plugin)
        assert_eq!(parsed_requirements[0].plugin_id, 1001);
        assert_eq!(parsed_requirements[0].min_version, (1, 0));
        assert_eq!(parsed_requirements[0].capability, PluginCapability::Required);
        assert_eq!(parsed_requirements[0].config_params, vec![0xAA]);
        
        // Verify second requirement (optional plugin)
        assert_eq!(parsed_requirements[1].plugin_id, 2002);
        assert_eq!(parsed_requirements[1].min_version, (2, 1));
        assert_eq!(parsed_requirements[1].capability, PluginCapability::Optional);
        assert_eq!(parsed_requirements[1].config_params, vec![0xBB, 0xCC]);
    }

    #[test]
    fn test_peer_requirement_validation_success() {
        let mut manager = PluginSettingsManager::new();
        manager.add_required_plugin(100, (1, 0), vec![]).expect("Add plugin");
        
        let peer_requirements = vec![
            PluginRequirement {
                plugin_id: 100,
                min_version: (1, 0),
                capability: PluginCapability::Required,
                config_params: vec![],
            }
        ];
        
        let result = manager.validate_peer_requirements(&peer_requirements);
        assert!(result.is_ok());
    }

    #[test]
    fn test_peer_requirement_validation_failure() {
        let manager = PluginSettingsManager::new(); // No plugins registered
        
        let peer_requirements = vec![
            PluginRequirement {
                plugin_id: 999,
                min_version: (1, 0),
                capability: PluginCapability::Required,
                config_params: vec![],
            }
        ];
        
        let result = manager.validate_peer_requirements(&peer_requirements);
        assert!(matches!(result, Err(PluginSettingsError::UnsupportedRequiredPlugin(999))));
    }

    #[test]
    fn test_malformed_settings_data_rejection() {
        let manager = PluginSettingsManager::new();
        
        // Test data too short
        let result = manager.parse_peer_settings_data(&[0x00]);
        assert!(matches!(result, Err(PluginSettingsError::MalformedData(_))));
        
        // Test truncated data  
        let truncated_data = vec![0x00, 0x01, 0x00]; // Claims 1 requirement but insufficient data
        let result = manager.parse_peer_settings_data(&truncated_data);
        assert!(matches!(result, Err(PluginSettingsError::MalformedData(_))));
    }

    #[test]
    fn test_too_many_required_plugins() {
        let mut manager = PluginSettingsManager::new();
        
        // Add maximum allowed required plugins
        for i in 0..MAX_REQUIRED_PLUGINS {
            manager.add_required_plugin(i as u32, (1, 0), vec![]).expect("Add within limit");
        }
        
        // Try to add one more - should fail
        let result = manager.add_required_plugin(MAX_REQUIRED_PLUGINS as u32, (1, 0), vec![]);
        assert!(matches!(result, Err(PluginSettingsError::TooManyRequiredPlugins { .. })));
    }

    #[test]
    fn test_negotiation_toggle() {
        let mut manager = PluginSettingsManager::new();
        assert!(manager.is_negotiation_enabled());
        
        manager.set_negotiation_enabled(false);
        assert!(!manager.is_negotiation_enabled());
        
        // When disabled, should generate empty settings data
        let settings_data = manager.generate_settings_frame_data().expect("Generate when disabled");
        assert!(settings_data.is_empty());
    }
}
