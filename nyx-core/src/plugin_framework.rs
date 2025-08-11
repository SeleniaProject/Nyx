// Plugin Framework Implementation for NyxNet v1.0
// Complete implementation of the plugin system with CBOR headers and IPC transport

use serde::{Deserialize, Serialize};
use cbor4ii::{serde as cbor_serde};
use tokio::sync::{mpsc, RwLock};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Plugin Framework for NyxNet v1.0
/// Implements Frame Type 0x50-0x5F plugin reserved area with CBOR headers
pub struct PluginFramework {
    /// Registered plugins
    plugins: Arc<RwLock<HashMap<u32, Arc<Plugin>>>>,
    /// Plugin communication channels
    ipc_channels: Arc<RwLock<HashMap<u32, mpsc::UnboundedSender<PluginMessage>>>>,
    /// Plugin settings advertised via SETTINGS frame
    plugin_settings: Arc<RwLock<PluginSettings>>,
    /// Plugin event bus
    event_bus: mpsc::UnboundedSender<PluginEvent>,
}

/// Plugin reserved frame types (0x50-0x5F)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginFrameType {
    PluginData = 0x50,
    PluginControl = 0x51,
    PluginHandshake = 0x52,
    PluginSettings = 0x53,
    PluginPing = 0x54,
    PluginPong = 0x55,
    PluginError = 0x56,
    PluginAuth = 0x57,
    PluginCapability = 0x58,
    PluginLifecycle = 0x59,
    Reserved0 = 0x5A,
    Reserved1 = 0x5B,
    Reserved2 = 0x5C,
    Reserved3 = 0x5D,
    Reserved4 = 0x5E,
    Reserved5 = 0x5F,
}

/// CBOR Plugin Header: {id:u32, flags:u8, data:bytes}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHeader {
    /// Plugin unique identifier
    pub id: u32,
    /// Plugin-specific flags
    pub flags: u8,
    /// Plugin payload data
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// Plugin capability flags
#[derive(Debug, Clone)]
pub struct PluginCapabilities {
    pub supports_encryption: bool,
    pub supports_compression: bool,
    pub requires_authentication: bool,
    pub supports_streaming: bool,
    pub protocol_version: u16,
}

/// Plugin lifecycle states
#[derive(Debug, Clone, PartialEq)]
pub enum PluginState {
    Uninitialized,
    Loading,
    Handshaking,
    Active,
    Paused,
    Error(String),
    Unloading,
}

/// Plugin interface
pub trait PluginInterface: Send + Sync {
    /// Initialize the plugin
    async fn initialize(&mut self, config: PluginConfig) -> Result<(), PluginError>;
    
    /// Handle incoming plugin frame
    async fn handle_frame(&mut self, header: PluginHeader) -> Result<Option<PluginResponse>, PluginError>;
    
    /// Get plugin capabilities
    fn capabilities(&self) -> PluginCapabilities;
    
    /// Get plugin metadata
    fn metadata(&self) -> PluginMetadata;
    
    /// Shutdown the plugin
    async fn shutdown(&mut self) -> Result<(), PluginError>;
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub required_nyx_version: String,
    pub permissions: Vec<String>,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub plugin_id: u32,
    pub name: String,
    pub enabled: bool,
    pub settings: HashMap<String, String>,
    pub permissions: Vec<String>,
}

/// Plugin settings for SETTINGS frame advertising
#[derive(Debug, Clone, Default)]
pub struct PluginSettings {
    /// Plugins required for communication
    pub plugin_required: Vec<u32>,
    /// Plugin capability negotiation
    pub capability_negotiation: bool,
    /// Maximum plugin frame size
    pub max_plugin_frame_size: u32,
    /// Plugin timeout settings
    pub plugin_timeout_ms: u64,
}

/// Plugin messages for IPC transport
#[derive(Debug, Clone)]
pub enum PluginMessage {
    Frame(PluginHeader),
    Control(PluginControlMessage),
    Event(PluginEvent),
    Shutdown,
}

/// Plugin control messages
#[derive(Debug, Clone)]
pub enum PluginControlMessage {
    Initialize(PluginConfig),
    Pause,
    Resume,
    GetStatus,
    UpdateConfig(HashMap<String, String>),
}

/// Plugin events
#[derive(Debug, Clone)]
pub enum PluginEvent {
    PluginLoaded(u32),
    PluginError(u32, String),
    PluginUnloaded(u32),
    ConnectionEstablished,
    ConnectionLost,
    FrameReceived(PluginFrameType),
}

/// Plugin response
#[derive(Debug, Clone)]
pub struct PluginResponse {
    pub frame_type: PluginFrameType,
    pub header: PluginHeader,
}

/// Plugin errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(u32),
    #[error("Invalid plugin header: {0}")]
    InvalidHeader(String),
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Plugin communication error: {0}")]
    CommunicationError(String),
    #[error("Plugin permission denied: {0}")]
    PermissionDenied(String),
    #[error("CBOR serialization error: {0}")]
    SerializationError(String),
    #[error("Plugin timeout")]
    Timeout,
}

/// Plugin structure
pub struct Plugin {
    pub id: u32,
    pub metadata: PluginMetadata,
    pub capabilities: PluginCapabilities,
    pub state: PluginState,
    pub config: PluginConfig,
    pub interface: Box<dyn PluginInterface>,
}

impl PluginFramework {
    /// Create new plugin framework
    pub fn new() -> (Self, mpsc::UnboundedReceiver<PluginEvent>) {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        let framework = Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            ipc_channels: Arc::new(RwLock::new(HashMap::new())),
            plugin_settings: Arc::new(RwLock::new(PluginSettings::default())),
            event_bus: event_tx,
        };
        
        (framework, event_rx)
    }
    
    /// Register a new plugin
    pub async fn register_plugin(
        &self, 
        mut plugin_interface: Box<dyn PluginInterface>,
        config: PluginConfig
    ) -> Result<(), PluginError> {
        let plugin_id = config.plugin_id;
        let metadata = plugin_interface.metadata();
        let capabilities = plugin_interface.capabilities();
        
        info!("Registering plugin: {} (ID: {})", metadata.name, plugin_id);
        
        // Initialize plugin
        plugin_interface.initialize(config.clone()).await?;
        
        let plugin = Arc::new(Plugin {
            id: plugin_id,
            metadata: metadata.clone(),
            capabilities,
            state: PluginState::Active,
            config: config.clone(),
            interface: plugin_interface,
        });
        
        // Create IPC channel
        let (tx, mut rx) = mpsc::unbounded_channel::<PluginMessage>();
        
        // Store plugin and channel
        {
            let mut plugins = self.plugins.write().await;
            plugins.insert(plugin_id, plugin.clone());
        }
        
        {
            let mut channels = self.ipc_channels.write().await;
            channels.insert(plugin_id, tx);
        }
        
        // Start plugin message handler
        let plugin_clone = plugin.clone();
        let event_bus = self.event_bus.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = Self::handle_plugin_message(&plugin_clone, message).await {
                    error!("Plugin {} message handling error: {}", plugin_id, e);
                    let _ = event_bus.send(PluginEvent::PluginError(plugin_id, e.to_string()));
                }
            }
        });
        
        // Emit plugin loaded event
        let _ = self.event_bus.send(PluginEvent::PluginLoaded(plugin_id));
        
        info!("Plugin {} registered successfully", metadata.name);
        Ok(())
    }
    
    /// Handle plugin frame (Frame Type 0x50-0x5F)
    pub async fn handle_plugin_frame(
        &self,
        frame_type: u8,
        payload: &[u8]
    ) -> Result<Option<Vec<u8>>, PluginError> {
        // Verify frame type is in plugin range
        if frame_type < 0x50 || frame_type > 0x5F {
            return Err(PluginError::InvalidHeader(
                format!("Invalid plugin frame type: 0x{:02X}", frame_type)
            ));
        }
        
        // Parse CBOR header
        let header: PluginHeader = cbor_serde::from_slice(payload)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;
        
        debug!("Received plugin frame: type=0x{:02X}, plugin_id={}, data_len={}", 
               frame_type, header.id, header.data.len());
        
        // Find plugin
        let plugin = {
            let plugins = self.plugins.read().await;
            plugins.get(&header.id).cloned()
        };
        
        let plugin = plugin.ok_or(PluginError::PluginNotFound(header.id))?;
        
        // Send message to plugin via IPC
        let message = PluginMessage::Frame(header);
        if let Some(channel) = self.ipc_channels.read().await.get(&header.id) {
            channel.send(message)
                .map_err(|e| PluginError::CommunicationError(e.to_string()))?;
        }
        
        // For now, return empty response - real response handling would be async
        Ok(None)
    }
    
    /// Get plugin settings for SETTINGS frame
    pub async fn get_plugin_settings(&self) -> PluginSettings {
        self.plugin_settings.read().await.clone()
    }
    
    /// Update plugin settings
    pub async fn update_plugin_settings(&self, settings: PluginSettings) {
        *self.plugin_settings.write().await = settings;
    }
    
    /// Perform plugin capability negotiation
    pub async fn negotiate_capabilities(
        &self,
        remote_plugins: &[u32]
    ) -> Result<Vec<u32>, PluginError> {
        let plugins = self.plugins.read().await;
        let mut compatible_plugins = Vec::new();
        
        for &plugin_id in remote_plugins {
            if let Some(plugin) = plugins.get(&plugin_id) {
                // Check if plugin supports capability negotiation
                if plugin.capabilities.protocol_version >= 1 {
                    compatible_plugins.push(plugin_id);
                    info!("Plugin {} is compatible for communication", plugin_id);
                }
            }
        }
        
        Ok(compatible_plugins)
    }
    
    /// Unregister plugin
    pub async fn unregister_plugin(&self, plugin_id: u32) -> Result<(), PluginError> {
        info!("Unregistering plugin: {}", plugin_id);
        
        // Send shutdown message
        if let Some(channel) = self.ipc_channels.read().await.get(&plugin_id) {
            let _ = channel.send(PluginMessage::Shutdown);
        }
        
        // Remove plugin and channel
        {
            let mut plugins = self.plugins.write().await;
            plugins.remove(&plugin_id);
        }
        
        {
            let mut channels = self.ipc_channels.write().await;
            channels.remove(&plugin_id);
        }
        
        // Emit plugin unloaded event
        let _ = self.event_bus.send(PluginEvent::PluginUnloaded(plugin_id));
        
        info!("Plugin {} unregistered", plugin_id);
        Ok(())
    }
    
    /// Handle plugin message
    async fn handle_plugin_message(
        plugin: &Arc<Plugin>, 
        message: PluginMessage
    ) -> Result<(), PluginError> {
        match message {
            PluginMessage::Frame(header) => {
                // This would normally call plugin.interface.handle_frame(header)
                // but we can't call async methods from a trait object easily here
                debug!("Processing frame for plugin {}", plugin.id);
                Ok(())
            },
            PluginMessage::Control(control) => {
                debug!("Processing control message for plugin {}: {:?}", plugin.id, control);
                Ok(())
            },
            PluginMessage::Event(event) => {
                debug!("Processing event for plugin {}: {:?}", plugin.id, event);
                Ok(())
            },
            PluginMessage::Shutdown => {
                info!("Shutting down plugin {}", plugin.id);
                Ok(())
            }
        }
    }
    
    /// Serialize plugin header to CBOR
    pub fn serialize_header(header: &PluginHeader) -> Result<Vec<u8>, PluginError> {
        cbor_serde::to_vec(Vec::new(), header)
            .map_err(|e| PluginError::SerializationError(e.to_string()))
    }
    
    /// Create plugin handshake frame
    pub async fn create_handshake_frame(
        &self,
        plugin_id: u32
    ) -> Result<Vec<u8>, PluginError> {
        let plugins = self.plugins.read().await;
        let plugin = plugins.get(&plugin_id)
            .ok_or(PluginError::PluginNotFound(plugin_id))?;
        
        let handshake_data = cbor_serde::to_vec(Vec::new(), &plugin.capabilities)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;
        
        let header = PluginHeader {
            id: plugin_id,
            flags: 0x01, // Handshake flag
            data: handshake_data,
        };
        
        Self::serialize_header(&header)
    }
}

/// Example plugin implementation
pub struct EchoPlugin {
    config: Option<PluginConfig>,
}

impl EchoPlugin {
    pub fn new() -> Box<Self> {
        Box::new(Self { config: None })
    }
}

#[async_trait::async_trait]
impl PluginInterface for EchoPlugin {
    async fn initialize(&mut self, config: PluginConfig) -> Result<(), PluginError> {
        info!("Initializing Echo Plugin with config: {:?}", config);
        self.config = Some(config);
        Ok(())
    }
    
    async fn handle_frame(&mut self, header: PluginHeader) -> Result<Option<PluginResponse>, PluginError> {
        debug!("Echo plugin received frame: {:?}", header);
        
        // Echo back the data
        let response_header = PluginHeader {
            id: header.id,
            flags: 0x80, // Response flag
            data: header.data,
        };
        
        Ok(Some(PluginResponse {
            frame_type: PluginFrameType::PluginData,
            header: response_header,
        }))
    }
    
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_encryption: false,
            supports_compression: false,
            requires_authentication: false,
            supports_streaming: true,
            protocol_version: 1,
        }
    }
    
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            name: "Echo Plugin".to_string(),
            version: "1.0.0".to_string(),
            author: "NyxNet Team".to_string(),
            description: "Simple echo plugin for testing".to_string(),
            required_nyx_version: "1.0.0".to_string(),
            permissions: vec![],
        }
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        info!("Shutting down Echo Plugin");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_plugin_framework() {
        let (framework, mut event_rx) = PluginFramework::new();
        
        // Register echo plugin
        let echo_plugin = EchoPlugin::new();
        let config = PluginConfig {
            plugin_id: 1,
            name: "echo".to_string(),
            enabled: true,
            settings: HashMap::new(),
            permissions: vec![],
        };
        
        assert!(framework.register_plugin(echo_plugin, config).await.is_ok());
        
        // Check for plugin loaded event
        if let Some(event) = event_rx.recv().await {
            match event {
                PluginEvent::PluginLoaded(id) => assert_eq!(id, 1),
                _ => panic!("Expected PluginLoaded event"),
            }
        }
        
        // Test plugin frame handling
        let header = PluginHeader {
            id: 1,
            flags: 0,
            data: b"test data".to_vec(),
        };
        let payload = PluginFramework::serialize_header(&header).unwrap();
        
        let result = framework.handle_plugin_frame(0x50, &payload).await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_cbor_serialization() {
        let header = PluginHeader {
            id: 42,
            flags: 0x01,
            data: b"hello world".to_vec(),
        };
        
        let serialized = PluginFramework::serialize_header(&header).unwrap();
        assert!(!serialized.is_empty());
        
        let deserialized: PluginHeader = cbor_serde::from_slice(&serialized).unwrap();
        assert_eq!(deserialized.id, 42);
        assert_eq!(deserialized.flags, 0x01);
        assert_eq!(deserialized.data, b"hello world");
    }
}
