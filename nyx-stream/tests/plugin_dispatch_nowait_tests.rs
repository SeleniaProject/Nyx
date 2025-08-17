#![forbid(unsafe_code)]

use std::sync::Arc;
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_DATA, FRAME_TYPE_PLUGIN_ERROR, FRAME_TYPE_PLUGIN_HANDSHAKE};
use nyx_stream::plugin_dispatch::{PluginDispatcher, DispatchError};
use nyx_stream::plugin_registry::{PluginRegistry, PluginInfo, Permission};

fn header_bytes(id: PluginId) -> Vec<u8> {
    let h = PluginHeader { id, flags: 0, data: vec![] };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out).expect("serialize header");
    out
}

#[tokio::test]
async fn nowait_rejects_invalid_type() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry);
    let err = dispatcher.dispatch_plugin_frame_nowait(0x40, vec![0]).await.unwrap_err();
    match err { DispatchError::InvalidFrameType(t) => assert_eq!(t, 0x40), e => panic!("{e:?}") }
}

#[tokio::test]
async fn nowait_unregistered_plugin_is_error() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry);
    let pid = PluginId(1001);
    let bytes = header_bytes(pid);
    let err = dispatcher.dispatch_plugin_frame_nowait(FRAME_TYPE_PLUGIN_DATA, bytes).await.unwrap_err();
    match err { DispatchError::PluginNotRegistered(x) => assert_eq!(x, pid), e => panic!("{e:?}") }
}

#[tokio::test]
async fn nowait_permission_enforced_for_each_type() {
    let registry = Arc::new(PluginRegistry::new());
    let pid = PluginId(2002);
    // 登録はするが権限なし
    registry.register(PluginInfo::new(pid, "p", [])).await.unwrap();
    let dispatcher = PluginDispatcher::new(registry.clone());
    // runtime が無いと別エラーになるため、最小容量で起動
    dispatcher.load_plugin_with_capacity(PluginInfo::new(pid, "p", []), 1).await.unwrap_or(());
    let hdr = header_bytes(pid);

    for (ft, name) in [
        (FRAME_TYPE_PLUGIN_HANDSHAKE, "hs"),
        (FRAME_TYPE_PLUGIN_DATA, "data"),
        (FRAME_TYPE_PLUGIN_CONTROL, "ctrl"),
        (FRAME_TYPE_PLUGIN_ERROR, "err"),
    ] {
        let e = dispatcher.dispatch_plugin_frame_nowait(ft, hdr.clone()).await.unwrap_err();
        match e { DispatchError::InsufficientPermissions(x) => assert_eq!(x, pid), other => panic!("{name}: {other:?}") }
    }
}

#[tokio::test]
async fn nowait_backpressure_errors_on_full() {
    let registry = Arc::new(PluginRegistry::new());
    let pid = PluginId(3003);
    let info = PluginInfo::new(pid, "p", [Permission::DataAccess]);
    registry.register(info.clone()).await.unwrap();
    let dispatcher = PluginDispatcher::new(registry.clone());
    dispatcher.load_plugin_with_capacity(info, 1).await.unwrap();
    let bytes = header_bytes(pid);
    // 最初は入る
    dispatcher.dispatch_plugin_frame_nowait(FRAME_TYPE_PLUGIN_DATA, bytes.clone()).await.unwrap();
    // すぐ次は満杯
    let e = dispatcher.dispatch_plugin_frame_nowait(FRAME_TYPE_PLUGIN_DATA, bytes).await.unwrap_err();
    match e { DispatchError::IpcSendFailed(_, _) => {}, o => panic!("{o:?}") }
}
