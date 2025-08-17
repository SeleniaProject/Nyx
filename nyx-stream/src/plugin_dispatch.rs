#![forbid(unsafe_code)]

//! Plugin frame dispatcher with permission enforcement.
//!
//! The dispatcher routes incoming Plugin Frames (0x50–0x5F) to the appropriate
//! runtime while ensuring the sending plugin has the required permissions.

use std::collections::HashMap;
use std::sync::{Arc, atomic::{AtomicU32, AtomicU64, Ordering}};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};
#[cfg(feature = "telemetry")]
use nyx_telemetry as telemetry;

use crate::{
	plugin::{is_plugin_frame, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_DATA, FRAME_TYPE_PLUGIN_ERROR, FRAME_TYPE_PLUGIN_HANDSHAKE, PluginHeader, PluginId},
	plugin_cbor::{parse_plugin_header, PluginCborError},
	plugin_registry::{Permission, PluginInfo, PluginRegistry},
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
	pub active_plugins: Arc<AtomicU32>,
	pub registered_plugins: Arc<AtomicU32>,
	pub total_dispatched_frames: Arc<AtomicU64>,
	pub total_processed_messages: Arc<AtomicU64>,
	pub total_errors: Arc<AtomicU64>,
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
	registry: Arc<PluginRegistry>,
	runtimes: Arc<Mutex<HashMap<PluginId, RuntimeHandle>>>,
	stats: PluginRuntimeStats,
}

impl PluginDispatcher {
	pub fn new(registry: Arc<PluginRegistry>) -> Self {
		Self {
			registry,
			runtimes: Arc::new(Mutex::new(HashMap::new())),
			stats: PluginRuntimeStats::default(),
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
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_invalid_type", 1);
			return Err(DispatchError::InvalidFrameType(frame_type));
		}

		// Parse CBOR header from frame data
		let plugin_header = match parse_plugin_header(&frame_data) {
			Ok(h) => h,
			Err(e) => {
				self.stats.total_errors.fetch_add(1, Ordering::Relaxed);
				return Err(DispatchError::CborError(e));
			}
		};
		let plugin_id = plugin_header.id;

	// Update statistics (atomic)
	self.stats.total_dispatched_frames.fetch_add(1, Ordering::Relaxed);

		// Check plugin registration and permissions
		if !self.registry.is_registered(plugin_id).await {
			self.stats.total_errors.fetch_add(1, Ordering::Relaxed);
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_unregistered", 1);
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

		if !self.registry.has_permission(plugin_id, required_permission).await {
			self.stats.total_errors.fetch_add(1, Ordering::Relaxed);
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_permission_denied", 1);
			warn!(
				plugin_id = %plugin_id,
				?required_permission,
				frame_type = format_args!("0x{:02X}", frame_type),
				"Plugin lacks required permission for frame",
			);
			return Err(DispatchError::InsufficientPermissions(plugin_id));
		}

		// Get runtime sender (clone) then drop lock before await
		let tx = {
			let runtimes = self.runtimes.lock().await;
			let rh = runtimes.get(&plugin_id).ok_or_else(|| {
				DispatchError::RuntimeError(plugin_id, "Runtime not found".to_string())
			})?;
			rh.ipc_tx.clone()
		};

		// Create plugin message
		let plugin_message = PluginMessage::new(frame_type, plugin_header, frame_data);

		// Send message via IPC (await outside of lock)
		let send_res = tx
			.send(plugin_message)
			.await
			.map_err(|_| DispatchError::IpcSendFailed(plugin_id, "Channel closed or full".to_string()));
		if send_res.is_err() { #[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_ipc_send_failed", 1); }
		send_res?;

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
		self.load_plugin_with_capacity(plugin_info, 1024).await
	}

	/// Load and start a plugin with a specific IPC queue capacity
	pub async fn load_plugin_with_capacity(&self, plugin_info: PluginInfo, capacity: usize) -> Result<(), DispatchError> {
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

		// Register plugin if not already present
		if !self.registry.is_registered(plugin_id).await {
			self.registry
				.register(plugin_info)
				.await
				.map_err(|e| DispatchError::InvalidFrame(e.to_string()))?;
			// Update registered count
			let count = self.registry.count().await as u32;
			self.stats.registered_plugins.store(count, Ordering::Relaxed);
		}


		// IPC channel
		let (tx, mut rx) = mpsc::channel::<PluginMessage>(capacity);

	// Clone shared runtime statistics once for spawned task
	let stats_clone = self.stats.clone();

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
						stats_clone.total_errors.fetch_add(1, Ordering::Relaxed);
						if error_count > 100 {
							error!(plugin_id = %plugin_id, errors = error_count, "Too many errors, terminating plugin runtime");
							break;
						}
					}
				}

				if message_count % 100 == 0 {
					stats_clone.total_processed_messages.fetch_add(100, Ordering::Relaxed);
				}
			}

			// Update remainder
			let rem = message_count % 100;
			if rem > 0 { stats_clone.total_processed_messages.fetch_add(rem, Ordering::Relaxed); }

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
	self.stats.active_plugins.store(self.runtimes.lock().await.len() as u32, Ordering::Relaxed);

		Ok(())
	}

	/// Non-blocking dispatch variant: returns immediately with error if channel is full/closed.
	pub async fn dispatch_plugin_frame_nowait(
		&self,
		frame_type: u8,
		frame_data: Vec<u8>,
	) -> Result<(), DispatchError> {
		if !is_plugin_frame(frame_type) {
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_nowait_invalid_type", 1);
			return Err(DispatchError::InvalidFrameType(frame_type));
		}
		let plugin_header = match parse_plugin_header(&frame_data) {
			Ok(h) => h,
			Err(e) => {
				self.stats.total_errors.fetch_add(1, Ordering::Relaxed);
				return Err(DispatchError::CborError(e));
			}
		};
		let plugin_id = plugin_header.id;
		self.stats.total_dispatched_frames.fetch_add(1, Ordering::Relaxed);
		if !self.registry.is_registered(plugin_id).await {
			self.stats.total_errors.fetch_add(1, Ordering::Relaxed);
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_nowait_unregistered", 1);
			return Err(DispatchError::PluginNotRegistered(plugin_id));
		}
		let required_permission = match frame_type {
			FRAME_TYPE_PLUGIN_HANDSHAKE => Permission::Handshake,
			FRAME_TYPE_PLUGIN_DATA => Permission::DataAccess,
			FRAME_TYPE_PLUGIN_CONTROL => Permission::Control,
			FRAME_TYPE_PLUGIN_ERROR => Permission::ErrorReporting,
			_ => Permission::DataAccess,
		};
		if !self.registry.has_permission(plugin_id, required_permission).await {
			self.stats.total_errors.fetch_add(1, Ordering::Relaxed);
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_nowait_permission_denied", 1);
			return Err(DispatchError::InsufficientPermissions(plugin_id));
		}
		let tx = {
			let runtimes = self.runtimes.lock().await;
			let rh = runtimes.get(&plugin_id).ok_or_else(|| {
				DispatchError::RuntimeError(plugin_id, "Runtime not found".to_string())
			})?;
			rh.ipc_tx.clone()
		};
		let msg = PluginMessage::new(frame_type, plugin_header, frame_data);
		tx.try_send(msg).map_err(|_| {
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_nowait_channel_full", 1);
			DispatchError::IpcSendFailed(plugin_id, "Channel closed or full".to_string())
		})?;
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
		self.registry
			.unregister(plugin_id)
			.await
			.map_err(|_| DispatchError::PluginNotRegistered(plugin_id))?;

		// Update counts
		let count = self.registry.count().await as u32;
		self.stats.registered_plugins.store(count, Ordering::Relaxed);

	// Update stats
	self.stats.active_plugins.store(self.runtimes.lock().await.len() as u32, Ordering::Relaxed);

		Ok(())
	}

	/// Get runtime statistics
	pub async fn get_stats(&self) -> PluginRuntimeStats { self.stats.clone() }

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
		let shown = sanitize_log_bytes(&message.plugin_header.data, 256);
		error!(plugin_id = %plugin_id, error = %shown, "Plugin reported error");
		Ok(())
	}
}

fn sanitize_log_bytes(buf: &[u8], max: usize) -> String {
	use std::borrow::Cow;
	let slice = if buf.len() > max { &buf[..max] } else { buf };
	let cow: Cow<str> = String::from_utf8_lossy(slice);
	cow.chars()
		.map(|c| match c {
			'\n' | '\r' | '\t' => ' ',
			c if c.is_control() => '�',
			c => c,
		})
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::plugin_registry::{PluginRegistry, Permission};
	use crate::plugin::{PluginId, PluginHeader, FRAME_TYPE_PLUGIN_DATA, FRAME_TYPE_PLUGIN_HANDSHAKE};

	#[tokio::test]
	async fn test_plugin_dispatcher_creation() {
	let registry = Arc::new(PluginRegistry::new());
		let dispatcher = PluginDispatcher::new(registry);

		let stats = dispatcher.get_stats().await;
	assert_eq!(stats.total_dispatched_frames.load(Ordering::Relaxed), 0);
	assert_eq!(stats.active_plugins.load(Ordering::Relaxed), 0);
	}

	fn header_bytes(id: PluginId) -> Vec<u8> {
		let h = PluginHeader { id, flags: 0, data: vec![] };
		let mut out = Vec::new();
		ciborium::ser::into_writer(&h, &mut out).expect("serialize header");
		out
	}

	#[tokio::test]
	async fn dispatch_unregistered_returns_error() {
		let registry = Arc::new(PluginRegistry::new());
		let dispatcher = PluginDispatcher::new(registry);
		let pid = PluginId(1);
		let bytes = header_bytes(pid);
		let err = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, bytes).await.expect_err("err");
		match err { DispatchError::PluginNotRegistered(x) => assert_eq!(x, pid), e => panic!("unexpected {e:?}") }
	}

	#[tokio::test]
	async fn dispatch_without_permission_is_denied() {
		let registry = Arc::new(PluginRegistry::new());
		// Register plugin with only Handshake permission
		let pid = PluginId(2);
		registry.register(PluginInfo::new(pid, "p2", [Permission::Handshake])).await.unwrap();

		let dispatcher = PluginDispatcher::new(registry.clone());
		// Start runtime to satisfy runtime lookup
		dispatcher.load_plugin(PluginInfo::new(pid, "p2", [Permission::Handshake])).await.unwrap_or(());

		let bytes = header_bytes(pid);
		let err = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, bytes).await.expect_err("err");
		match err { DispatchError::InsufficientPermissions(x) => assert_eq!(x, pid), e => panic!("unexpected {e:?}") }
	}

	#[tokio::test]
	async fn dispatch_with_permission_succeeds() {
		let registry = Arc::new(PluginRegistry::new());
		let pid = PluginId(3);
		let info = PluginInfo::new(pid, "p3", [Permission::Handshake, Permission::DataAccess]);
		registry.register(info.clone()).await.unwrap();
		let dispatcher = PluginDispatcher::new(registry.clone());
		dispatcher.load_plugin(info).await.unwrap();

		let bytes = header_bytes(pid);
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_HANDSHAKE, bytes.clone()).await.unwrap();
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, bytes).await.unwrap();
	}

	#[tokio::test]
	async fn invalid_frame_type_rejected() {
		let registry = Arc::new(PluginRegistry::new());
		let dispatcher = PluginDispatcher::new(registry);
		let bytes = header_bytes(PluginId(9));
		let err = dispatcher.dispatch_plugin_frame(0x40, bytes).await.unwrap_err();
		match err { DispatchError::InvalidFrameType(t) => assert_eq!(t, 0x40), e => panic!("{e:?}") }
	}

	#[tokio::test]
	async fn invalid_cbor_is_reported() {
		let registry = Arc::new(PluginRegistry::new());
		let dispatcher = PluginDispatcher::new(registry);
		let err = dispatcher.dispatch_plugin_frame(crate::plugin::FRAME_TYPE_PLUGIN_DATA, vec![0xFF, 0x00]).await.unwrap_err();
		match err { DispatchError::CborError(_) => {}, e => panic!("{e:?}") }
	}

	#[tokio::test]
	async fn runtime_missing_is_error() {
		let registry = Arc::new(PluginRegistry::new());
		let pid = PluginId(11);
		let info = PluginInfo::new(pid, "p11", [Permission::DataAccess]);
		registry.register(info.clone()).await.unwrap();
		let dispatcher = PluginDispatcher::new(registry.clone());
		// 故意に runtime を起動しない
		let bytes = header_bytes(pid);
		let err = dispatcher.dispatch_plugin_frame(crate::plugin::FRAME_TYPE_PLUGIN_DATA, bytes).await.unwrap_err();
		match err { DispatchError::RuntimeError(_, _) => {}, e => panic!("{e:?}") }
	}

	#[tokio::test]
	async fn stats_and_counts_update_on_load_unload() {
		let registry = Arc::new(PluginRegistry::new());
		let dispatcher = PluginDispatcher::new(registry.clone());
		let pid = PluginId(21);
		let info = PluginInfo::new(pid, "p21", [Permission::Handshake]);

		dispatcher.load_plugin(info.clone()).await.unwrap();
		let stats = dispatcher.get_stats().await;
		assert_eq!(stats.active_plugins.load(Ordering::Relaxed), 1);
		assert!(registry.is_registered(pid).await);

		// dispatch one handshake frame (increments total_dispatched_frames)
		let bytes = header_bytes(pid);
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_HANDSHAKE, bytes).await.unwrap();
		let stats = dispatcher.get_stats().await;
		assert_eq!(stats.total_dispatched_frames.load(Ordering::Relaxed), 1);

		// unload updates counts
		dispatcher.unload_plugin(pid).await.unwrap();
		let stats = dispatcher.get_stats().await;
		assert_eq!(stats.active_plugins.load(Ordering::Relaxed), 0);
		assert!(!registry.is_registered(pid).await);
	}

	#[test]
	fn sanitize_log_bytes_masks_controls_and_truncates() {
		let input = b"bad\n\t\x01\x02ok";
		let s = sanitize_log_bytes(input, 5);
		// first 5 bytes after mapping control -> ' ' or '�'
		// input mapped: "bad  �"
		assert!(s.len() <= 5);
		assert!(!s.contains('\n'));
		assert!(!s.contains('\t'));
	}

	#[tokio::test]
	async fn double_load_is_idempotent() {
		let registry = Arc::new(PluginRegistry::new());
		let dispatcher = PluginDispatcher::new(registry.clone());
		let pid = PluginId(22);
		let info = PluginInfo::new(pid, "p22", [Permission::Handshake]);

		dispatcher.load_plugin(info.clone()).await.unwrap();
		let s1 = dispatcher.get_stats().await;
		assert_eq!(s1.registered_plugins.load(Ordering::Relaxed), 1);
		assert_eq!(s1.active_plugins.load(Ordering::Relaxed), 1);

		// Load again
		dispatcher.load_plugin(info).await.unwrap_or(());
		let s2 = dispatcher.get_stats().await;
		assert_eq!(s2.registered_plugins.load(Ordering::Relaxed), 1);
		assert_eq!(s2.active_plugins.load(Ordering::Relaxed), 1);
	}

	#[tokio::test]
	async fn shutdown_unloads_all() {
		let registry = Arc::new(PluginRegistry::new());
		let dispatcher = PluginDispatcher::new(registry.clone());

		for i in 30..32 {
			let pid = PluginId(i);
			let info = PluginInfo::new(pid, format!("p{i}"), [Permission::Handshake]);
			dispatcher.load_plugin(info).await.unwrap();
		}
		let before = dispatcher.get_stats().await;
		assert_eq!(before.active_plugins.load(Ordering::Relaxed), 2);

		dispatcher.shutdown().await;
		let after = dispatcher.get_stats().await;
		assert_eq!(after.active_plugins.load(Ordering::Relaxed), 0);
		assert_eq!(registry.count().await, 0);
	}

	#[tokio::test]
	async fn backpressure_try_send_errors_when_full() {
		let registry = Arc::new(PluginRegistry::new());
		let dispatcher = PluginDispatcher::new(registry.clone());
		let pid = PluginId(40);
		let info = PluginInfo::new(pid, "p40", [Permission::DataAccess]);
		registry.register(info.clone()).await.unwrap();
		dispatcher.load_plugin_with_capacity(info, 1).await.unwrap();

		// Build a data header
		let bytes = header_bytes(pid);
		// First nowait should succeed (fills capacity)
		dispatcher.dispatch_plugin_frame_nowait(FRAME_TYPE_PLUGIN_DATA, bytes.clone()).await.unwrap();
		// Second nowait should fail with channel full
		let err = dispatcher.dispatch_plugin_frame_nowait(FRAME_TYPE_PLUGIN_DATA, bytes.clone()).await.unwrap_err();
		match err { DispatchError::IpcSendFailed(_, _) => {}, e => panic!("{e:?}") }

		// send (await) may also back up; to keep test deterministic, give runtime a tick to drain
		tokio::time::sleep(std::time::Duration::from_millis(5)).await;
		// Now send should succeed
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, bytes).await.unwrap();
	}
}

