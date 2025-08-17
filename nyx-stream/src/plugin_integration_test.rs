#![cfg(test)]
#![forbid(unsafe_code)]

use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::plugin_registry::{PluginRegistry, PluginInfo, Permission};
use crate::plugin_dispatch::PluginDispatcher;
use crate::plugin::PluginId;
use crate::plugin_handshake::{HandshakeInfo, build_handshake_header_bytes, HANDSHAKE_FRAME_TYPE};

#[tokio::test]
async fn e2e_load_and_dispatch() {
	let registry = Arc::new(PluginRegistry::new());
	let dispatcher = PluginDispatcher::new(registry.clone());

	let pid = PluginId(42);
	let info = PluginInfo::new(pid, "geo", [Permission::Handshake, Permission::DataAccess]);
	// load will register and spawn runtime
	dispatcher.load_plugin(info).await.unwrap();

	// handshake
	let h = HandshakeInfo::new(1, "geo");
	let header_bytes = build_handshake_header_bytes(pid, &h).unwrap();
	dispatcher.dispatch_plugin_frame(HANDSHAKE_FRAME_TYPE, header_bytes).await.unwrap();

	// data (reuse empty data header)
	let mut header_bytes2 = Vec::new();
	let header = crate::plugin::PluginHeader { id: pid, flags: 0, data: vec![] };
	ciborium::ser::into_writer(&header, &mut header_bytes2).unwrap();
	dispatcher.dispatch_plugin_frame(crate::plugin::FRAME_TYPE_PLUGIN_DATA, header_bytes2).await.unwrap();

	// small delay to let runtime process queued messages
	sleep(Duration::from_millis(10)).await;
}
