#![forbid(unsafe_code)]

use crate::plugin::{PluginHeader, PluginId};
use crate::plugin_registry::{Permission, PluginInfo, PluginRegistry};
use crate::plugin_sandbox::{SandboxGuard, SandboxPolicy};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

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
    pub fn plugin_id(&self) -> PluginId {
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

/// Plugin runtime state for managing loaded plugins
#[derive(Debug)]
pub struct PluginRuntime {
    /// Plugin information
    pub info: PluginInfo,
    /// Message sender channel for the plugin
    pub sender: mpsc::Sender<PluginMessage>,
    /// Sandbox guard for security enforcement
    pub sandbox_guard: Option<SandboxGuard>,
    /// Statistics for the plugin
    pub stats: PluginStats,
}

/// Statistics for plugin execution
#[derive(Debug, Default, Clone)]
pub struct PluginStats {
    /// Number of messages processed
    pub messages_processed: u64,
    /// Number of errors encountered
    pub errors: u64,
    /// Total bytes processed
    pub bytes_processed: u64,
    /// Number of permission violations
    pub permission_violations: u64,
}

/// Errors that can occur during plugin dispatch
#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("Plugin {0:?} is not registered")]
    PluginNotRegistered(PluginId),
    #[error("Plugin {0:?} lacks permission for operation")]
    PermissionDenied(PluginId),
    #[error("IPC send failed for plugin {0:?}: {1}")]
    IpcSendFailed(PluginId, String),
    #[error("Plugin runtime error: {0}")]
    RuntimeError(String),
    #[error("Sandbox violation for plugin {0:?}: {1}")]
    SandboxViolation(PluginId, String),
}

/// Plugin dispatcher for managing plugin execution
#[derive(Debug)]
pub struct PluginDispatcher {
    /// Plugin registry for metadata management
    registry: Arc<PluginRegistry>,
    /// Loaded plugin runtimes
    runtimes: Arc<RwLock<HashMap<PluginId, PluginRuntime>>>,
    /// Default sandbox policy
    sandbox_policy: Option<SandboxPolicy>,
    /// Dispatcher statistics
    dispatch_stats: Arc<RwLock<DispatchStats>>,
}

/// Statistics for the dispatcher itself
#[derive(Debug, Default)]
pub struct DispatchStats {
    /// Total plugins loaded
    pub plugins_loaded: u64,
    /// Total plugins unloaded
    pub plugins_unloaded: u64,
    /// Total frames dispatched
    pub frames_dispatched: u64,
    /// Total dispatch errors
    pub dispatch_errors: u64,
}

impl PluginDispatcher {
    /// Create a new plugin dispatcher
    pub fn new(registry: Arc<PluginRegistry>) -> Self {
        Self {
            registry,
            runtimes: Arc::new(RwLock::new(HashMap::new())),
            sandbox_policy: None,
            dispatch_stats: Arc::new(RwLock::new(DispatchStats::default())),
        }
    }

    /// Create a new plugin dispatcher with sandbox policy
    pub fn new_with_sandbox(registry: Arc<PluginRegistry>, policy: SandboxPolicy) -> Self {
        Self {
            registry,
            runtimes: Arc::new(RwLock::new(HashMap::new())),
            sandbox_policy: Some(policy),
            dispatch_stats: Arc::new(RwLock::new(DispatchStats::default())),
        }
    }

    /// Load a plugin with default channel capacity
    pub async fn load_plugin(&self, info: PluginInfo) -> Result<(), Box<dyn std::error::Error>> {
        self.load_plugin_with_capacity(info, 100).await
    }

    /// Load a plugin with specified channel capacity
    pub async fn load_plugin_with_capacity(
        &self,
        info: PluginInfo,
        capacity: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let plugin_id = info.id;

        // Check if plugin is already loaded
        {
            let runtimes = self.runtimes.read().await;
            if runtimes.contains_key(&plugin_id) {
                return Err(format!("Plugin {plugin_id:?} is already loaded").into());
            }
        }

        // Register the plugin if not already registered
        if !self.registry.is_registered(plugin_id).await {
            self.registry
                .register(info.clone())
                .await
                .map_err(|e| format!("Failed to register plugin: {e}"))?;
        }

        // Create sandbox guard if policy is set
        let sandbox_guard = self
            .sandbox_policy
            .as_ref()
            .map(|policy| SandboxGuard::new(policy.clone()));

        // Create communication channel for the plugin
        let (sender, receiver) = mpsc::channel::<PluginMessage>(capacity);

        // Create runtime entry
        let runtime = PluginRuntime {
            info: info.clone(),
            sender,
            sandbox_guard,
            stats: PluginStats::default(),
        };

        // Store runtime
        {
            let mut runtimes = self.runtimes.write().await;
            runtimes.insert(plugin_id, runtime);
        }

        // Update statistics
        {
            let mut stats = self.dispatch_stats.write().await;
            stats.plugins_loaded += 1;
        }

        // Spawn plugin message processing task
        let runtimes_clone = self.runtimes.clone();
        tokio::spawn(async move {
            Self::plugin_message_loop(plugin_id, runtimes_clone, receiver).await;
        });

        info!("Plugin {:?} ({}) loaded successfully", plugin_id, info.name);
        Ok(())
    }

    /// Unload a plugin
    pub async fn unload_plugin(
        &self,
        plugin_id: PluginId,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Remove from runtimes (which will drop the sender and terminate the message loop)
        let was_loaded = {
            let mut runtimes = self.runtimes.write().await;
            runtimes.remove(&plugin_id).is_some()
        };

        if !was_loaded {
            return Err(format!("Plugin {plugin_id:?} is not loaded").into());
        }

        // Unregister from registry
        self.registry
            .unregister(plugin_id)
            .await
            .map_err(|e| format!("Failed to unregister plugin: {e}"))?;

        // Update statistics
        {
            let mut stats = self.dispatch_stats.write().await;
            stats.plugins_unloaded += 1;
        }

        info!("Plugin {:?} unloaded successfully", plugin_id);
        Ok(())
    }

    /// Dispatch a plugin frame with blocking send
    pub async fn dispatch_plugin_frame(
        &self,
        frame_type: u8,
        header_bytes: Vec<u8>,
    ) -> Result<(), DispatchError> {
        self.dispatch_frame_internal(frame_type, header_bytes, false)
            .await
    }

    /// Dispatch a plugin frame with non-blocking send (for backpressure testing)
    pub async fn dispatch_plugin_framenowait(
        &self,
        frame_type: u8,
        header_bytes: Vec<u8>,
    ) -> Result<(), DispatchError> {
        self.dispatch_frame_internal(frame_type, header_bytes, true)
            .await
    }

    /// Internal dispatch implementation
    async fn dispatch_frame_internal(
        &self,
        frame_type: u8,
        header_bytes: Vec<u8>,
        no_wait: bool,
    ) -> Result<(), DispatchError> {
        // Parse the header
        let header: PluginHeader = ciborium::de::from_reader(&header_bytes[..])
            .map_err(|e| DispatchError::RuntimeError(format!("Header parse error: {e}")))?;

        let plugin_id = header.id;

        // Check if plugin is registered
        if !self.registry.is_registered(plugin_id).await {
            let mut stats = self.dispatch_stats.write().await;
            stats.dispatch_errors += 1;
            return Err(DispatchError::PluginNotRegistered(plugin_id));
        }

        // Check frame-specific permissions
        let required_permission = match frame_type {
            0x51 => Permission::Handshake,      // Handshake frame
            0x52 => Permission::DataAccess,     // Data frame
            0x53 => Permission::Control,        // Control frame
            0x54 => Permission::ErrorReporting, // Error frame
            _ => Permission::DataAccess,        // Default to data access
        };

        if !self
            .registry
            .has_permission(plugin_id, required_permission)
            .await
        {
            let mut stats = self.dispatch_stats.write().await;
            stats.dispatch_errors += 1;
            return Err(DispatchError::PermissionDenied(plugin_id));
        }

        // Get the runtime sender
        let sender = {
            let runtimes = self.runtimes.read().await;
            match runtimes.get(&plugin_id) {
                Some(runtime) => runtime.sender.clone(),
                None => {
                    return Err(DispatchError::RuntimeError(format!(
                        "Plugin {plugin_id:?} runtime not found"
                    )))
                }
            }
        };

        // Create message
        let message = PluginMessage::new(frame_type, header, header_bytes);

        // Send message based on wait mode
        let send_result = if no_wait {
            sender
                .try_send(message)
                .map_err(|e| DispatchError::IpcSendFailed(plugin_id, e.to_string()))
        } else {
            sender
                .send(message)
                .await
                .map_err(|e| DispatchError::IpcSendFailed(plugin_id, e.to_string()))
        };

        match send_result {
            Ok(()) => {
                // Update dispatch statistics
                let mut stats = self.dispatch_stats.write().await;
                stats.frames_dispatched += 1;

                debug!(
                    "Frame type {} dispatched to plugin {:?}",
                    frame_type, plugin_id
                );
                Ok(())
            }
            Err(e) => {
                let mut stats = self.dispatch_stats.write().await;
                stats.dispatch_errors += 1;
                Err(e)
            }
        }
    }

    /// Plugin message processing loop
    async fn plugin_message_loop(
        plugin_id: PluginId,
        runtimes: Arc<RwLock<HashMap<PluginId, PluginRuntime>>>,
        mut receiver: mpsc::Receiver<PluginMessage>,
    ) {
        debug!("Starting message loop for plugin {:?}", plugin_id);

        while let Some(message) = receiver.recv().await {
            // Process the message
            match Self::process_plugin_message(&message).await {
                Ok(()) => {
                    // Update statistics
                    let mut runtimes_guard = runtimes.write().await;
                    if let Some(runtime) = runtimes_guard.get_mut(&plugin_id) {
                        runtime.stats.messages_processed += 1;
                        runtime.stats.bytes_processed += message.size() as u64;
                    }
                }
                Err(e) => {
                    warn!("Error processing message for plugin {:?}: {}", plugin_id, e);
                    // Update error statistics
                    let mut runtimes_guard = runtimes.write().await;
                    if let Some(runtime) = runtimes_guard.get_mut(&plugin_id) {
                        runtime.stats.errors += 1;
                    }
                }
            }
        }

        debug!("Message loop terminated for plugin {:?}", plugin_id);
    }

    /// Process a single plugin message
    async fn process_plugin_message(message: &PluginMessage) -> Result<(), String> {
        match message.frame_type {
            0x51 => {
                // Handshake frame
                debug!("Processing handshake for plugin {:?}", message.plugin_id());
                // Handshake processing logic would go here
            }
            0x52 => {
                // Data frame
                debug!("Processing data frame for plugin {:?}", message.plugin_id());
                // Data processing logic would go here
            }
            0x53 => {
                // Control frame
                debug!(
                    "Processing control frame for plugin {:?}",
                    message.plugin_id()
                );
                // Control processing logic would go here
            }
            0x54 => {
                // Error frame
                debug!(
                    "Processing error frame for plugin {:?}",
                    message.plugin_id()
                );
                // Error processing logic would go here
            }
            _ => {
                warn!(
                    "Unknown frame type {} for plugin {:?}",
                    message.frame_type,
                    message.plugin_id()
                );
            }
        }
        Ok(())
    }

    /// Get plugin statistics
    pub async fn get_plugin_stats(&self, plugin_id: PluginId) -> Option<PluginStats> {
        let runtimes = self.runtimes.read().await;
        runtimes
            .get(&plugin_id)
            .map(|runtime| runtime.stats.clone())
    }

    /// Get dispatcher statistics
    pub async fn get_dispatch_stats(&self) -> DispatchStats {
        let stats = self.dispatch_stats.read().await;
        DispatchStats {
            plugins_loaded: stats.plugins_loaded,
            plugins_unloaded: stats.plugins_unloaded,
            frames_dispatched: stats.frames_dispatched,
            dispatch_errors: stats.dispatch_errors,
        }
    }

    /// Get number of loaded plugins
    pub async fn loaded_plugin_count(&self) -> usize {
        let runtimes = self.runtimes.read().await;
        runtimes.len()
    }

    /// Check if a plugin is loaded
    pub async fn is_plugin_loaded(&self, plugin_id: PluginId) -> bool {
        let runtimes = self.runtimes.read().await;
        runtimes.contains_key(&plugin_id)
    }

    /// Get list of loaded plugin IDs
    pub async fn loaded_plugins(&self) -> Vec<PluginId> {
        let runtimes = self.runtimes.read().await;
        runtimes.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::plugin_registry::PluginRegistry;

    #[tokio::test]
    async fn test_plugin_dispatcher_creation() {
        let registry = Arc::new(PluginRegistry::new());
        let dispatcher = PluginDispatcher::new(registry.clone());

        assert_eq!(dispatcher.loaded_plugin_count().await, 0);
    }

    #[tokio::test]
    async fn test_plugin_load_unload() {
        let registry = Arc::new(PluginRegistry::new());
        let dispatcher = PluginDispatcher::new(registry.clone());

        let plugin_id = PluginId(42);
        let info = PluginInfo::new(plugin_id, "test_plugin", [Permission::DataAccess]);

        // Load plugin
        dispatcher.load_plugin(info).await.unwrap();
        assert!(dispatcher.is_plugin_loaded(plugin_id).await);
        assert_eq!(dispatcher.loaded_plugin_count().await, 1);

        // Unload plugin
        dispatcher.unload_plugin(plugin_id).await.unwrap();
        assert!(!dispatcher.is_plugin_loaded(plugin_id).await);
        assert_eq!(dispatcher.loaded_plugin_count().await, 0);
    }

    #[tokio::test]
    async fn test_dispatch_to_unregistered_plugin() {
        let registry = Arc::new(PluginRegistry::new());
        let dispatcher = PluginDispatcher::new(registry.clone());

        let plugin_id = PluginId(99);
        let header = PluginHeader {
            id: plugin_id,
            flags: 0,
            data: vec![],
        };

        let mut header_bytes = Vec::new();
        ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

        let result = dispatcher.dispatch_plugin_frame(0x52, header_bytes).await;
        assert!(matches!(result, Err(DispatchError::PluginNotRegistered(_))));
    }

    #[tokio::test]
    async fn test_permission_checking() {
        let registry = Arc::new(PluginRegistry::new());
        let dispatcher = PluginDispatcher::new(registry.clone());

        let plugin_id = PluginId(123);
        let info = PluginInfo::new(plugin_id, "limited_plugin", [Permission::Handshake]);

        dispatcher.load_plugin(info).await.unwrap();

        let header = PluginHeader {
            id: plugin_id,
            flags: 0,
            data: vec![],
        };

        let mut header_bytes = Vec::new();
        ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

        // Should succeed for handshake frame (0x51)
        let result = dispatcher
            .dispatch_plugin_frame(0x51, header_bytes.clone())
            .await;
        assert!(result.is_ok());

        // Should fail for data frame (0x52) due to missing DataAccess permission
        let result = dispatcher.dispatch_plugin_frame(0x52, header_bytes).await;
        assert!(matches!(result, Err(DispatchError::PermissionDenied(_))));
    }
}
