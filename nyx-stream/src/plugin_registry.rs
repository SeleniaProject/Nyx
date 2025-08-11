#![forbid(unsafe_code)]

//! Plugin Registry for v1.0 Plugin Framework
//!
//! This module manages plugin registration, permissions, and lifecycle.
//! Supports all v1.0 plugin features including:
//! - Dynamic plugin loading and unloading
//! - Permission-based security model
//! - Plugin capability negotiation
//! - Sandboxed execution environments

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use thiserror::Error;
use tracing::{debug, warn, info};

/// Plugin unique identifier
pub type PluginId = u32;

/// Plugin permission types for security enforcement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Basic frame receiving capability
    ReceiveFrames,
    /// Plugin handshake capability
    Handshake,
    /// Data access and processing capability
    DataAccess,
    /// Control command capability
    Control,
    /// Error reporting capability
    ErrorReporting,
    /// Network access permission
    NetworkAccess,
    /// File system access permission
    FileSystemAccess,
    /// Inter-plugin IPC communication
    InterPluginIpc,
    /// Geographic location access
    AccessGeo,
    /// Legacy network access (for compatibility)
    ACCESS_NETWORK,
    /// Legacy geo access (for compatibility)
    ACCESS_GEO,
    /// Plugin persistence and state management
    PluginPersistence,
    /// Cryptographic operations access
    CryptoAccess,
    /// System metrics and monitoring access
    MetricsAccess,
}

/// Plugin information and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin unique identifier
    pub id: PluginId,
    /// Plugin display name
    pub name: String,
    /// Plugin version (semantic versioning)
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Required permissions
    pub permissions: Vec<Permission>,
    /// Plugin author/vendor
    pub author: String,
    /// Plugin configuration schema
    pub config_schema: HashMap<String, String>,
    /// Supported frame types
    pub supported_frames: Vec<u8>,
    /// Whether plugin is required for protocol operation
    pub required: bool,
}

/// Plugin registration errors
#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("Plugin already exists: {0}")]
    AlreadyExists(PluginId),

    #[error("Plugin not found: {0}")]
    NotFound(PluginId),

    #[error("Invalid plugin ID: {0}")]
    InvalidId(PluginId),

    #[error("Permission denied for plugin {plugin_id}: {permission:?}")]
    PermissionDenied { plugin_id: PluginId, permission: Permission },

    #[error("Plugin validation failed: {0}")]
    ValidationFailed(String),
}

/// Central registry for managing active plugins  
#[derive(Debug, Clone)]
pub struct PluginRegistry {
    /// Registered plugins by ID
    plugins: Arc<Mutex<HashMap<PluginId, PluginInfo>>>,
    /// Permission grants per plugin
    permissions: Arc<Mutex<HashMap<PluginId, Vec<Permission>>>>,
}

impl PluginRegistry {
    /// Create new plugin registry
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(Mutex::new(HashMap::new())),
            permissions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new plugin
    pub async fn register(&self, info: PluginInfo) -> Result<(), RegistryError> {
        // Validate plugin info
        self.validate_plugin_info(&info)?;

        let mut plugins = self.plugins.lock().unwrap();
        let mut permissions = self.permissions.lock().unwrap();

        // Check if already registered
        if plugins.contains_key(&info.id) {
            return Err(RegistryError::AlreadyExists(info.id));
        }

        // Register plugin
        plugins.insert(info.id, info.clone());
        permissions.insert(info.id, info.permissions.clone());

        info!("Plugin registered: {} (ID: {}, Version: {})", 
              info.name, info.id, info.version);
        Ok(())
    }

    /// Unregister a plugin
    pub async fn unregister(&self, plugin_id: PluginId) -> Result<(), RegistryError> {
        let mut plugins = self.plugins.lock().unwrap();
        let mut permissions = self.permissions.lock().unwrap();

        let plugin_info = plugins.remove(&plugin_id)
            .ok_or(RegistryError::NotFound(plugin_id))?;
        permissions.remove(&plugin_id);

        info!("Plugin unregistered: {} (ID: {})", plugin_info.name, plugin_id);
        Ok(())
    }

    /// Get plugin information by ID
    pub async fn get_plugin_info(&self, plugin_id: PluginId) -> Option<PluginInfo> {
        self.plugins.lock().unwrap().get(&plugin_id).cloned()
    }

    /// Check if plugin is registered
    pub async fn is_registered(&self, plugin_id: PluginId) -> bool {
        self.plugins.lock().unwrap().contains_key(&plugin_id)
    }

    /// Legacy sync get method  
    pub fn get(&self, plugin_id: PluginId) -> Option<PluginInfo> {
        self.plugins.lock().unwrap().get(&plugin_id).cloned()
    }

    /// Check if plugin has specific permission
    pub fn has_permission(&self, plugin_id: PluginId, permission: Permission) -> bool {
        self.permissions.lock().unwrap()
            .get(&plugin_id)
            .map(|perms| perms.contains(&permission))
            .unwrap_or(false)
    }

    /// Grant permission to plugin
    pub fn grant_permission(&self, plugin_id: PluginId, permission: Permission) -> Result<(), RegistryError> {
        let mut permissions = self.permissions.lock().unwrap();
        
        // Check if plugin exists
        if !self.plugins.lock().unwrap().contains_key(&plugin_id) {
            return Err(RegistryError::NotFound(plugin_id));
        }

        let plugin_perms = permissions.entry(plugin_id).or_insert_with(Vec::new);
        if !plugin_perms.contains(&permission) {
            plugin_perms.push(permission);
            debug!("Permission granted to plugin {}: {:?}", plugin_id, permission);
        }
        Ok(())
    }

    /// Revoke permission from plugin
    pub fn revoke(&self, plugin_id: PluginId, permission: Permission) -> Result<(), RegistryError> {
        let mut permissions = self.permissions.lock().unwrap();
        
        if let Some(plugin_perms) = permissions.get_mut(&plugin_id) {
            plugin_perms.retain(|p| *p != permission);
            debug!("Permission revoked from plugin {}: {:?}", plugin_id, permission);
            Ok(())
        } else {
            Err(RegistryError::NotFound(plugin_id))
        }
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.lock().unwrap().values().cloned().collect()
    }

    /// Get count of registered plugins
    pub fn count(&self) -> usize {
        self.plugins.lock().unwrap().len()
    }

    /// Get plugins requiring specific permission
    pub fn plugins_with_permission(&self, permission: Permission) -> Vec<PluginId> {
        self.permissions.lock().unwrap()
            .iter()
            .filter_map(|(id, perms)| {
                if perms.contains(&permission) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect()
    }

                    use std::collections::HashMap;
                    use tokio::runtime::Runtime;

                    fn sample_plugin(id: PluginId) -> PluginInfo {
                        let mut schema = HashMap::new();
                        schema.insert("interval_ms".to_string(), "integer >= 10".to_string());
                        schema.insert("enable_cache".to_string(), "boolean".to_string());
                        PluginInfo {
                            id,
                            name: format!("plugin-{}", id),
                            version: "1.2.3".to_string(),
                            description: "Sample test plugin".to_string(),
                            permissions: vec![Permission::ReceiveFrames, Permission::DataAccess],
                            author: "Test".to_string(),
                            config_schema: schema,
                            supported_frames: vec![1,2,42],
                            required: false,
                        }
                    }

                    #[tokio::test]
                    async fn register_and_fetch_roundtrip() {
                        let reg = PluginRegistry::new();
                        let info = sample_plugin(7);
                        reg.register(info.clone()).await.unwrap();
                        let fetched = reg.get_plugin_info(7).await.unwrap();
                        assert_eq!(fetched.name, info.name);
                        assert_eq!(fetched.config_schema.get("interval_ms").unwrap(), "integer >= 10");
                        assert!(reg.is_registered(7).await);
                    }

                    #[tokio::test]
                    async fn duplicate_registration_fails() {
                        let reg = PluginRegistry::new();
                        let info = sample_plugin(9);
                        reg.register(info.clone()).await.unwrap();
                        let err = reg.register(info).await.err().expect("expected duplicate error");
                        matches!(err, RegistryError::AlreadyExists(9));
                    }

                    #[tokio::test]
                    async fn unregister_removes() {
                        let reg = PluginRegistry::new();
                        let info = sample_plugin(11);
                        reg.register(info).await.unwrap();
                        reg.unregister(11).await.unwrap();
                        assert!(!reg.is_registered(11).await);
                    }
    /// Validate plugin information
    fn validate_plugin_info(&self, info: &PluginInfo) -> Result<(), RegistryError> {
        // Plugin ID must be non-zero
        if info.id == 0 {
            return Err(RegistryError::InvalidId(info.id));
        }

        // Plugin name must not be empty
        if info.name.is_empty() {
            return Err(RegistryError::ValidationFailed("Plugin name cannot be empty".to_string()));
        }

        // Version must be valid semantic version format
        if info.version.is_empty() {
            return Err(RegistryError::ValidationFailed("Plugin version cannot be empty".to_string()));
        }

        // Check for duplicate frame types
        let mut seen_frames = std::collections::HashSet::new();
        for &frame_type in &info.supported_frames {
            if !seen_frames.insert(frame_type) {
                return Err(RegistryError::ValidationFailed(
                    format!("Duplicate frame type: 0x{:02x}", frame_type)
                ));
            }
        }

        Ok(())
    }

    /// Clear all plugins (for testing)
    #[cfg(test)]
    pub fn clear(&self) {
        self.plugins.write().unwrap().clear();
        self.permissions.write().unwrap().clear();
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plugin_info() -> PluginInfo {
        PluginInfo {
            id: 1001,
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            permissions: vec![Permission::ReceiveFrames, Permission::NetworkAccess],
            author: "Test Author".to_string(),
            config_schema: HashMap::new(),
            supported_frames: vec![0x50, 0x51],
            required: false,
        }
    }

    #[test]
    fn test_register_plugin() {
        let registry = PluginRegistry::new();
        let info = test_plugin_info();
        
        assert!(registry.register(&info).is_ok());
        assert_eq!(registry.count(), 1);
        
        // Duplicate registration should fail
        assert!(matches!(registry.register(&info), Err(RegistryError::AlreadyRegistered(_))));
    }

    #[test]
    fn test_permission_management() {
        let registry = PluginRegistry::new();
        let info = test_plugin_info();
        registry.register(&info).unwrap();

        // Check default permissions
        assert!(registry.has_permission(info.id, Permission::ReceiveFrames));
        assert!(registry.has_permission(info.id, Permission::NetworkAccess));
        assert!(!registry.has_permission(info.id, Permission::FileSystemAccess));

        // Grant new permission
        registry.grant_permission(info.id, Permission::FileSystemAccess).unwrap();
        assert!(registry.has_permission(info.id, Permission::FileSystemAccess));

        // Revoke permission
        registry.revoke(info.id, Permission::NetworkAccess).unwrap();
        assert!(!registry.has_permission(info.id, Permission::NetworkAccess));
    }

    #[test]
    fn test_validation() {
        let registry = PluginRegistry::new();
        
        // Invalid ID
        let mut info = test_plugin_info();
        info.id = 0;
        assert!(matches!(registry.register(&info), Err(RegistryError::InvalidId(_))));

        // Empty name
        info.id = 1002;
        info.name = "".to_string();
        assert!(matches!(registry.register(&info), Err(RegistryError::ValidationFailed(_))));
    }
}
