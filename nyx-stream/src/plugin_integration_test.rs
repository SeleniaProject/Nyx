#![cfg(test)]
#![forbid(unsafe_code)]

use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::plugin_registry::{PluginRegistry, PluginInfo, Permission};
use crate::plugin_dispatch::PluginDispatcher;
use crate::plugin::{PluginId, PluginHeader, FRAME_TYPE_PLUGIN_DATA, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_ERROR};
use crate::plugin_handshake::{HandshakeInfo, build_handshake_header_bytes, HANDSHAKE_FRAME_TYPE};

#[tokio::test]
async fn e2e_load_and_dispatch() -> Result<(), Box<dyn std::error::Error>> {
	let registry = Arc::new(PluginRegistry::new());
	let dispatcher = PluginDispatcher::new(registry.clone());

	let pid = PluginId(42);
	let info = PluginInfo::new(pid, "geo", [Permission::Handshake, Permission::DataAccess]);
	// load will register and spawn runtime
	dispatcher.load_plugin(info).await?;

	// handshake
	let h = HandshakeInfo::new(1, "geo");
	let header_bytes = build_handshake_header_bytes(pid, &h)?;
	dispatcher.dispatch_plugin_frame(HANDSHAKE_FRAME_TYPE, header_bytes).await?;

	// data (reuse empty data header)
	let mut header_bytes2 = Vec::new();
	let header = crate::plugin::PluginHeader { id: pid, flags: 0, data: vec![] };
	ciborium::ser::into_writer(&header, &mut header_bytes2)?;
	dispatcher.dispatch_plugin_frame(crate::plugin::FRAME_TYPE_PLUGIN_DATA, header_bytes2).await?;

	// small delay to let runtime process queued messages
	sleep(Duration::from_millis(10)).await;
    Ok(())
}

fn empty_header_bytes(id: PluginId) -> Result<Vec<u8>, ciborium::ser::Error<std::io::Error>> {
	let header = PluginHeader { id, flags: 0, data: vec![] };
	let mut out = Vec::new();
	ciborium::ser::into_writer(&header, &mut out)?;
	Ok(out)
}

#[tokio::test]
async fn e2enowait_with_retry_backoff() -> Result<(), Box<dyn std::error::Error>> {
	let registry = Arc::new(PluginRegistry::new());
	let dispatcher = PluginDispatcher::new(registry.clone());
	let pid = PluginId(55);
	let info = PluginInfo::new(pid, "retry", [Permission::DataAccess]);
	registry.register(info.clone()).await?;
	// Capacity 1 to force backpressure easily
	dispatcher.load_plugin_with_capacity(info, 1).await?;

	let bytes = empty_header_bytes(pid)?;
	// First nowait should succeed and fill queue
	dispatcher.dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, bytes.clone()).await?;

	// Second nowait may fail; implement simple retry with backoff until it succeeds
	let mut attempt = 0u32;
	let mut delay = Duration::from_millis(1);
	loop {
		match dispatcher.dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s.clone()).await {
			Ok(()) => break,
			Err(crate::plugin_dispatch::DispatchError::IpcSendFailed(_, _)) => {
				attempt += 1;
				if attempt > 50 { 
					panic!("exceeded retry attempts under backpressure"); 
				}
				sleep(delay).await;
				// exponential backoff up to a small cap
				delay = Duration::from_millis((delay.as_millis().min(8) as u64).saturating_mul(2)).min(Duration::from_millis(16));
			}
			Err(e) => panic!("unexpected error: {e:?}"),
}

#[tokio::test]
async fn e2e_reconnect_after_unload_reload() -> Result<(), Box<dyn std::error::Error>> {
	let registry = Arc::new(PluginRegistry::new());
	let dispatcher = PluginDispatcher::new(registry.clone());
	let pid = PluginId(66);
	let info = PluginInfo::new(pid, "hs", [Permission::Handshake]);

	dispatcher.load_plugin(info.clone()).await?;
	// Dispatch handshake ok
	let hs = HandshakeInfo::new(1, "hs");
	let hdr = build_handshake_header_bytes(pid, &hs)?;
	dispatcher.dispatch_plugin_frame(HANDSHAKE_FRAME_TYPE, hdr.clone()).await?;

	// Unload runtime (also unregisters)
	dispatcher.unload_plugin(pid).await?;

	// Now dispatch should report not registered
	let err = dispatcher.dispatch_plugin_frame(HANDSHAKE_FRAME_TYPE, hdr.clone()).await.unwrap_err();
	match err { 
		crate::plugin_dispatch::DispatchError::PluginNotRegistered(x) => assert_eq!(x, pid), 
		e => panic!("{e:?}") 
	}

	// Reload and dispatch again
	dispatcher.load_plugin(info).await?;
	dispatcher.dispatch_plugin_frame(HANDSHAKE_FRAME_TYPE, hdr).await?;
    Ok(())
}

#[tokio::test]
async fn e2e_all_frame_types_with_permissions() -> Result<(), Box<dyn std::error::Error>> {
	let registry = Arc::new(PluginRegistry::new());
	let dispatcher = PluginDispatcher::new(registry.clone());
	let pid = PluginId(77);
	let info = PluginInfo::new(pid, "all", [
		Permission::Handshake,
		Permission::DataAccess,
		Permission::Control,
		Permission::ErrorReporting,
	]);
	registry.register(info.clone()).await?;
	dispatcher.load_plugin(info).await?;

	// Handshake
	let hs = HandshakeInfo::new(1, "all");
	let hdr_hs = build_handshake_header_bytes(pid, &hs)?;
	dispatcher.dispatch_plugin_frame(HANDSHAKE_FRAME_TYPE, hdr_hs).await?;

	// Data/Control/Error frames use empty header payload for simplicity
	let empty = empty_header_bytes(pid)?;
	dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, empty.clone()).await?;
	dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, empty.clone()).await?;
	dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_ERROR, empty.clone()).await?;
    Ok(())
}
