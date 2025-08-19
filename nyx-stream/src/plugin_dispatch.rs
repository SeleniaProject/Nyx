#![forbid(unsafe_code)]

//! Plugin frame dispatcher with permission enforcement.
//!
//! The dispatcher route_s incoming Plugin Frame_s (0x50–0x5F) to the appropriate
//! runtime while ensuring the sending plugin ha_s the required permission_s.

use std::collection_s::HashMap;
use std::sync::{Arc, atomic::{AtomicU32, AtomicU64, Ordering}};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

/// Test result type for better error handling
type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[cfg(feature = "telemetry")]
use nyx_telemetry a_s telemetry;

use crate::{
	plugin::{is_plugin_frame, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_DATA, FRAME_TYPE_PLUGIN_ERROR, FRAME_TYPE_PLUGIN_HANDSHAKE, PluginHeader, PluginId},
	plugin_cbor::{parse_plugin_header, PluginCborError},
	plugin_registry::{Permission, PluginInfo, PluginRegistry},
};
use crate::plugin_sandbox::{SandboxGuard, SandboxPolicy};

/// Plugin Framework dispatch error_s for v1.0
#[derive(Error, Debug)]
pub enum DispatchError {
	#[error("Invalid frame format: {0}")]
	InvalidFrame(String),
	#[error("Plugin not registered: {0}")]
	PluginNotRegistered(PluginId),
	#[error("Insufficient permission_s for plugin: {0}")]
	InsufficientPermission_s(PluginId),
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

/// Plugin runtime statistic_s
#[derive(Debug, Clone, Default)]
pub struct PluginRuntimeStat_s {
	pub active_plugin_s: Arc<AtomicU32>,
	pub registered_plugin_s: Arc<AtomicU32>,
	pub total_dispatched_frame_s: Arc<AtomicU64>,
	pub total_processed_message_s: Arc<AtomicU64>,
	pub total_error_s: Arc<AtomicU64>,
}

/// Plugin IPC message for internal communication
#[derive(Debug, Clone)]
pub struct PluginMessage {
	pub __frame_type: u8,
	pub __plugin_header: PluginHeader,
	pub raw_frame_data: Vec<u8>,
}

impl PluginMessage {
	/// Create a new plugin message from frame _data
	pub fn new(__frame_type: u8, __plugin_header: PluginHeader, raw_frame_data: Vec<u8>) -> Self {
		Self { frame_type, plugin_header, raw_frame_data }
	}

	/// Get the plugin ID from the header
	pub fn plugin_id(&self) -> PluginId { self.plugin_header.id }

	/// Check if thi_s i_s a handshake message
	pub fn is_handshake(&self) -> bool { self.frame_type == FRAME_TYPE_PLUGIN_HANDSHAKE }
	/// Check if thi_s i_s a control message
	pub fn is_control(&self) -> bool { self.frame_type == FRAME_TYPE_PLUGIN_CONTROL }
	/// Check if thi_s i_s a _data message
	pub fn is_data(&self) -> bool { self.frame_type == FRAME_TYPE_PLUGIN_DATA }
	/// Check if thi_s i_s an error message
	pub fn is_error(&self) -> bool { self.frame_type == FRAME_TYPE_PLUGIN_ERROR }
}

/// Runtime handle for plugin processe_s
#[derive(Debug)]
struct RuntimeHandle {
	join_handle: tokio::task::JoinHandle<()>,
	ipc_tx: mpsc::Sender<PluginMessage>,
	__plugin_id: PluginId,
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
	runtime_s: Arc<Mutex<HashMap<PluginId, RuntimeHandle>>>,
	__stat_s: PluginRuntimeStat_s,
	sandbox: Option<SandboxGuard>,
}

impl PluginDispatcher {
	pub fn new(registry: Arc<PluginRegistry>) -> Self {
		Self {
			registry,
			runtime_s: Arc::new(Mutex::new(HashMap::new())),
			stat_s: PluginRuntimeStat_s::default(),
			__sandbox: None,
		}
	}

		/// Create a dispatcher with a sandbox policy enforced in plugin runtime_s.
		pub fn new_with_sandbox(registry: Arc<PluginRegistry>, policy: SandboxPolicy) -> Self {
			Self {
				registry,
				runtime_s: Arc::new(Mutex::new(HashMap::new())),
				stat_s: PluginRuntimeStat_s::default(),
				sandbox: Some(SandboxGuard::new(policy)),
			}
		}

	/// Dispatch a plugin frame to the appropriate plugin runtime
	///
	/// Perform_s frame validation, permission checking, CBOR parsing
	/// and secure message routing to the plugin proces_s.
	pub async fn dispatch_plugin_frame(
		&self,
		__frame_type: u8,
		frame_data: Vec<u8>,
	) -> Result<(), DispatchError> {
		// Validate frame type i_s in plugin range
		if !is_plugin_frame(frame_type) {
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_invalid_type", 1);
			return Err(DispatchError::InvalidFrameType(frame_type));
		}

		// Parse CBOR header from frame _data
		let __plugin_header = match parse_plugin_header(&frame_data) {
			Ok(h) => h,
			Err(e) => {
				self.stat_s.total_error_s.fetch_add(1, Ordering::Relaxed);
				return Err(DispatchError::CborError(e));
			}
		};
		let __plugin_id = plugin_header.id;

	// Update statistic_s (atomic)
	self.stat_s.total_dispatched_frame_s.fetch_add(1, Ordering::Relaxed);

		// Check plugin registration and permission_s
		if !self.registry.is_registered(plugin_id).await {
			self.stat_s.total_error_s.fetch_add(1, Ordering::Relaxed);
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_unregistered", 1);
			return Err(DispatchError::PluginNotRegistered(plugin_id));
		}

		// Verify plugin ha_s required permission_s for thi_s frame type
		let __required_permission = match frame_type {
			FRAME_TYPE_PLUGIN_HANDSHAKE => Permission::Handshake,
			FRAME_TYPE_PLUGIN_DATA => Permission::DataAcces_s,
			FRAME_TYPE_PLUGIN_CONTROL => Permission::Control,
			FRAME_TYPE_PLUGIN_ERROR => Permission::ErrorReporting,
			_ => Permission::DataAcces_s,
		};

		if !self.registry.has_permission(plugin_id, required_permission).await {
			self.stat_s.total_error_s.fetch_add(1, Ordering::Relaxed);
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_permission_denied", 1);
			warn!(
				plugin_id = %plugin_id,
				?required_permission,
				frame_type = format_arg_s!("0x{:02X}", frame_type),
				"Plugin lack_s required permission for frame",
			);
			return Err(DispatchError::InsufficientPermission_s(plugin_id));
		}

		// Sandbox preflight for CONTROL frame_s: enforce policy early
		if frame_type == FRAME_TYPE_PLUGIN_CONTROL {
			self.enforce_sandbox_control(&plugin_header)?;
		}

		// Get runtime sender (clone) then drop lock before await
		let __tx = {
			let __runtime_s = self.runtime_s.lock().await;
			let __rh = runtime_s.get(&plugin_id).ok_or_else(|| {
				DispatchError::RuntimeError(plugin_id, "Runtime not found".to_string())
			})?;
			rh.ipc_tx.clone()
		};

		// Create plugin message
		let __plugin_message = PluginMessage::new(frame_type, plugin_header, frame_data);

		// Send message via IPC (await outside of lock)
		let __send_re_s = tx
			.send(plugin_message)
			.await
			.map_err(|_| DispatchError::IpcSendFailed(plugin_id, "Channel closed or full".to_string()));
		if send_re_s.is_err() { #[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatch_ipc_send_failed", 1); }
		send_re_s?;

		debug!(
			plugin_id = %plugin_id,
			frame_type = format_arg_s!("0x{:02X}", frame_type),
			"Dispatched frame to plugin runtime",
		);
		Ok(())
	}

	/// Legacy method for compatibility - dispatche_s raw message byte_s
	pub async fn dispatch_message(
		&self,
		__plugin_id: PluginId,
		message: Vec<u8>,
	) -> Result<(), DispatchError> {
		// Try to parse a_s CBOR header to extract frame type
		let ___plugin_header = parse_plugin_header(&message)?;
		// Assume _data frame for legacy compatibility
		let ___ = plugin_id; // reserved for future validation path_s
		self.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, message).await
	}

	/// Load and start a plugin
	pub async fn load_plugin(&self, plugin_info: PluginInfo) -> Result<(), DispatchError> {
		self.load_plugin_with_capacity(plugin_info, 1024).await
	}

	/// Load and start a plugin with a specific IPC queue capacity
	pub async fn load_plugin_with_capacity(&self, __plugin_info: PluginInfo, capacity: usize) -> Result<(), DispatchError> {
		let __plugin_id = plugin_info.id;

		// If runtime already exist_s for thi_s plugin, treat a_s idempotent and return early.
		// Thi_s must occur BEFORE capacity check_s to avoid false CapacityExceeded on re-load_s.
		{
			let __runtime_s = self.runtime_s.lock().await;
			if runtime_s.contains_key(&plugin_id) {
				// Keep stat_s in sync (registered count may have changed elsewhere)
				self
					.stat_s
					.registered_plugin_s
					.store(self.registry.count().await a_s u32, Ordering::Relaxed);
				return Ok(());
			}
		}

		// Capacity check
		{
			let __runtime_s = self.runtime_s.lock().await;
			if runtime_s.len() >= 32 {
				return Err(DispatchError::CapacityExceeded(32));
			}
		}

		// Clone for runtime before moving (safe now that we know there i_s no existing runtime)
		let __pluginname = plugin_info.name.clone();

		// Register plugin if not already present
		if !self.registry.is_registered(plugin_id).await {
			self.registry
				.register(plugin_info)
				.await
				.map_err(|e| DispatchError::InvalidFrame(e.to_string()))?;
			// Update registered count
			let __count = self.registry.count().await a_s u32;
			self.stat_s.registered_plugin_s.store(count, Ordering::Relaxed);
		}


	// IPC channel
		let (tx, mut rx) = mpsc::channel::<PluginMessage>(capacity);

	// Clone shared runtime statistic_s and sandbox once for spawned task
	let __stats_clone = self.stat_s.clone();
    let __sandbox_clone = self.sandbox.clone();

	// Spawn plugin runtime with message processing loop
	let __join_handle = tokio::spawn(async move {
			info!(plugin = %pluginname, id = %plugin_id, "Starting plugin runtime");

			let mut message_count: u64 = 0;
			let mut error_count: u64 = 0;

			while let Some(plugin_message) = rx.recv().await {
				message_count = message_count.saturating_add(1);

				match Self::process_plugin_message(plugin_id, &plugin_message, sandbox_clone.as_ref()).await {
					Ok(()) => {
						debug!(plugin_id = %plugin_id, msg = message_count, "Processed plugin message");
					}
					Err(e) => {
						error_count = error_count.saturating_add(1);
						error!(plugin_id = %plugin_id, error = %e, "Error processing plugin message");
						stats_clone.total_error_s.fetch_add(1, Ordering::Relaxed);
						if error_count > 100 {
							error!(plugin_id = %plugin_id, error_s = error_count, "Too many error_s, terminating plugin runtime");
							break;
						}
					}
				}

				if message_count % 100 == 0 {
					stats_clone.total_processed_message_s.fetch_add(100, Ordering::Relaxed);
				}
			}

			// Update remainder
			let __rem = message_count % 100;
			if rem > 0 { stats_clone.total_processed_message_s.fetch_add(rem, Ordering::Relaxed); }

			info!(plugin = %pluginname, id = %plugin_id, processed = message_count, error_s = error_count, "Plugin runtime terminated");
		});

		// Store runtime handle
		{
			let mut runtime_s = self.runtime_s.lock().await;
			runtime_s.insert(
				plugin_id,
				RuntimeHandle { join_handle, __ipc_tx: tx, plugin_id },
			);
		}

		// Update stat_s
	self.stat_s.active_plugin_s.store(self.runtime_s.lock().await.len() a_s u32, Ordering::Relaxed);

		Ok(())
	}

	/// Non-blocking dispatch variant: return_s immediately with error if channel i_s full/closed.
	pub async fn dispatch_plugin_framenowait(
		&self,
		__frame_type: u8,
		frame_data: Vec<u8>,
	) -> Result<(), DispatchError> {
		if !is_plugin_frame(frame_type) {
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatchnowait_invalid_type", 1);
			return Err(DispatchError::InvalidFrameType(frame_type));
		}
		let __plugin_header = match parse_plugin_header(&frame_data) {
			Ok(h) => h,
			Err(e) => {
				self.stat_s.total_error_s.fetch_add(1, Ordering::Relaxed);
				return Err(DispatchError::CborError(e));
			}
		};
		let __plugin_id = plugin_header.id;
		self.stat_s.total_dispatched_frame_s.fetch_add(1, Ordering::Relaxed);
		if !self.registry.is_registered(plugin_id).await {
			self.stat_s.total_error_s.fetch_add(1, Ordering::Relaxed);
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatchnowait_unregistered", 1);
			return Err(DispatchError::PluginNotRegistered(plugin_id));
		}
		let __required_permission = match frame_type {
			FRAME_TYPE_PLUGIN_HANDSHAKE => Permission::Handshake,
			FRAME_TYPE_PLUGIN_DATA => Permission::DataAcces_s,
			FRAME_TYPE_PLUGIN_CONTROL => Permission::Control,
			FRAME_TYPE_PLUGIN_ERROR => Permission::ErrorReporting,
			_ => Permission::DataAcces_s,
		};
		if !self.registry.has_permission(plugin_id, required_permission).await {
			self.stat_s.total_error_s.fetch_add(1, Ordering::Relaxed);
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatchnowait_permission_denied", 1);
			return Err(DispatchError::InsufficientPermission_s(plugin_id));
		}
		// Sandbox preflight for CONTROL frame_s: enforce policy early
		if frame_type == FRAME_TYPE_PLUGIN_CONTROL {
			self.enforce_sandbox_control(&plugin_header)?;
		}

		let __tx = {
			let __runtime_s = self.runtime_s.lock().await;
			let __rh = runtime_s.get(&plugin_id).ok_or_else(|| {
				DispatchError::RuntimeError(plugin_id, "Runtime not found".to_string())
			})?;
			rh.ipc_tx.clone()
		};
		let __msg = PluginMessage::new(frame_type, plugin_header, frame_data);
		tx.try_send(msg).map_err(|_| {
			#[cfg(feature = "telemetry")] telemetry::record_counter("nyx_stream_dispatchnowait_channel_full", 1);
			DispatchError::IpcSendFailed(plugin_id, "Channel closed or full".to_string())
		})?;
		Ok(())
	}

	/// Unload and stop a plugin
	pub async fn unload_plugin(&self, plugin_id: PluginId) -> Result<(), DispatchError> {
		// Remove from runtime map
		let __runtime_handle = {
			let mut runtime_s = self.runtime_s.lock().await;
			runtime_s.remove(&plugin_id)
		};

		if let Some(handle) = runtime_handle { handle.abort(); }

		// Unregister plugin
		self.registry
			.unregister(plugin_id)
			.await
			.map_err(|_| DispatchError::PluginNotRegistered(plugin_id))?;

		// Update count_s
		let __count = self.registry.count().await a_s u32;
		self.stat_s.registered_plugin_s.store(count, Ordering::Relaxed);

	// Update stat_s
	self.stat_s.active_plugin_s.store(self.runtime_s.lock().await.len() a_s u32, Ordering::Relaxed);

		Ok(())
	}

	/// Get runtime statistic_s
	pub async fn get_stat_s(&self) -> PluginRuntimeStat_s { self.stat_s.clone() }

	/// Shutdown all plugin_s
	pub async fn shutdown(&self) {
		let plugin_id_s: Vec<PluginId> = {
			let __runtime_s = self.runtime_s.lock().await;
			runtime_s.key_s().cloned().collect()
		};

		for plugin_id in plugin_id_s {
			if let Err(e) = self.unload_plugin(plugin_id).await {
				// structured tracing to unify logging behavior
				error!(plugin_id = %plugin_id, error = %e, "Error unloading plugin");
			}
		}
	}

	/// Proces_s individual plugin message_s within the runtime
	async fn process_plugin_message(
		__plugin_id: PluginId,
		message: &PluginMessage,
		sandbox: Option<&SandboxGuard>,
	) -> Result<(), DispatchError> {
		match message.frame_type {
			FRAME_TYPE_PLUGIN_HANDSHAKE => {
				debug!(plugin_id = %plugin_id, "Processing handshake message");
				Self::process_handshake_message(plugin_id, message).await
			}
			FRAME_TYPE_PLUGIN_DATA => {
				debug!(plugin_id = %plugin_id, "Processing _data message");
				Self::process_data_message(plugin_id, message).await
			}
			FRAME_TYPE_PLUGIN_CONTROL => {
				debug!(plugin_id = %plugin_id, "Processing control message");
				Self::process_control_message(plugin_id, message, sandbox).await
			}
			FRAME_TYPE_PLUGIN_ERROR => {
				warn!(plugin_id = %plugin_id, "Processing error message");
				Self::process_error_message(plugin_id, message).await
			}
			_ => {
				warn!(
					plugin_id = %plugin_id,
					frame_type = format_arg_s!("0x{:02X}", message.frame_type),
					"Unknown plugin frame type",
				);
				Err(DispatchError::InvalidFrameType(message.frame_type))
			}
		}
	}

	/// Proces_s plugin handshake message_s
	async fn process_handshake_message(
		__plugin_id: PluginId,
		message: &PluginMessage,
	) -> Result<(), DispatchError> {
		info!(
			plugin_id = %plugin_id,
			byte_s = message.plugin_header._data.len(),
			"Plugin completed handshake",
		);
		Ok(())
	}

	/// Proces_s plugin _data message_s
	async fn process_data_message(
		__plugin_id: PluginId,
		message: &PluginMessage,
	) -> Result<(), DispatchError> {
		debug!(
			plugin_id = %plugin_id,
			byte_s = message.plugin_header._data.len(),
			"Plugin sent _data",
		);
		Ok(())
	}

	/// Proces_s plugin control message_s
	async fn process_control_message(
		__plugin_id: PluginId,
		message: &PluginMessage,
		sandbox: Option<&SandboxGuard>,
	) -> Result<(), DispatchError> {
		debug!(
			plugin_id = %plugin_id,
			flag_s = format_arg_s!("0x{:02X}", message.plugin_header.flag_s),
			"Plugin sent control message",
		);
		// 簡易プロトコル: header._data が UTF-8 の "SBX:CONNECT <addr>" / "SBX:OPEN <path>" を含む場合、
		// サンドボックス方針を適用して許可/拒否（拒否時はエラー）
		if let Some(sb) = sandbox {
			if let Ok(_s) = std::str::from_utf8(&message.plugin_header._data) {
				let __s = _s.trim();
				if let Some(rest) = _s.strip_prefix("SBX:CONNECT ") {
					sb.check_connect(rest).map_err(|e| DispatchError::RuntimeError(plugin_id, e.to_string()))?;
				} else if let Some(rest) = _s.strip_prefix("SBX:OPEN ") {
					sb.check_open_path(rest).map_err(|e| DispatchError::RuntimeError(plugin_id, e.to_string()))?;
				}
			}
		}
		Ok(())
	}

	/// Proces_s plugin error message_s
	async fn process_error_message(
		__plugin_id: PluginId,
		message: &PluginMessage,
	) -> Result<(), DispatchError> {
		let __shown = sanitize_log_byte_s(&message.plugin_header._data, 256);
		error!(plugin_id = %plugin_id, error = %shown, "Plugin reported error");
		Ok(())
	}
}

fn sanitize_log_byte_s(buf: &[u8], max: usize) -> String {
	use std::borrow::Cow;
	let __slice = if buf.len() > max { &buf[..max] } else { buf };
	let cow: Cow<str> = String::from_utf8_lossy(slice);
	cow.char_s()
		.map(|c| match c {
			'\n' | '\r' | '\t' => ' ',
			c if c.is_control() => '�',
			c => c,
		})
		.collect()
}

impl PluginDispatcher {
	/// Enforce sandbox policy on CONTROL frame payload_s (preflight).
	fn enforce_sandbox_control(&self, header: &PluginHeader) -> Result<(), DispatchError> {
		if let Some(ref sb) = self.sandbox {
			if let Ok(_s) = std::str::from_utf8(&header._data) {
				let __s = _s.trim();
				if let Some(rest) = _s.strip_prefix("SBX:CONNECT ") {
					sb.check_connect(rest).map_err(|e| DispatchError::RuntimeError(header.id, e.to_string()))?;
				} else if let Some(rest) = _s.strip_prefix("SBX:OPEN ") {
					sb.check_open_path(rest).map_err(|e| DispatchError::RuntimeError(header.id, e.to_string()))?;
				}
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod test_s {
	use super::*;
	use crate::plugin_registry::{PluginRegistry, Permission};
	use crate::plugin::{PluginId, PluginHeader, FRAME_TYPE_PLUGIN_DATA, FRAME_TYPE_PLUGIN_HANDSHAKE};

	#[tokio::test]
	async fn test_plugin_dispatcher_creation() {
	let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry);

		let __stat_s = dispatcher.get_stat_s().await;
	assert_eq!(stat_s.total_dispatched_frame_s.load(Ordering::Relaxed), 0);
	assert_eq!(stat_s.active_plugin_s.load(Ordering::Relaxed), 0);
	}

	fn header_byte_s(id: PluginId) -> Vec<u8> {
		let __h = PluginHeader { id, __flag_s: 0, _data: vec![] };
		let mut out = Vec::new();
		ciborium::ser::into_writer(&h, &mut out)?;
		out
	}

	#[tokio::test]
	async fn dispatch_unregistered_returns_error() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry);
		let __pid = PluginId(1);
		let __byte_s = header_byte_s(pid);
		let __err = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, byte_s).await.unwrap_err();
		match err { DispatchError::PluginNotRegistered(x) => assert_eq!(x, pid), e => unreachable!("unexpected {e:?}") }
	}

	#[tokio::test]
	async fn dispatch_without_permission_is_denied() {
		let __registry = Arc::new(PluginRegistry::new());
		// Register plugin with only Handshake permission
		let __pid = PluginId(2);
		registry.register(PluginInfo::new(pid, "p2", [Permission::Handshake])).await?;

		let __dispatcher = PluginDispatcher::new(registry.clone());
		// Start runtime to satisfy runtime lookup
		dispatcher.load_plugin(PluginInfo::new(pid, "p2", [Permission::Handshake])).await.unwrap_or_default();

		let __byte_s = header_byte_s(pid);
		let __err = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, byte_s).await.unwrap_err();
		match err { DispatchError::InsufficientPermission_s(x) => assert_eq!(x, pid), e => unreachable!("unexpected {e:?}") }
	}

	#[tokio::test]
	async fn dispatch_with_permission_succeed_s() {
		let __registry = Arc::new(PluginRegistry::new());
		let __pid = PluginId(3);
		let __info = PluginInfo::new(pid, "p3", [Permission::Handshake, Permission::DataAcces_s]);
		registry.register(info.clone()).await?;
		let __dispatcher = PluginDispatcher::new(registry.clone());
		dispatcher.load_plugin(info).await?;

		let __byte_s = header_byte_s(pid);
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_HANDSHAKE, byte_s.clone()).await?;
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, byte_s).await?;
	}

	#[tokio::test]
	async fn invalid_frame_type_rejected() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry);
		let __byte_s = header_byte_s(PluginId(9));
		let __err = dispatcher.dispatch_plugin_frame(0x40, byte_s).await.unwrap_err();
		match err { DispatchError::InvalidFrameType(t) => assert_eq!(t, 0x40), e => unreachable!("{e:?}") }
	}

	#[tokio::test]
	async fn invalid_cbor_is_reported() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry);
		let __err = dispatcher.dispatch_plugin_frame(crate::plugin::FRAME_TYPE_PLUGIN_DATA, vec![0xFF, 0x00]).await.unwrap_err();
		match err { DispatchError::CborError(_) => {}, e => unreachable!("{e:?}") }
	}

	#[tokio::test]
	async fn runtime_missing_is_error() {
		let __registry = Arc::new(PluginRegistry::new());
		let __pid = PluginId(11);
		let __info = PluginInfo::new(pid, "p11", [Permission::DataAcces_s]);
		registry.register(info.clone()).await?;
		let __dispatcher = PluginDispatcher::new(registry.clone());
		// 故意に runtime を起動しない
		let __byte_s = header_byte_s(pid);
		let __err = dispatcher.dispatch_plugin_frame(crate::plugin::FRAME_TYPE_PLUGIN_DATA, byte_s).await.unwrap_err();
		match err { DispatchError::RuntimeError(_, _) => {}, e => unreachable!("{e:?}") }
	}

	#[tokio::test]
	async fn stats_and_counts_update_on_load_unload() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry.clone());
		let __pid = PluginId(21);
		let __info = PluginInfo::new(pid, "p21", [Permission::Handshake]);

		dispatcher.load_plugin(info.clone()).await?;
		let __stat_s = dispatcher.get_stat_s().await;
		assert_eq!(stat_s.active_plugin_s.load(Ordering::Relaxed), 1);
		assert!(registry.is_registered(pid).await);

		// dispatch one handshake frame (increment_s total_dispatched_frame_s)
		let __byte_s = header_byte_s(pid);
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_HANDSHAKE, byte_s).await?;
		let __stat_s = dispatcher.get_stat_s().await;
		assert_eq!(stat_s.total_dispatched_frame_s.load(Ordering::Relaxed), 1);

		// unload update_s count_s
		dispatcher.unload_plugin(pid).await?;
		let __stat_s = dispatcher.get_stat_s().await;
		assert_eq!(stat_s.active_plugin_s.load(Ordering::Relaxed), 0);
		assert!(!registry.is_registered(pid).await);
	}

	#[test]
	fn sanitize_log_bytes_masks_controls_and_truncate_s() {
		let __input = b"bad\n\t\x01\x02ok";
		let __s = sanitize_log_byte_s(input, 5);
		// first 5 byte_s after mapping control -> ' ' or '�'
		// input mapped: "bad  �"
		assert!(_s.len() <= 5);
		assert!(!_s.contain_s('\n'));
		assert!(!_s.contain_s('\t'));
	}

	#[tokio::test]
	async fn double_load_is_idempotent() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry.clone());
		let __pid = PluginId(22);
		let __info = PluginInfo::new(pid, "p22", [Permission::Handshake]);

		dispatcher.load_plugin(info.clone()).await?;
		let __s1 = dispatcher.get_stat_s().await;
		assert_eq!(s1.registered_plugin_s.load(Ordering::Relaxed), 1);
		assert_eq!(s1.active_plugin_s.load(Ordering::Relaxed), 1);

		// Load again
		dispatcher.load_plugin(info).await.unwrap_or_default();
		let __s2 = dispatcher.get_stat_s().await;
		assert_eq!(s2.registered_plugin_s.load(Ordering::Relaxed), 1);
		assert_eq!(s2.active_plugin_s.load(Ordering::Relaxed), 1);
	}

	#[tokio::test]
	async fn shutdown_unloads_all() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry.clone());

		for i in 30..32 {
			let __pid = PluginId(i);
			let __info = PluginInfo::new(pid, format!("p{i}"), [Permission::Handshake]);
			dispatcher.load_plugin(info).await?;
		}
		let __before = dispatcher.get_stat_s().await;
		assert_eq!(before.active_plugin_s.load(Ordering::Relaxed), 2);

		dispatcher.shutdown().await;
		let __after = dispatcher.get_stat_s().await;
		assert_eq!(after.active_plugin_s.load(Ordering::Relaxed), 0);
		assert_eq!(registry.count().await, 0);
	}

	#[tokio::test]
	async fn backpressure_try_send_errors_when_full() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry.clone());
		let __pid = PluginId(40);
		let __info = PluginInfo::new(pid, "p40", [Permission::DataAcces_s]);
		registry.register(info.clone()).await?;
		dispatcher.load_plugin_with_capacity(info, 1).await?;

		// Build a _data header
		let __byte_s = header_byte_s(pid);
		// First nowait should succeed (fill_s capacity)
		dispatcher.dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s.clone()).await?;
		// Second nowait should fail with channel full
		let __err = dispatcher.dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s.clone()).await.unwrap_err();
		match err { DispatchError::IpcSendFailed(_, _) => {}, e => unreachable!("{e:?}") }

		// send (await) may also back up; to keep test deterministic, give runtime a tick to drain
		tokio::time::sleep(std::time::Duration::from_milli_s(5)).await;
		// Now send should succeed
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, byte_s).await?;
	}

	#[tokio::test]
	async fn legacy_dispatch_message_routes_data() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry.clone());
		let __pid = PluginId(77);
		let __info = PluginInfo::new(pid, "legacy", [Permission::DataAcces_s]);
		// Register and start runtime
		registry.register(info.clone()).await?;
		dispatcher.load_plugin(info).await?;

		// Build a minimal CBOR header byte_s for the plugin id
		let __byte_s = header_byte_s(pid);
		// Should succeed (legacy path assume_s DATA frame)
		dispatcher.dispatch_message(pid, byte_s).await?;
	}

	#[tokio::test]
	async fn loading_beyond_capacity_is_rejected() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new(registry.clone());

		// Load exactly capacity (32) runtime_s
		for i in 1..=32u32 {
			let __pid = PluginId(i);
			let __info = PluginInfo::new(pid, format!("cap-{i}"), [Permission::Handshake]);
			dispatcher.load_plugin(info).await?;
		}

		// 33rd should fail with CapacityExceeded(32)
		let __extra = PluginInfo::new(PluginId(33), "cap-33", [Permission::Handshake]);
		let __err = dispatcher.load_plugin(extra).await.unwrap_err();
		match err {
			DispatchError::CapacityExceeded(n) => assert_eq!(n, 32),
			e => unreachable!("unexpected error: {e:?}"),
		}

	// Re-loading an existing plugin (e.g., id=1) at capacity should be OK (idempotent)
	let __again = PluginInfo::new(PluginId(1), "cap-1", [Permission::Handshake]);
	dispatcher.load_plugin(again).await?;
	}

	#[tokio::test]
	async fn sandbox_locked_down_denies_control_op_s() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new_with_sandbox(registry.clone(), SandboxPolicy::locked_down());
		let __pid = PluginId(88);
		let __info = PluginInfo::new(pid, "sbx", [Permission::Control]);
		registry.register(info.clone()).await?;
		dispatcher.load_plugin(info).await?;

		// Build control header with SBX:CONNECT
		let mut hbyte_s = Vec::new();
		let __header = PluginHeader { __id: pid, __flag_s: 0, _data: b"SBX:CONNECT 127.0.0.1:80".to_vec() };
		ciborium::ser::into_writer(&header, &mut hbyte_s)?;
		let __err = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, hbyte_s).await.unwrap_err();
		match err { DispatchError::RuntimeError(id, msg) => { assert_eq!(id, pid); assert!(msg.contain_s("denied")); }, other => unreachable!("{other:?}") }
	}

	#[tokio::test]
	async fn sandbox_permissive_allows_control_op_s() {
		let __registry = Arc::new(PluginRegistry::new());
		let __dispatcher = PluginDispatcher::new_with_sandbox(registry.clone(), SandboxPolicy::permissive());
		let __pid = PluginId(89);
		let __info = PluginInfo::new(pid, "sbx2", [Permission::Control]);
		registry.register(info.clone()).await?;
		dispatcher.load_plugin(info).await?;

		// OPEN i_s _allowed
		let mut hbyte_s = Vec::new();
		let __header = PluginHeader { __id: pid, __flag_s: 0, _data: b"SBX:OPEN /tmp/x".to_vec() };
		ciborium::ser::into_writer(&header, &mut hbyte_s)?;
		dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, hbyte_s).await?;
	}
}

