//! Protocol Combinator (Plugin Framework)
//!
//! This module implements the dynamic plugin framework for Nyx Protocol v1.0,
//! enabling runtime extension of protocol capabilities through standardized
//! plugin interfaces, CBOR-based communication, and capability negotiation.
//!
//! Specification Compliance:
//! - Frame Type 0x50–0x5F reserved for Plugin frames
//! - CBOR header format: {id:u32, flags:u8, data:bytes}
//! - Plugin handshake via SETTINGS PLUGIN_REQUIRED advertisement
//! - Backward compatibility with v0.1 implementations

use serde::{Deserialize, Serialize};
use serde_cbor;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

// Re-export for convenience
pub use crate::errors::Error;
pub use crate::frame::{Frame, FrameHeader, FrameType};

/// Plugin framework error types
#[derive(Error, Debug, Clone)]
pub enum PluginError {
    #[error("Plugin not found: {id}")]
    PluginNotFound { id: u32 },

    #[error("Plugin incompatible: {id}, version: {version}")]
    IncompatiblePlugin { id: u32, version: String },

    #[error("Plugin registration failed: {reason}")]
    RegistrationFailed { reason: String },

    #[error("CBOR serialization error: {0}")]
    SerializationError(String),

    #[error("Plugin communication error: {0}")]
    CommunicationError(String),

    #[error("Plugin capability mismatch: required={required}, available={available}")]
    CapabilityMismatch { required: String, available: String },

    #[error("Plugin lifecycle error: {phase}: {reason}")]
    LifecycleError { phase: String, reason: String },

    #[error("Plugin security violation: {0}")]
    SecurityViolation(String),

    #[error("Plugin timeout: {operation}")]
    Timeout { operation: String },
}

type PluginResult<T> = Result<T, PluginError>;

/// Plugin frame types (0x50-0x5F range)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PluginFrameType {
    /// Plugin registration request
    Register = 0x50,
    /// Plugin registration response
    RegisterResponse = 0x51,
    /// Plugin data transfer
    Data = 0x52,
    /// Plugin control message
    Control = 0x53,
    /// Plugin capability advertisement
    Capability = 0x54,
    /// Plugin heartbeat/keepalive
    Heartbeat = 0x55,
    /// Plugin error notification
    Error = 0x56,
    /// Plugin shutdown notification
    Shutdown = 0x57,
    // 0x58-0x5F reserved for future extension
}

impl From<PluginFrameType> for FrameType {
    fn from(plugin_type: PluginFrameType) -> Self {
        FrameType::Custom(plugin_type as u8)
    }
}

/// CBOR plugin header format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHeader {
    /// Plugin identifier (unique per plugin type)
    pub id: u32,
    /// Plugin-specific flags
    pub flags: u8,
    /// Plugin data payload
    pub data: Vec<u8>,
}

/// Plugin capability description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapability {
    /// Capability name/identifier
    pub name: String,
    /// Capability version
    pub version: String,
    /// Whether this capability is required
    pub required: bool,
    /// Optional capability parameters
    pub parameters: HashMap<String, serde_cbor::Value>,
}

/// Plugin metadata and registration information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin unique identifier
    pub id: u32,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin author/organization
    pub author: String,
    /// Plugin description
    pub description: String,
    /// Required capabilities
    pub capabilities: Vec<PluginCapability>,
    /// Minimum protocol version
    pub min_protocol_version: String,
    /// Plugin load priority (higher = first)
    pub priority: i32,
    /// Plugin configuration schema
    pub config_schema: Option<serde_cbor::Value>,
}

/// Plugin instance lifecycle state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Plugin is unloaded
    Unloaded,
    /// Plugin is loading
    Loading,
    /// Plugin is ready for use
    Ready,
    /// Plugin is actively processing
    Active,
    /// Plugin is paused
    Paused,
    /// Plugin encountered an error
    Error(String),
    /// Plugin is shutting down
    ShuttingDown,
}

/// Plugin communication interface
#[async_trait::async_trait]
pub trait Plugin: Send + Sync {
    /// Get plugin metadata
    fn metadata(&self) -> &PluginMetadata;

    /// Initialize the plugin with configuration
    async fn initialize(&mut self, config: serde_cbor::Value) -> PluginResult<()>;

    /// Process incoming frame data
    async fn process_frame(
        &mut self,
        header: &PluginHeader,
        frame: &Frame,
    ) -> PluginResult<Vec<Frame>>;

    /// Handle control messages
    async fn handle_control(
        &mut self,
        message: serde_cbor::Value,
    ) -> PluginResult<serde_cbor::Value>;

    /// Periodic heartbeat/maintenance
    async fn heartbeat(&mut self) -> PluginResult<()>;

    /// Shutdown the plugin gracefully
    async fn shutdown(&mut self) -> PluginResult<()>;

    /// Get current plugin state
    fn state(&self) -> PluginState;

    /// Get plugin statistics
    fn statistics(&self) -> HashMap<String, u64>;
}

/// Plugin instance wrapper
pub struct PluginInstance {
    /// Plugin implementation
    plugin: Box<dyn Plugin>,
    /// Plugin state
    state: PluginState,
    /// Registration timestamp
    registered_at: Instant,
    /// Last activity timestamp
    last_activity: Instant,
    /// Frame processing statistics
    frames_processed: u64,
    /// Error count
    error_count: u64,
    /// Configuration
    config: Option<serde_cbor::Value>,
}

impl PluginInstance {
    pub fn new(plugin: Box<dyn Plugin>) -> Self {
        let now = Instant::now();
        Self {
            plugin,
            state: PluginState::Unloaded,
            registered_at: now,
            last_activity: now,
            frames_processed: 0,
            error_count: 0,
            config: None,
        }
    }

    pub fn metadata(&self) -> &PluginMetadata {
        self.plugin.metadata()
    }

    pub fn state(&self) -> &PluginState {
        &self.state
    }

    pub fn statistics(&self) -> HashMap<String, u64> {
        let mut stats = self.plugin.statistics();
        stats.insert("frames_processed".to_string(), self.frames_processed);
        stats.insert("error_count".to_string(), self.error_count);
        stats.insert(
            "uptime_seconds".to_string(),
            self.registered_at.elapsed().as_secs(),
        );
        stats
    }

    pub async fn initialize(&mut self, config: serde_cbor::Value) -> PluginResult<()> {
        self.state = PluginState::Loading;

        match self.plugin.initialize(config.clone()).await {
            Ok(()) => {
                self.state = PluginState::Ready;
                self.config = Some(config);
                info!("Plugin {} initialized successfully", self.metadata().name);
                Ok(())
            }
            Err(e) => {
                self.state = PluginState::Error(e.to_string());
                self.error_count += 1;
                error!(
                    "Plugin {} initialization failed: {}",
                    self.metadata().name,
                    e
                );
                Err(e)
            }
        }
    }

    pub async fn process_frame(
        &mut self,
        header: &PluginHeader,
        frame: &Frame,
    ) -> PluginResult<Vec<Frame>> {
        if !matches!(self.state, PluginState::Ready | PluginState::Active) {
            return Err(PluginError::LifecycleError {
                phase: "process_frame".to_string(),
                reason: format!("Invalid state: {:?}", self.state),
            });
        }

        self.state = PluginState::Active;
        self.last_activity = Instant::now();

        match self.plugin.process_frame(header, frame).await {
            Ok(result) => {
                self.frames_processed += 1;
                self.state = PluginState::Ready;
                Ok(result)
            }
            Err(e) => {
                self.error_count += 1;
                self.state = PluginState::Error(e.to_string());
                error!(
                    "Plugin {} frame processing failed: {}",
                    self.metadata().name,
                    e
                );
                Err(e)
            }
        }
    }
}

/// Plugin Manager - coordinates plugin lifecycle and communication
pub struct PluginManager {
    /// Registered plugin instances
    plugins: RwLock<HashMap<u32, PluginInstance>>,
    /// Available capabilities
    capabilities: RwLock<HashMap<String, PluginCapability>>,
    /// Required capabilities for negotiation
    required_capabilities: RwLock<HashSet<String>>,
    /// Plugin communication channels
    channels: RwLock<HashMap<u32, mpsc::Sender<PluginMessage>>>,
    /// Plugin load order (by priority)
    load_order: RwLock<Vec<u32>>,
    /// Configuration
    config: PluginManagerConfig,
}

/// Plugin manager configuration
#[derive(Debug, Clone)]
pub struct PluginManagerConfig {
    /// Maximum number of plugins
    pub max_plugins: usize,
    /// Plugin timeout for operations
    pub plugin_timeout: Duration,
    /// Heartbeat interval
    pub heartbeat_interval: Duration,
    /// Enable plugin isolation
    pub enable_isolation: bool,
    /// Maximum frame size for plugins
    pub max_frame_size: usize,
}

impl Default for PluginManagerConfig {
    fn default() -> Self {
        Self {
            max_plugins: 32,
            plugin_timeout: Duration::from_secs(30),
            heartbeat_interval: Duration::from_secs(60),
            enable_isolation: true,
            max_frame_size: 65536,
        }
    }
}

/// Plugin communication message types
#[derive(Debug, Clone)]
pub enum PluginMessage {
    Frame { header: PluginHeader, frame: Frame },
    Control { message: serde_cbor::Value },
    Heartbeat,
    Shutdown,
}

impl PluginManager {
    /// Create new plugin manager
    pub fn new(config: PluginManagerConfig) -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
            capabilities: RwLock::new(HashMap::new()),
            required_capabilities: RwLock::new(HashSet::new()),
            channels: RwLock::new(HashMap::new()),
            load_order: RwLock::new(Vec::new()),
            config,
        }
    }

    /// Register a new plugin
    pub async fn register_plugin(&self, plugin: Box<dyn Plugin>) -> PluginResult<u32> {
        let metadata = plugin.metadata().clone();
        let plugin_id = metadata.id;

        // Check if plugin already exists
        {
            let plugins = self
                .plugins
                .read()
                .map_err(|_| PluginError::RegistrationFailed {
                    reason: "Plugin registry lock poisoned".to_string(),
                })?;
            if plugins.contains_key(&plugin_id) {
                return Err(PluginError::RegistrationFailed {
                    reason: format!("Plugin {plugin_id} already registered"),
                });
            }
        }

        // Check plugin limits
        {
            let plugins = self
                .plugins
                .read()
                .map_err(|_| PluginError::RegistrationFailed {
                    reason: "Plugin registry lock poisoned".to_string(),
                })?;
            if plugins.len() >= self.config.max_plugins {
                return Err(PluginError::RegistrationFailed {
                    reason: "Maximum plugin limit reached".to_string(),
                });
            }
        }

        // Validate plugin capabilities
        self.validate_plugin_capabilities(&metadata.capabilities)?;

        // Create plugin instance
        let mut instance = PluginInstance::new(plugin);

        // Initialize with default configuration
        let default_config = serde_cbor::Value::Map(BTreeMap::new());
        instance.initialize(default_config).await?;

        // Register capabilities
        {
            let mut capabilities =
                self.capabilities
                    .write()
                    .map_err(|_| PluginError::RegistrationFailed {
                        reason: "Capabilities registry lock poisoned".to_string(),
                    })?;
            for cap in &metadata.capabilities {
                capabilities.insert(cap.name.clone(), cap.clone());
            }
        }

        // Insert plugin with proper ordering
        {
            let mut plugins =
                self.plugins
                    .write()
                    .map_err(|_| PluginError::RegistrationFailed {
                        reason: "Plugin registry lock poisoned".to_string(),
                    })?;
            let mut load_order =
                self.load_order
                    .write()
                    .map_err(|_| PluginError::RegistrationFailed {
                        reason: "Load order lock poisoned".to_string(),
                    })?;

            plugins.insert(plugin_id, instance);

            // Insert in priority order (higher priority first)
            let insert_pos = load_order
                .iter()
                .position(|&id| {
                    plugins
                        .get(&id)
                        .is_some_and(|p| p.metadata().priority < metadata.priority)
                })
                .unwrap_or(load_order.len());

            load_order.insert(insert_pos, plugin_id);
        }

        info!(
            "Plugin {} (v{}) registered successfully with priority {}",
            metadata.name, metadata.version, metadata.priority
        );

        Ok(plugin_id)
    }

    /// Unregister a plugin
    pub async fn unregister_plugin(&self, plugin_id: u32) -> PluginResult<()> {
        // Shutdown plugin first (avoid holding write lock across await)
        let mut maybe_instance = {
            if self.plugins.read().unwrap().contains_key(&plugin_id) {
                let mut plugins = self.plugins.write().unwrap();
                plugins.remove(&plugin_id)
            } else {
                None
            }
        };
        if let Some(instance) = maybe_instance.as_mut() {
            instance.plugin.shutdown().await?;
        }
        // Re-insert to proceed with normal removal below
        if let Some(instance) = maybe_instance.take() {
            self.plugins.write().unwrap().insert(plugin_id, instance);
        }

        // Remove from collections
        {
            let mut plugins = self.plugins.write().unwrap();
            let mut load_order = self.load_order.write().unwrap();
            let mut channels = self.channels.write().unwrap();

            plugins.remove(&plugin_id);
            load_order.retain(|&id| id != plugin_id);
            channels.remove(&plugin_id);
        }

        info!("Plugin {} unregistered", plugin_id);
        Ok(())
    }

    /// Process a plugin frame
    pub async fn process_plugin_frame(&self, frame: &Frame) -> PluginResult<Vec<Frame>> {
        // Decode plugin header from frame payload
        let header: PluginHeader = serde_cbor::from_slice(&frame.payload)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;

        let plugin_id = header.id;

        // Find and process with appropriate plugin
        // Avoid holding write lock during await
        if !self.plugins.read().unwrap().contains_key(&plugin_id) {
            return Err(PluginError::PluginNotFound { id: plugin_id });
        }
        // Take instance out, process, then put back
        let mut instance = {
            let mut plugins = self.plugins.write().unwrap();
            plugins.remove(&plugin_id).expect("plugin must exist")
        };
        let res = instance.process_frame(&header, frame).await;
        {
            let mut plugins = self.plugins.write().unwrap();
            plugins.insert(plugin_id, instance);
        }
        res
    }

    /// Create plugin frame
    pub fn create_plugin_frame(
        &self,
        stream_id: u32,
        seq: u64,
        plugin_type: PluginFrameType,
        header: &PluginHeader,
    ) -> PluginResult<Frame> {
        let payload = serde_cbor::to_vec(header)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;

        if payload.len() > self.config.max_frame_size {
            return Err(PluginError::CommunicationError(
                "Plugin frame exceeds maximum size".to_string(),
            ));
        }

        Ok(Frame {
            header: FrameHeader {
                stream_id,
                seq,
                ty: FrameType::Custom(plugin_type as u8),
            },
            payload,
        })
    }

    /// Get available capabilities for negotiation
    pub fn get_capabilities(&self) -> Vec<PluginCapability> {
        self.capabilities
            .read()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    /// Set required capabilities
    pub fn set_required_capabilities(&self, capabilities: Vec<String>) {
        let mut required = self.required_capabilities.write().unwrap();
        required.clear();
        required.extend(capabilities);
    }

    /// Check if all required capabilities are available
    pub fn check_capability_compatibility(&self) -> bool {
        let required = self.required_capabilities.read().unwrap();
        let available = self.capabilities.read().unwrap();

        required.iter().all(|cap| available.contains_key(cap))
    }

    /// Get plugin statistics
    pub fn get_plugin_statistics(&self) -> HashMap<u32, HashMap<String, u64>> {
        let plugins = self.plugins.read().unwrap();
        plugins
            .iter()
            .map(|(&id, instance)| (id, instance.statistics()))
            .collect()
    }

    /// Perform plugin heartbeat
    pub async fn heartbeat(&self) -> PluginResult<()> {
        let plugin_ids: Vec<u32> = { self.plugins.read().unwrap().keys().copied().collect() };

        for plugin_id in plugin_ids {
            // Take the instance out to avoid holding lock across await
            let maybe_instance = self.plugins.write().unwrap().remove(&plugin_id);
            if let Some(mut instance) = maybe_instance {
                if let Err(e) = instance.plugin.heartbeat().await {
                    warn!("Plugin {} heartbeat failed: {}", plugin_id, e);
                    instance.error_count += 1;
                }
                // Put it back
                self.plugins.write().unwrap().insert(plugin_id, instance);
            }
        }

        Ok(())
    }

    // Helper methods

    fn validate_plugin_capabilities(&self, capabilities: &[PluginCapability]) -> PluginResult<()> {
        for cap in capabilities {
            if cap.name.is_empty() {
                return Err(PluginError::RegistrationFailed {
                    reason: "Empty capability name".to_string(),
                });
            }

            if cap.version.is_empty() {
                return Err(PluginError::RegistrationFailed {
                    reason: format!("Empty version for capability {}", cap.name),
                });
            }
        }

        Ok(())
    }
}

/// Plugin capability negotiation support
pub struct CapabilityNegotiator {
    manager: Arc<PluginManager>,
}

impl CapabilityNegotiator {
    pub fn new(manager: Arc<PluginManager>) -> Self {
        Self { manager }
    }

    /// Create capability advertisement frame
    pub fn create_capability_frame(&self, stream_id: u32, seq: u64) -> PluginResult<Frame> {
        let capabilities = self.manager.get_capabilities();
        let data = serde_cbor::to_vec(&capabilities)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;

        let header = PluginHeader {
            id: 0, // Reserved for system messages
            flags: 0,
            data,
        };

        self.manager
            .create_plugin_frame(stream_id, seq, PluginFrameType::Capability, &header)
    }

    /// Process received capability advertisement
    pub fn process_capability_frame(&self, frame: &Frame) -> PluginResult<Vec<PluginCapability>> {
        let header: PluginHeader = serde_cbor::from_slice(&frame.payload)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;

        let capabilities: Vec<PluginCapability> = serde_cbor::from_slice(&header.data)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;

        Ok(capabilities)
    }

    /// Negotiate compatibility with peer capabilities
    pub fn negotiate_compatibility(
        &self,
        peer_capabilities: &[PluginCapability],
    ) -> PluginResult<bool> {
        let our_capabilities = self.manager.get_capabilities();
        let required = self.manager.required_capabilities.read().unwrap();

        // Check if all required capabilities are supported by peer
        for required_cap in required.iter() {
            let is_supported = peer_capabilities
                .iter()
                .any(|cap| cap.name == *required_cap);

            if !is_supported {
                return Err(PluginError::CapabilityMismatch {
                    required: required_cap.clone(),
                    available: peer_capabilities
                        .iter()
                        .map(|c| c.name.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
        }

        // Check if we can provide peer's required capabilities
        for peer_cap in peer_capabilities.iter().filter(|c| c.required) {
            let can_provide = our_capabilities.iter().any(|cap| cap.name == peer_cap.name);

            if !can_provide {
                return Err(PluginError::CapabilityMismatch {
                    required: peer_cap.name.clone(),
                    available: our_capabilities
                        .iter()
                        .map(|c| c.name.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
        }

        Ok(true)
    }
}

/// Example compression plugin implementation
#[derive(Debug)]
pub struct CompressionPlugin {
    metadata: PluginMetadata,
    state: PluginState,
    stats: HashMap<String, u64>,
}

impl CompressionPlugin {
    pub fn new() -> Self {
        let metadata = PluginMetadata {
            id: 0x10000001, // Example plugin ID
            name: "LZ4Compression".to_string(),
            version: "1.0.0".to_string(),
            author: "Nyx Project".to_string(),
            description: "LZ4 compression plugin for Nyx Protocol".to_string(),
            capabilities: vec![PluginCapability {
                name: "compression.lz4".to_string(),
                version: "1.0".to_string(),
                required: false,
                parameters: HashMap::new(),
            }],
            min_protocol_version: "1.0.0".to_string(),
            priority: 100,
            config_schema: None,
        };

        Self {
            metadata,
            state: PluginState::Unloaded,
            stats: HashMap::new(),
        }
    }
}

impl Default for CompressionPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Plugin for CompressionPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, _config: serde_cbor::Value) -> PluginResult<()> {
        self.state = PluginState::Ready;
        info!("LZ4 Compression plugin initialized");
        Ok(())
    }

    async fn process_frame(
        &mut self,
        _header: &PluginHeader,
        frame: &Frame,
    ) -> PluginResult<Vec<Frame>> {
        // Example: compress frame payload
        let compressed_payload = frame.payload.clone(); // Placeholder for actual compression

        let compressed_frame = Frame {
            header: frame.header.clone(),
            payload: compressed_payload,
        };

        // Update statistics
        *self
            .stats
            .entry("frames_compressed".to_string())
            .or_insert(0) += 1;

        Ok(vec![compressed_frame])
    }

    async fn handle_control(
        &mut self,
        _message: serde_cbor::Value,
    ) -> PluginResult<serde_cbor::Value> {
        // Handle plugin-specific control messages
        Ok(serde_cbor::Value::Text("OK".to_string()))
    }

    async fn heartbeat(&mut self) -> PluginResult<()> {
        // Plugin is alive
        Ok(())
    }

    async fn shutdown(&mut self) -> PluginResult<()> {
        self.state = PluginState::ShuttingDown;
        info!("LZ4 Compression plugin shutting down");
        Ok(())
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    fn statistics(&self) -> HashMap<String, u64> {
        self.stats.clone()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use tokio::test;

    #[test]
    async fn test_plugin_manager_basic() {
        let config = PluginManagerConfig::default();
        let manager = PluginManager::new(config);

        // Register compression plugin
        let plugin = Box::new(CompressionPlugin::new());
        let plugin_id = manager.register_plugin(plugin).await.unwrap();

        assert_eq!(plugin_id, 0x10000001);

        // Check capabilities
        let capabilities = manager.get_capabilities();
        assert_eq!(capabilities.len(), 1);
        assert_eq!(capabilities[0].name, "compression.lz4");

        // Unregister plugin
        manager.unregister_plugin(plugin_id).await.unwrap();
    }

    #[test]
    async fn test_capability_negotiation() {
        let config = PluginManagerConfig::default();
        let manager = Arc::new(PluginManager::new(config));

        // Register plugin
        let plugin = Box::new(CompressionPlugin::new());
        manager.register_plugin(plugin).await.unwrap();

        let negotiator = CapabilityNegotiator::new(manager);

        // Test compatible capabilities
        let peer_caps = vec![PluginCapability {
            name: "compression.lz4".to_string(),
            version: "1.0".to_string(),
            required: false,
            parameters: HashMap::new(),
        }];

        assert!(negotiator.negotiate_compatibility(&peer_caps).unwrap());
    }

    #[test]
    async fn test_plugin_frame_processing() {
        let config = PluginManagerConfig::default();
        let manager = PluginManager::new(config);

        // Register plugin
        let plugin = Box::new(CompressionPlugin::new());
        let plugin_id = manager.register_plugin(plugin).await.unwrap();

        // Create test frame
        let header = PluginHeader {
            id: plugin_id,
            flags: 0,
            data: b"test data".to_vec(),
        };

        let frame = manager
            .create_plugin_frame(1, 1, PluginFrameType::Data, &header)
            .unwrap();

        // Process frame
        let result = manager.process_plugin_frame(&frame).await.unwrap();
        assert_eq!(result.len(), 1);
    }
}
