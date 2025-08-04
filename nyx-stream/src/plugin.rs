#![forbid(unsafe_code)]

//! Plugin Framework for Nyx Protocol v1.0
//!
//! This module implements the complete Plugin Framework including:
//! - Frame Type 0x50-0x5F plugin reservation
//! - CBOR header parsing with {id:u32, flags:u8, data:bytes}
//! - SETTINGS PLUGIN_REQUIRED advertising
//! - Plugin handshake mechanisms
//! - Plugin IPC transport integration

use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, warn, trace};

use crate::frame::{
    FRAME_TYPE_PLUGIN_START, FRAME_TYPE_PLUGIN_END,
    FRAME_TYPE_PLUGIN_HANDSHAKE, FRAME_TYPE_PLUGIN_DATA,
    FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_ERROR,
    is_plugin_frame
};

/// Plugin identifier type
pub type PluginId = u32;

/// Plugin frame flags
pub mod plugin_flags {
    /// Plugin is required for operation (peer must support or abort with 0x07)
    pub const FLAG_PLUGIN_REQUIRED: u8 = 0x01;
    /// Plugin is optional, can be skipped if not supported
    pub const FLAG_PLUGIN_OPTIONAL: u8 = 0x02;
    /// Plugin payload is encrypted
    pub const FLAG_PLUGIN_ENCRYPTED: u8 = 0x04;
    /// Plugin payload is compressed
    pub const FLAG_PLUGIN_COMPRESSED: u8 = 0x08;
    /// Plugin frame is fragmented (continuation follows)
    pub const FLAG_PLUGIN_FRAGMENTED: u8 = 0x10;
    /// Plugin requires network access permission
    pub const FLAG_PLUGIN_NETWORK_ACCESS: u8 = 0x20;
    /// Plugin requires file system access permission
    pub const FLAG_PLUGIN_FILE_ACCESS: u8 = 0x40;
    /// Plugin requires inter-plugin IPC communication
    pub const FLAG_PLUGIN_IPC_ACCESS: u8 = 0x80;
}

/// CBOR header structure for plugin frames
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginHeader<'a> {
    /// Plugin identifier (32-bit)
    pub id: u32,
    /// Plugin-specific flags (8-bit)
    pub flags: u8,
    /// Plugin payload data
    #[serde(with = "serde_bytes")]
    pub data: &'a [u8],
}

impl<'a> PluginHeader<'a> {
    /// Encode plugin header to CBOR format
    pub fn encode(&self) -> Result<Vec<u8>, PluginError> {
        serde_cbor::to_vec(self).map_err(|e| PluginError::SerializationError(e.to_string()))
    }

    /// Decode plugin header from CBOR format
    pub fn decode(bytes: &'a [u8]) -> Result<Self, PluginError> {
        serde_cbor::from_slice(bytes).map_err(|e| PluginError::SerializationError(e.to_string()))
    }

    /// Validate plugin header structure and constraints
    pub fn validate(&self) -> Result<(), PluginError> {
        // Validate plugin ID range (non-zero)
        if self.id == 0 {
            return Err(PluginError::InvalidPluginId(self.id));
        }

        // Validate data size limits (max 64KB for plugin payload)
        if self.data.len() > 65536 {
            return Err(PluginError::PayloadTooLarge(self.data.len()));
        }

        // Validate flag combinations
        if (self.flags & plugin_flags::FLAG_PLUGIN_REQUIRED) != 0 &&
           (self.flags & plugin_flags::FLAG_PLUGIN_OPTIONAL) != 0 {
            return Err(PluginError::InvalidFlags(self.flags));
        }

        Ok(())
    }
}

/// Plugin Framework errors
#[derive(Error, Debug, Clone)]
pub enum PluginError {
    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid plugin ID: {0}")]
    InvalidPluginId(u32),

    #[error("Plugin not found: {0}")]
    PluginNotFound(u32),

    #[error("Plugin already registered: {0}")]
    PluginAlreadyRegistered(u32),

    #[error("Payload too large: {0} bytes (max 65536)")]
    PayloadTooLarge(usize),

    #[error("Invalid plugin flags: 0x{0:02x}")]
    InvalidFlags(u8),

    #[error("Plugin handshake failed: {0}")]
    HandshakeFailed(String),

    #[error("IPC transport error: {0}")]
    IpcTransportError(String),

    #[error("Plugin frame processing error: {0}")]
    FrameProcessingError(String),
}

/// Plugin capability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCapability {
    /// Plugin unique identifier
    pub id: PluginId,
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Required by this plugin
    pub required: bool,
    /// Supported frame types
    pub supported_frames: Vec<u8>,
    /// Plugin-specific configuration
    pub config: HashMap<String, String>,
}

/// Plugin handshake message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHandshake {
    /// Plugin capability information
    pub capability: PluginCapability,
    /// Handshake challenge/response data
    pub challenge: Vec<u8>,
    /// Authentication token
    pub auth_token: Option<Vec<u8>>,
}

/// Plugin frame types for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginFrame {
    /// Plugin handshake frame (0x50)
    Handshake(PluginHandshake),
    /// Plugin data frame (0x51)
    Data { plugin_id: PluginId, payload: Vec<u8> },
    /// Plugin control frame (0x52)
    Control { plugin_id: PluginId, command: String, params: HashMap<String, String> },
    /// Plugin error frame (0x53)
    Error { plugin_id: PluginId, error_code: u16, message: String },
}

impl PluginFrame {
    /// Get the appropriate frame type for this plugin frame
    pub fn frame_type(&self) -> u8 {
        match self {
            PluginFrame::Handshake(_) => FRAME_TYPE_PLUGIN_HANDSHAKE,
            PluginFrame::Data { .. } => FRAME_TYPE_PLUGIN_DATA,
            PluginFrame::Control { .. } => FRAME_TYPE_PLUGIN_CONTROL,
            PluginFrame::Error { .. } => FRAME_TYPE_PLUGIN_ERROR,
        }
    }

    /// Encode plugin frame to bytes
    pub fn encode(&self) -> Result<Vec<u8>, PluginError> {
        let payload = serde_cbor::to_vec(self)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;
        Ok(payload)
    }

    /// Decode plugin frame from bytes  
    pub fn decode(frame_type: u8, payload: &[u8]) -> Result<Self, PluginError> {
        if !is_plugin_frame(frame_type) {
            return Err(PluginError::FrameProcessingError(
                format!("Invalid plugin frame type: 0x{:02x}", frame_type)
            ));
        }

        let frame: PluginFrame = serde_cbor::from_slice(payload)
            .map_err(|e| PluginError::SerializationError(e.to_string()))?;

        // Verify frame type matches the decoded content
        if frame.frame_type() != frame_type {
            return Err(PluginError::FrameProcessingError(
                format!("Frame type mismatch: expected 0x{:02x}, got 0x{:02x}", 
                        frame_type, frame.frame_type())
            ));
        }

        Ok(frame)
    }
}

/// Plugin registry for managing active plugins
#[derive(Debug)]
pub struct PluginRegistry {
    /// Registered plugins by ID
    plugins: Arc<RwLock<HashMap<PluginId, PluginCapability>>>,
    /// Required plugins that must be present
    required_plugins: Arc<RwLock<Vec<PluginId>>>,
    /// Plugin event channel
    event_tx: mpsc::UnboundedSender<PluginEvent>,
    /// Plugin event receiver
    event_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<PluginEvent>>>>,
}

/// Plugin events for coordination and monitoring
#[derive(Debug, Clone)]
pub enum PluginEvent {
    /// Plugin registered successfully
    PluginRegistered { plugin_id: PluginId, capability: PluginCapability },
    /// Plugin unregistered
    PluginUnregistered { plugin_id: PluginId },
    /// Plugin handshake completed
    HandshakeCompleted { plugin_id: PluginId, success: bool },
    /// Plugin frame received
    FrameReceived { plugin_id: PluginId, frame_type: u8, size: usize },
    /// Plugin error occurred
    PluginError { plugin_id: PluginId, error: PluginError },
}

impl PluginRegistry {
    /// Create new plugin registry
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            required_plugins: Arc::new(RwLock::new(Vec::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
        }
    }

    /// Register a plugin with the registry
    pub fn register_plugin(&self, capability: PluginCapability) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().unwrap();
        
        if plugins.contains_key(&capability.id) {
            return Err(PluginError::PluginAlreadyRegistered(capability.id));
        }

        // Add to required plugins if marked as required
        if capability.required {
            let mut required = self.required_plugins.write().unwrap();
            if !required.contains(&capability.id) {
                required.push(capability.id);
            }
        }

        plugins.insert(capability.id, capability.clone());

        // Send registration event
        let _ = self.event_tx.send(PluginEvent::PluginRegistered {
            plugin_id: capability.id,
            capability: capability.clone(),
        });

        debug!(plugin_id = capability.id, "Plugin registered successfully");
        Ok(())
    }

    /// Unregister a plugin
    pub fn unregister_plugin(&self, plugin_id: PluginId) -> Result<(), PluginError> {
        let mut plugins = self.plugins.write().unwrap();
        
        if !plugins.contains_key(&plugin_id) {
            return Err(PluginError::PluginNotFound(plugin_id));
        }

        plugins.remove(&plugin_id);

        // Remove from required plugins
        let mut required = self.required_plugins.write().unwrap();
        required.retain(|&id| id != plugin_id);

        // Send unregistration event
        let _ = self.event_tx.send(PluginEvent::PluginUnregistered { plugin_id });

        debug!(plugin_id = plugin_id, "Plugin unregistered successfully");
        Ok(())
    }

    /// Get plugin capability by ID
    pub fn get_plugin(&self, plugin_id: PluginId) -> Option<PluginCapability> {
        let plugins = self.plugins.read().unwrap();
        plugins.get(&plugin_id).cloned()
    }

    /// List all registered plugins
    pub fn list_plugins(&self) -> Vec<PluginCapability> {
        let plugins = self.plugins.read().unwrap();
        plugins.values().cloned().collect()
    }

    /// Get required plugins list
    pub fn get_required_plugins(&self) -> Vec<PluginId> {
        let required = self.required_plugins.read().unwrap();
        required.clone()
    }

    /// Check if all required plugins are registered
    pub fn validate_required_plugins(&self) -> Result<(), PluginError> {
        let plugins = self.plugins.read().unwrap();
        let required = self.required_plugins.read().unwrap();

        for &plugin_id in required.iter() {
            if !plugins.contains_key(&plugin_id) {
                return Err(PluginError::PluginNotFound(plugin_id));
            }
        }

        Ok(())
    }

    /// Take the event receiver (can only be called once)
    pub fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<PluginEvent>> {
        let mut receiver = self.event_rx.write().unwrap();
        receiver.take()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Plugin dispatcher for handling plugin frames and IPC
#[derive(Debug)]
pub struct PluginDispatcher {
    /// Plugin registry
    registry: Arc<PluginRegistry>,
    /// Active plugin connections
    connections: Arc<RwLock<HashMap<PluginId, PluginConnection>>>,
}

/// Plugin IPC connection
#[derive(Debug)]
pub struct PluginConnection {
    /// Plugin ID
    pub plugin_id: PluginId,
    /// Frame sender to plugin
    pub frame_tx: mpsc::UnboundedSender<PluginFrame>,
    /// Frame receiver from plugin
    pub frame_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<PluginFrame>>>>,
    /// Connection state
    pub state: PluginConnectionState,
}

/// Plugin connection states
#[derive(Debug, Clone, PartialEq)]
pub enum PluginConnectionState {
    /// Connection initializing
    Initializing,
    /// Handshake in progress
    Handshaking,
    /// Connection active and ready
    Active,
    /// Connection error state
    Error(String),
    /// Connection closed
    Closed,
}

impl PluginDispatcher {
    /// Create new plugin dispatcher
    pub fn new(registry: Arc<PluginRegistry>) -> Self {
        Self {
            registry,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Process incoming plugin frame
    pub async fn process_frame(&self, frame_type: u8, payload: &[u8]) -> Result<(), PluginError> {
        if !is_plugin_frame(frame_type) {
            return Err(PluginError::FrameProcessingError(
                format!("Invalid plugin frame type: 0x{:02x}", frame_type)
            ));
        }

        let plugin_frame = PluginFrame::decode(frame_type, payload)?;

        match plugin_frame {
            PluginFrame::Handshake(handshake) => {
                self.process_handshake(handshake).await
            }
            PluginFrame::Data { plugin_id, payload } => {
                self.process_data_frame(plugin_id, payload).await
            }
            PluginFrame::Control { plugin_id, command, params } => {
                self.process_control_frame(plugin_id, command, params).await
            }
            PluginFrame::Error { plugin_id, error_code, message } => {
                self.process_error_frame(plugin_id, error_code, message).await
            }
        }
    }

    /// Process plugin handshake
    async fn process_handshake(&self, handshake: PluginHandshake) -> Result<(), PluginError> {
        let plugin_id = handshake.capability.id;
        
        // Register plugin capability
        self.registry.register_plugin(handshake.capability.clone())?;

        // Create plugin connection
        let (frame_tx, frame_rx) = mpsc::unbounded_channel();
        let connection = PluginConnection {
            plugin_id,
            frame_tx,
            frame_rx: Arc::new(RwLock::new(Some(frame_rx))),
            state: PluginConnectionState::Handshaking,
        };

        {
            let mut connections = self.connections.write().unwrap();
            connections.insert(plugin_id, connection);
        }

        // Send handshake completion event
        let _ = self.registry.event_tx.send(PluginEvent::HandshakeCompleted {
            plugin_id,
            success: true,
        });

        debug!(plugin_id = plugin_id, "Plugin handshake completed successfully");
        Ok(())
    }

    /// Process plugin data frame
    async fn process_data_frame(&self, plugin_id: PluginId, payload: Vec<u8>) -> Result<(), PluginError> {
        // Verify plugin is registered
        if self.registry.get_plugin(plugin_id).is_none() {
            return Err(PluginError::PluginNotFound(plugin_id));
        }

        // Send frame received event
        let _ = self.registry.event_tx.send(PluginEvent::FrameReceived {
            plugin_id,
            frame_type: FRAME_TYPE_PLUGIN_DATA,
            size: payload.len(),
        });

        trace!(plugin_id = plugin_id, size = payload.len(), "Processed plugin data frame");
        Ok(())
    }

    /// Process plugin control frame
    async fn process_control_frame(
        &self,
        plugin_id: PluginId,
        command: String,
        params: HashMap<String, String>
    ) -> Result<(), PluginError> {
        // Verify plugin is registered
        if self.registry.get_plugin(plugin_id).is_none() {
            return Err(PluginError::PluginNotFound(plugin_id));
        }

        debug!(
            plugin_id = plugin_id,
            command = %command,
            "Processing plugin control frame"
        );

        // Handle common control commands
        match command.as_str() {
            "ping" => {
                // Respond with pong
                self.send_control_frame(plugin_id, "pong".to_string(), HashMap::new()).await?;
            }
            "shutdown" => {
                // Gracefully shutdown plugin connection
                self.close_connection(plugin_id).await?;
            }
            _ => {
                // Plugin-specific command handling would go here
                trace!(plugin_id = plugin_id, command = %command, "Unknown plugin control command");
            }
        }

        Ok(())
    }

    /// Process plugin error frame
    async fn process_error_frame(
        &self,
        plugin_id: PluginId,
        error_code: u16,
        message: String
    ) -> Result<(), PluginError> {
        let error = PluginError::FrameProcessingError(
            format!("Plugin {} error {}: {}", plugin_id, error_code, message)
        );

        // Send plugin error event
        let _ = self.registry.event_tx.send(PluginEvent::PluginError {
            plugin_id,
            error: error.clone(),
        });

        warn!(
            plugin_id = plugin_id,
            error_code = error_code,
            message = %message,
            "Plugin error frame received"
        );

        Err(error)
    }

    /// Send control frame to plugin
    pub async fn send_control_frame(
        &self,
        plugin_id: PluginId,
        command: String,
        params: HashMap<String, String>
    ) -> Result<(), PluginError> {
        let connections = self.connections.read().unwrap();
        
        if let Some(connection) = connections.get(&plugin_id) {
            let control_frame = PluginFrame::Control { plugin_id, command, params };
            
            connection.frame_tx.send(control_frame)
                .map_err(|e| PluginError::IpcTransportError(e.to_string()))?;
                
            Ok(())
        } else {
            Err(PluginError::PluginNotFound(plugin_id))
        }
    }

    /// Close plugin connection
    pub async fn close_connection(&self, plugin_id: PluginId) -> Result<(), PluginError> {
        let mut connections = self.connections.write().unwrap();
        
        if let Some(mut connection) = connections.remove(&plugin_id) {
            connection.state = PluginConnectionState::Closed;
            
            // Unregister plugin
            self.registry.unregister_plugin(plugin_id)?;
            
            debug!(plugin_id = plugin_id, "Plugin connection closed");
            Ok(())
        } else {
            Err(PluginError::PluginNotFound(plugin_id))
        }
    }

    /// Get connection state for plugin
    pub fn get_connection_state(&self, plugin_id: PluginId) -> Option<PluginConnectionState> {
        let connections = self.connections.read().unwrap();
        connections.get(&plugin_id).map(|conn| conn.state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_header_encode_decode() {
        let header = PluginHeader {
            id: 12345,
            flags: plugin_flags::FLAG_PLUGIN_REQUIRED,
            data: b"test plugin data",
        };

        let encoded = header.encode().expect("Failed to encode");
        let decoded = PluginHeader::decode(&encoded).expect("Failed to decode");

        assert_eq!(header, decoded);
    }

    #[test]
    fn test_plugin_header_validation() {
        // Valid header
        let valid_header = PluginHeader {
            id: 1,
            flags: plugin_flags::FLAG_PLUGIN_OPTIONAL,
            data: b"valid data",
        };
        assert!(valid_header.validate().is_ok());

        // Invalid ID (zero)
        let invalid_id = PluginHeader {
            id: 0,
            flags: 0,
            data: b"data",
        };
        assert!(invalid_id.validate().is_err());

        // Invalid flags (conflicting)
        let invalid_flags = PluginHeader {
            id: 1,
            flags: plugin_flags::FLAG_PLUGIN_REQUIRED | plugin_flags::FLAG_PLUGIN_OPTIONAL,
            data: b"data",
        };
        assert!(invalid_flags.validate().is_err());
    }

    #[tokio::test]
    async fn test_plugin_registry() {
        let registry = PluginRegistry::new();

        let capability = PluginCapability {
            id: 1001,
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            required: true,
            supported_frames: vec![FRAME_TYPE_PLUGIN_DATA, FRAME_TYPE_PLUGIN_CONTROL],
            config: HashMap::new(),
        };

        // Register plugin
        assert!(registry.register_plugin(capability.clone()).is_ok());

        // Verify plugin is registered
        assert!(registry.get_plugin(1001).is_some());
        assert_eq!(registry.list_plugins().len(), 1);
        assert_eq!(registry.get_required_plugins(), vec![1001]);

        // Verify required plugins validation
        assert!(registry.validate_required_plugins().is_ok());

        // Unregister plugin
        assert!(registry.unregister_plugin(1001).is_ok());
        assert!(registry.get_plugin(1001).is_none());
    }

    #[tokio::test]
    async fn test_plugin_frame_encoding() {
        let handshake = PluginHandshake {
            capability: PluginCapability {
                id: 2001,
                name: "Test Plugin".to_string(),
                version: "1.0.0".to_string(),
                required: false,
                supported_frames: vec![FRAME_TYPE_PLUGIN_DATA],
                config: HashMap::new(),
            },
            challenge: vec![1, 2, 3, 4],
            auth_token: Some(vec![5, 6, 7, 8]),
        };

        let frame = PluginFrame::Handshake(handshake);
        let frame_type = frame.frame_type();
        let encoded = frame.encode().expect("Failed to encode frame");
        let decoded = PluginFrame::decode(frame_type, &encoded).expect("Failed to decode frame");

        assert_eq!(frame_type, FRAME_TYPE_PLUGIN_HANDSHAKE);
        // Note: PluginFrame doesn't implement PartialEq, so we check the frame type instead
        assert_eq!(decoded.frame_type(), FRAME_TYPE_PLUGIN_HANDSHAKE);
    }

    #[test]
    fn test_is_plugin_frame() {
        assert!(is_plugin_frame(0x50));
        assert!(is_plugin_frame(0x55));
        assert!(is_plugin_frame(0x5F));
        assert!(!is_plugin_frame(0x4F));
        assert!(!is_plugin_frame(0x60));
    }
}

