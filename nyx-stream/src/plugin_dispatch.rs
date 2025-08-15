#![forbid(unsafe_code)]

//! Plugin frame dispatcher with permission enforcement.
//!
//! The dispatcher routes incoming Plugin Frames (0x50–0x5F) to the appropriate
//! runtime while ensuring the sending plugin has the required permissions.

use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::{
	frame::{
		is_plugin_frame, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_DATA,
		FRAME_TYPE_PLUGIN_ERROR, FRAME_TYPE_PLUGIN_HANDSHAKE,
	},
	plugin_cbor::{parse_plugin_header, PluginCborError, PluginHeader},
	plugin_registry::{Permission, PluginInfo, PluginRegistry},
	PluginId,
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
	#[error(transparent)]
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
	pub fn plugin_id(&self) -> PluginId { self.plugin_header.id }

	/// Check if this is a handshake message
	pub fn is_handshake(&self) -> bool { self.frame_type == FRAME_TYPE_PLUGIN_HANDSHAKE }
	/// Check if this is a control message
	pub fn is_control(&self) -> bool { self.frame_type == FRAME_TYPE_PLUGIN_CONTROL }
	/// Check if this is a data message
	pub fn is_data(&self) -> bool { self.frame_type == FRAME_TYPE_PLUGIN_DATA }
	/// Check if this is an error message
	pub fn is_error(&self) -> bool { self.frame_type == FRAME_TYPE_PLUGIN_ERROR }
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
		debug!(plugin_id = %self.plugin_id, "Aborting plugin runtime");
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
	/// Performs frame validation, permission checking, CBOR parsing
	/// and secure message routing to the plugin process.
	pub async fn dispatch_plugin_frame(
		&self,
		frame_type: u8,
		frame_data: Vec<u8>,
	) -> Result<(), DispatchError> {
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
			_ => Permission::DataAccess,
		};

		if !registry.has_permission(plugin_id, required_permission) {
			let mut stats = self.stats.write().await;
			stats.total_errors += 1;
			warn!(
				plugin_id = %plugin_id,
				?required_permission,
				frame_type = format_args!("0x{:02X}", frame_type),
				"Plugin lacks required permission for frame",
			);
			return Err(DispatchError::InsufficientPermissions(plugin_id));
		}

		drop(registry); // Release registry lock early

		// Get runtime handle and send message
		let runtimes = self.runtimes.lock().await;
		let runtime_handle = runtimes.get(&plugin_id).ok_or_else(|| {
			DispatchError::RuntimeError(plugin_id, "Runtime not found".to_string())
		})?;

		// Create plugin message
		let plugin_message = PluginMessage::new(frame_type, plugin_header, frame_data);

		// Send message via IPC
		runtime_handle
			.ipc_tx
			.send(plugin_message)
			.await
			.map_err(|_| DispatchError::IpcSendFailed(plugin_id, "Channel closed or full".to_string()))?;

		debug!(
			plugin_id = %plugin_id,
			frame_type = format_args!("0x{:02X}", frame_type),
			"Dispatched frame to plugin runtime",
		);
		Ok(())
	}

	/// Legacy method for compatibility - dispatches raw message bytes
	pub async fn dispatch_message(
		&self,
		plugin_id: PluginId,
		message: Vec<u8>,
	) -> Result<(), DispatchError> {
		// Try to parse as CBOR header to extract frame type
		let _plugin_header = parse_plugin_header(&message)?;
		// Assume data frame for legacy compatibility
		let _ = plugin_id; // reserved for future validation paths
		self.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, message).await
	}

	/// Load and start a plugin
	pub async fn load_plugin(&self, plugin_info: PluginInfo) -> Result<(), DispatchError> {
		let plugin_id = plugin_info.id;

		// Capacity check
		{
			let runtimes = self.runtimes.lock().await;
			if runtimes.len() >= 32 {
				return Err(DispatchError::CapacityExceeded(32));
			}
		}

		// Clone for runtime before moving
		let plugin_name = plugin_info.name.clone();

		// Register plugin
		{
			let registry = self.registry.lock().await;
			registry
				.register(plugin_info)
				.await
				.map_err(|e| DispatchError::InvalidFrame(e.to_string()))?;
		}

		// IPC channel
		let (tx, mut rx) = mpsc::channel::<PluginMessage>(1024);

		// Clone shared runtime statistics once for spawned task
		let stats_clone = Arc::clone(&self.stats);

		// Spawn plugin runtime with message processing loop
		let join_handle = tokio::spawn(async move {
			info!(plugin = %plugin_name, id = %plugin_id, "Starting plugin runtime");

			let mut message_count: u64 = 0;
			let mut error_count: u64 = 0;

			while let Some(plugin_message) = rx.recv().await {
				message_count = message_count.saturating_add(1);

				match Self::process_plugin_message(plugin_id, &plugin_message).await {
					Ok(()) => {
						debug!(plugin_id = %plugin_id, msg = message_count, "Processed plugin message");
					}
					Err(e) => {
						error_count = error_count.saturating_add(1);
						error!(plugin_id = %plugin_id, error = %e, "Error processing plugin message");
						{
							let mut stats = stats_clone.write().await;
							stats.total_errors = stats.total_errors.saturating_add(1);
						}
						if error_count > 100 {
							error!(plugin_id = %plugin_id, errors = error_count, "Too many errors, terminating plugin runtime");
							break;
						}
					}
				}

				if message_count % 100 == 0 {
					let mut stats = stats_clone.write().await;
					stats.total_processed_messages = stats.total_processed_messages.saturating_add(100);
				}
			}

			// Update remainder
			{
				let mut stats = stats_clone.write().await;
				stats.total_processed_messages = stats.total_processed_messages.saturating_add(message_count % 100);
			}

			info!(plugin = %plugin_name, id = %plugin_id, processed = message_count, errors = error_count, "Plugin runtime terminated");
		});

		// Store runtime handle
		{
			let mut runtimes = self.runtimes.lock().await;
			runtimes.insert(
				plugin_id,
				RuntimeHandle { join_handle, ipc_tx: tx, plugin_id },
			);
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
		// Remove from runtime map
		let runtime_handle = {
			let mut runtimes = self.runtimes.lock().await;
			runtimes.remove(&plugin_id)
		};

		if let Some(handle) = runtime_handle { handle.abort(); }

		// Unregister plugin
		{
			let mut registry = self.registry.lock().await;
			registry
				.unregister(plugin_id)
				.await
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
	pub async fn get_stats(&self) -> PluginRuntimeStats { self.stats.read().await.clone() }

	/// Shutdown all plugins
	pub async fn shutdown(&self) {
		let plugin_ids: Vec<PluginId> = {
			let runtimes = self.runtimes.lock().await;
			runtimes.keys().cloned().collect()
		};

		for plugin_id in plugin_ids {
			if let Err(e) = self.unload_plugin(plugin_id).await {
				// structured tracing to unify logging behavior
				error!(plugin_id = %plugin_id, error = %e, "Error unloading plugin");
			}
		}
	}

	/// Process individual plugin messages within the runtime
	async fn process_plugin_message(
		plugin_id: PluginId,
		message: &PluginMessage,
	) -> Result<(), DispatchError> {
		match message.frame_type {
			FRAME_TYPE_PLUGIN_HANDSHAKE => {
				debug!(plugin_id = %plugin_id, "Processing handshake message");
				Self::process_handshake_message(plugin_id, message).await
			}
			FRAME_TYPE_PLUGIN_DATA => {
				debug!(plugin_id = %plugin_id, "Processing data message");
				Self::process_data_message(plugin_id, message).await
			}
			FRAME_TYPE_PLUGIN_CONTROL => {
				debug!(plugin_id = %plugin_id, "Processing control message");
				Self::process_control_message(plugin_id, message).await
			}
			FRAME_TYPE_PLUGIN_ERROR => {
				warn!(plugin_id = %plugin_id, "Processing error message");
				Self::process_error_message(plugin_id, message).await
			}
			_ => {
				warn!(
					plugin_id = %plugin_id,
					frame_type = format_args!("0x{:02X}", message.frame_type),
					"Unknown plugin frame type",
				);
				Err(DispatchError::InvalidFrameType(message.frame_type))
			}
		}
	}

	/// Process plugin handshake messages
	async fn process_handshake_message(
		plugin_id: PluginId,
		message: &PluginMessage,
	) -> Result<(), DispatchError> {
		info!(
			plugin_id = %plugin_id,
			bytes = message.plugin_header.data.len(),
			"Plugin completed handshake",
		);
		Ok(())
	}

	/// Process plugin data messages
	async fn process_data_message(
		plugin_id: PluginId,
		message: &PluginMessage,
	) -> Result<(), DispatchError> {
		debug!(
			plugin_id = %plugin_id,
			bytes = message.plugin_header.data.len(),
			"Plugin sent data",
		);
		Ok(())
	}

	/// Process plugin control messages
	async fn process_control_message(
		plugin_id: PluginId,
		message: &PluginMessage,
	) -> Result<(), DispatchError> {
		debug!(
			plugin_id = %plugin_id,
			flags = format_args!("0x{:02X}", message.plugin_header.flags),
			"Plugin sent control message",
		);
		Ok(())
	}

	/// Process plugin error messages
	async fn process_error_message(
		plugin_id: PluginId,
		message: &PluginMessage,
	) -> Result<(), DispatchError> {
		error!(
			plugin_id = %plugin_id,
			error = %String::from_utf8_lossy(&message.plugin_header.data),
			"Plugin reported error",
		);
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

