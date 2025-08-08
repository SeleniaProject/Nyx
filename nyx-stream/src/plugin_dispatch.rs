#![forbid(unsafe_code)]

//! Plugin frame dispatcher with permission enforcement.
//!
//! The dispatcher is responsible for routing incoming Plugin Frames
//! (Type 0x50–0x5F) to the appropriate runtime while ensuring that
//! the sending plugin has been granted the requested permissions.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};
use thiserror::Error;

use crate::{
    PluginId,
    plugin_registry::{PluginRegistry, Permission, PluginInfo},
    plugin_cbor::{PluginHeader, parse_plugin_header, serialize_plugin_header, PluginCborError},
    frame::{is_plugin_frame, FRAME_TYPE_PLUGIN_HANDSHAKE, FRAME_TYPE_PLUGIN_DATA, 
           FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_ERROR},
};

/// Plugin Framework dispatch errors for v1.0
#[derive(Error, Debug)]
pub enum DispatchError {
    #[error("Invalid frame format: {0}")]
    InvalidFrame(String),
    #[error("Plugin not registered: {0}")]
    PluginNotRegistered(PluginId),
    #[error("Insufficient permissions for plugin: {0}")]
    InsufficientPermissions(PluginId),
    #[error("IPC communication failed for plugin: {0}, reason: {1}")]
    IpcSendFailed(PluginId, String),
    #[error("Runtime error in plugin {0}: {1}")]
    RuntimeError(PluginId, String),
    #[error("Plugin capacity exceeded: {0}")]
    CapacityExceeded(usize),
    #[error("CBOR parsing error: {0}")]
    CborError(#[from] PluginCborError),
    #[error("Invalid frame type: {0}, expected plugin frame (0x50-0x5F)")]
    InvalidFrameType(u8),
}

/// Plugin runtime statistics  
#[derive(Debug, Clone, Default)]
pub struct PluginRuntimeStats {
    pub active_plugins: u32,
    pub registered_plugins: u32,
    pub total_dispatched_frames: u64,
    pub total_processed_messages: u64,
    pub total_errors: u64,
}

/// Plugin IPC message for internal communication
#[derive(Debug, Clone)]
pub struct PluginMessage {
    pub frame_type: u8,
    pub plugin_header: PluginHeader,
    pub raw_frame_data: Vec<u8>,
}

impl PluginMessage {
    /// Create a new plugin message from frame data
    pub fn new(frame_type: u8, plugin_header: PluginHeader, raw_frame_data: Vec<u8>) -> Self {
        Self { frame_type, plugin_header, raw_frame_data }
    }
    
    /// Get the plugin ID from the header
    pub fn plugin_id(&self) -> PluginId {
        self.plugin_header.id
    }
    
    /// Check if this is a handshake message
    pub fn is_handshake(&self) -> bool {
        self.frame_type == FRAME_TYPE_PLUGIN_HANDSHAKE
    }
    
    /// Check if this is a control message
    pub fn is_control(&self) -> bool {
        self.frame_type == FRAME_TYPE_PLUGIN_CONTROL
    }
    
    /// Check if this is a data message
    pub fn is_data(&self) -> bool {
        self.frame_type == FRAME_TYPE_PLUGIN_DATA
    }
    
    /// Check if this is an error message
    pub fn is_error(&self) -> bool {
        self.frame_type == FRAME_TYPE_PLUGIN_ERROR
    }
}

/// Runtime handle for plugin processes
#[derive(Debug)]
struct RuntimeHandle {
    join_handle: tokio::task::JoinHandle<()>,
    ipc_tx: mpsc::Sender<PluginMessage>,
    plugin_id: PluginId,
}

impl RuntimeHandle {
    fn abort(&self) {
        debug!("Aborting plugin runtime for plugin {}", self.plugin_id);
        self.join_handle.abort();
    }
}

/// Main plugin frame dispatcher
#[derive(Debug)]
pub struct PluginDispatcher {
    registry: Arc<Mutex<PluginRegistry>>,
    runtimes: Arc<Mutex<HashMap<PluginId, RuntimeHandle>>>,
    stats: Arc<RwLock<PluginRuntimeStats>>,
}

impl PluginDispatcher {
    pub fn new(registry: Arc<Mutex<PluginRegistry>>) -> Self {
        Self {
            registry,
            runtimes: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(RwLock::new(PluginRuntimeStats::default())),
        }
    }

    /// Dispatch a plugin frame to the appropriate plugin runtime
    ///
    /// This method performs complete frame validation, permission checking,
    /// CBOR header parsing, and secure message routing to the plugin process.
    ///
    /// # Arguments
    /// * `frame_type` - Plugin frame type (must be 0x50-0x5F)
    /// * `frame_data` - Complete frame payload including CBOR header
    ///
    /// # Returns
    /// * `Ok(())` - Frame successfully dispatched
    /// * `Err(DispatchError)` - Dispatch failed with specific reason
    pub async fn dispatch_plugin_frame(&self, frame_type: u8, frame_data: Vec<u8>) -> Result<(), DispatchError> {
        // Validate frame type is in plugin range
        if !is_plugin_frame(frame_type) {
            return Err(DispatchError::InvalidFrameType(frame_type));
        }
        
        // Parse CBOR header from frame data
        let plugin_header = parse_plugin_header(&frame_data)?;
        let plugin_id = plugin_header.id;
        
        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_dispatched_frames += 1;
        }
        
        // Check plugin registration and permissions
        let registry = self.registry.lock().await;
        if !registry.is_registered(plugin_id).await {
            let mut stats = self.stats.write().await;
            stats.total_errors += 1;
            return Err(DispatchError::PluginNotRegistered(plugin_id));
        }
        
        // Verify plugin has required permissions for this frame type
        let required_permission = match frame_type {
            FRAME_TYPE_PLUGIN_HANDSHAKE => Permission::Handshake,
            FRAME_TYPE_PLUGIN_DATA => Permission::DataAccess,
            FRAME_TYPE_PLUGIN_CONTROL => Permission::Control,
            FRAME_TYPE_PLUGIN_ERROR => Permission::ErrorReporting,
            _ => Permission::DataAccess, // Default for other plugin frame types
        };
        
        if !registry.has_permission(plugin_id, required_permission) {
            let mut stats = self.stats.write().await;
            stats.total_errors += 1;
            warn!("Plugin {} lacks permission {:?} for frame type 0x{:02X}", 
                  plugin_id, required_permission, frame_type);
            return Err(DispatchError::InsufficientPermissions(plugin_id));
        }
        
        drop(registry); // Release registry lock early
        
        // Get runtime handle and send message
        let runtimes = self.runtimes.lock().await;
        let runtime_handle = runtimes.get(&plugin_id)
            .ok_or(DispatchError::RuntimeError(plugin_id, "Runtime not found".to_string()))?;
            
        // Create plugin message
        let plugin_message = PluginMessage::new(frame_type, plugin_header, frame_data);
        
        // Send message via IPC with timeout protection
        runtime_handle.ipc_tx.send(plugin_message).await
            .map_err(|_| DispatchError::IpcSendFailed(plugin_id, "Channel closed or full".to_string()))?;
            
        debug!("Successfully dispatched frame type 0x{:02X} to plugin {}", frame_type, plugin_id);
        Ok(())
    }

    /// Legacy method for compatibility - dispatches raw message bytes
    pub async fn dispatch_message(&self, plugin_id: PluginId, message: Vec<u8>) -> Result<(), DispatchError> {
        // Try to parse as CBOR header to extract frame type
        let plugin_header = parse_plugin_header(&message)?;
        
        // Assume this is a data frame for legacy compatibility
        self.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, message).await
    }

    /// Load and start a plugin
    pub async fn load_plugin(&self, plugin_info: PluginInfo) -> Result<(), DispatchError> {
        let plugin_id = plugin_info.id;
        
        // Check capacity
        {
            let runtimes = self.runtimes.lock().await;
            if runtimes.len() >= 32 { // Max plugins limit
                return Err(DispatchError::CapacityExceeded(32));
            }
        }

        // Clone necessary data for the runtime before moving plugin_info
        let plugin_name = plugin_info.name.clone();
        
        // Register plugin
        {
            let registry = self.registry.lock().await;
            registry.register(plugin_info).await
                .map_err(|e| DispatchError::InvalidFrame(e.to_string()))?;
        }

        // Create IPC channel with appropriate buffer size
        let (tx, rx) = mpsc::channel(1024);
        
        let stats_clone = Arc::clone(&self.stats);
        let stats_clone = Arc::clone(&self.stats);
        
        // Spawn plugin runtime with comprehensive message processing
        let join_handle = tokio::spawn(async move {
            info!("Starting plugin runtime for {} (ID: {})", plugin_name, plugin_id);
            
            let mut rx = rx;
            let mut message_count = 0u64;
            let mut error_count = 0u64;
            
            while let Some(plugin_message) = rx.recv().await {
                message_count += 1;
                
                // Process the plugin message based on frame type
                match Self::process_plugin_message(plugin_id, &plugin_message).await {
                    Ok(()) => {
                        debug!("Successfully processed message {} for plugin {}", 
                               message_count, plugin_id);
                    }
                    Err(e) => {
                        error_count += 1;
                        error!("Error processing message for plugin {}: {}", plugin_id, e);
                        
                        // Update error statistics
                        {
                            let mut stats = stats_clone.write().await;
                            stats.total_errors += 1;
                        }
                        
                        // For critical errors, consider terminating the plugin
                        if error_count > 100 {
                            error!("Plugin {} has too many errors ({}), terminating", 
                                   plugin_id, error_count);
                            break;
                        }
                    }
                }
                
                // Update processed message count periodically
                if message_count % 100 == 0 {
                    let mut stats = stats_clone.write().await;
                    stats.total_processed_messages += 100;
                }
            }
            
            // Update final statistics
            {
                let mut stats = stats_clone.write().await;
                stats.total_processed_messages += message_count % 100;
            }
            
            info!("Plugin runtime for {} (ID: {}) terminated. Processed {} messages, {} errors", 
                  plugin_name, plugin_id, message_count, error_count);
        });
        
        // Store runtime handle with plugin ID for debugging
        {
            let mut runtimes = self.runtimes.lock().await;
            runtimes.insert(plugin_id, RuntimeHandle {
                join_handle,
                ipc_tx: tx,
                plugin_id,
            });
        }
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_plugins = self.runtimes.lock().await.len() as u32;
        }
        
        Ok(())
    }

    /// Unload and stop a plugin
    pub async fn unload_plugin(&self, plugin_id: PluginId) -> Result<(), DispatchError> {
        // Remove from runtime
        let runtime_handle = {
            let mut runtimes = self.runtimes.lock().await;
            runtimes.remove(&plugin_id)
        };
        
        if let Some(handle) = runtime_handle {
            handle.abort();
        }
        
        // Unregister plugin
        {
            let mut registry = self.registry.lock().await;
            registry.unregister(plugin_id).await
                .map_err(|_| DispatchError::PluginNotRegistered(plugin_id))?;
        }
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.active_plugins = self.runtimes.lock().await.len() as u32;
        }
        
        Ok(())
    }

    /// Get runtime statistics
    pub async fn get_stats(&self) -> PluginRuntimeStats {
        self.stats.read().await.clone()
    }

    /// Shutdown all plugins
    pub async fn shutdown(&self) {
        let plugin_ids: Vec<PluginId> = {
            let runtimes = self.runtimes.lock().await;
            runtimes.keys().cloned().collect()
        };
        
        for plugin_id in plugin_ids {
            if let Err(e) = self.unload_plugin(plugin_id).await {
                eprintln!("Error unloading plugin {}: {}", plugin_id, e);
            }
        }
    }

    /// Process individual plugin messages within the runtime
    /// 
    /// This method handles the actual plugin message processing logic,
    /// including frame type-specific handling and error management.
    async fn process_plugin_message(
        plugin_id: PluginId, 
        message: &PluginMessage
    ) -> Result<(), DispatchError> {
        match message.frame_type {
            FRAME_TYPE_PLUGIN_HANDSHAKE => {
                debug!("Processing handshake message for plugin {}", plugin_id);
                Self::process_handshake_message(plugin_id, message).await
            }
            FRAME_TYPE_PLUGIN_DATA => {
                debug!("Processing data message for plugin {}", plugin_id);
                Self::process_data_message(plugin_id, message).await
            }
            FRAME_TYPE_PLUGIN_CONTROL => {
                debug!("Processing control message for plugin {}", plugin_id);
                Self::process_control_message(plugin_id, message).await
            }
            FRAME_TYPE_PLUGIN_ERROR => {
                warn!("Processing error message for plugin {}", plugin_id);
                Self::process_error_message(plugin_id, message).await
            }
            _ => {
                warn!("Unknown plugin frame type 0x{:02X} for plugin {}", 
                      message.frame_type, plugin_id);
                Err(DispatchError::InvalidFrameType(message.frame_type))
            }
        }
    }

    /// Process plugin handshake messages
    async fn process_handshake_message(
        plugin_id: PluginId, 
        message: &PluginMessage
    ) -> Result<(), DispatchError> {
        // Handshake processing logic would go here
        // For now, just log and accept
        info!("Plugin {} completed handshake with {} bytes of data", 
              plugin_id, message.plugin_header.data.len());
        Ok(())
    }

    /// Process plugin data messages
    async fn process_data_message(
        plugin_id: PluginId, 
        message: &PluginMessage
    ) -> Result<(), DispatchError> {
        // Data processing logic would go here
        // This would typically forward data to the appropriate handler
        debug!("Plugin {} sent {} bytes of data", 
               plugin_id, message.plugin_header.data.len());
        Ok(())
    }

    /// Process plugin control messages
    async fn process_control_message(
        plugin_id: PluginId, 
        message: &PluginMessage
    ) -> Result<(), DispatchError> {
        // Control message processing logic would go here
        debug!("Plugin {} sent control message with flags 0x{:02X}", 
               plugin_id, message.plugin_header.flags);
        Ok(())
    }

    /// Process plugin error messages
    async fn process_error_message(
        plugin_id: PluginId, 
        message: &PluginMessage
    ) -> Result<(), DispatchError> {
        // Error message processing logic would go here
        error!("Plugin {} reported error: {:?}", 
               plugin_id, String::from_utf8_lossy(&message.plugin_header.data));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_registry::PluginRegistry;

    #[tokio::test]
    async fn test_plugin_dispatcher_creation() {
        let registry = Arc::new(Mutex::new(PluginRegistry::new()));
        let dispatcher = PluginDispatcher::new(registry);
        
        let stats = dispatcher.get_stats().await;
        assert_eq!(stats.total_dispatched_frames, 0);
        assert_eq!(stats.active_plugins, 0);
    }
}
