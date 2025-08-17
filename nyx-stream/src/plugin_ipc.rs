#![forbid(unsafe_code)]

use crate::plugin::{PluginId, PluginHeader};

/// Abstract IPC transport for plugins. This crate provides only traits/stubs
/// to keep core independent from platform specifics.
pub trait PluginIpcSender: Send + Sync {
	fn send(&self, header: &PluginHeader, frame_type: u8, raw: &[u8]) -> Result<(), String>;
}

pub trait PluginIpcReceiver: Send + Sync {
	fn try_recv(&self) -> Option<(u8, PluginHeader, Vec<u8>)>;
}

/// A no-op sender used in tests.
#[derive(Default, Clone)]
pub struct NoopSender;
impl PluginIpcSender for NoopSender {
	fn send(&self, _header: &PluginHeader, _frame_type: u8, _raw: &[u8]) -> Result<(), String> {
		Ok(())
	}
}

/// Helper to name a plugin for logs.
pub fn format_plugin(p: PluginId, name: &str) -> String { format!("{name}#{p}") }
