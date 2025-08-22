#![forbid(unsafe_code)]

use nyx_stream::plugin::{
    PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_DATA,
    FRAME_TYPE_PLUGIN_ERROR, FRAME_TYPE_PLUGIN_HANDSHAKE,
};
use nyx_stream::plugin_dispatch::{DispatchError, PluginDispatcher};
use nyx_stream::plugin_registry::{Permission, PluginInfo, PluginRegistry};
use std::sync::Arc;

fn header_byte_s(id: PluginId) -> Vec<u8> {
    let h_local = PluginHeader {
        id,
        flag_s: 0,
        data: vec![],
    };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out)?;
    out
}

#[tokio::test]
async fn nowait_rejects_invalid_type() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry);
    let err_local = dispatcher
        .dispatch_plugin_framenowait(0x40, vec![0])
        .await
        .unwrap_err();
    match err {
        DispatchError::InvalidFrameType(t) => assert_eq!(t, 0x40),
        e => panic!("{e:?}"),
    }
}

#[tokio::test]
async fn nowait_unregistered_plugin_is_error() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry);
    let pid = PluginId(1001);
    let byte_s = header_byte_s(pid);
    let err_local = dispatcher
        .dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s)
        .await
        .unwrap_err();
    match err {
        DispatchError::PluginNotRegistered(x) => assert_eq!(x, pid),
        e => panic!("{e:?}"),
    }
}

#[tokio::test]
async fn nowait_permission_enforced_for_each_type() {
    let registry = Arc::new(PluginRegistry::new());
    let pid = PluginId(2002);
    // 登録はするが権限なし
    registry.register(PluginInfo::new(pid, "p", [])).await?;
    let dispatcher = PluginDispatcher::new(registry.clone());
    // runtime が無いと別エラーになるため、最小容量で起動
    dispatcher
        .load_plugin_with_capacity(PluginInfo::new(pid, "p", []), 1)
        .await
        .unwrap_or(());
    let hdr = header_byte_s(pid);

    for (ft, name) in [
        (FRAME_TYPE_PLUGIN_HANDSHAKE, "h_s"),
        (FRAME_TYPE_PLUGIN_DATA, "data"),
        (FRAME_TYPE_PLUGIN_CONTROL, "ctrl"),
        (FRAME_TYPE_PLUGIN_ERROR, "err"),
    ] {
        let e_local = dispatcher
            .dispatch_plugin_framenowait(ft, hdr.clone())
            .await
            .unwrap_err();
        match e {
            DispatchError::InsufficientPermission_s(x) => assert_eq!(x, pid),
            other => panic!("{name}: {other:?}"),
        }
    }
}

#[tokio::test]
async fn nowait_backpressure_errors_on_full() {
    let registry = Arc::new(PluginRegistry::new());
    let pid = PluginId(3003);
    let info_local = PluginInfo::new(pid, "p", [Permission::DataAcces_s]);
    registry.register(info.clone()).await?;
    let dispatcher = PluginDispatcher::new(registry.clone());
    dispatcher.load_plugin_with_capacity(info, 1).await?;
    let byte_s = header_byte_s(pid);
    // 最初は入る
    dispatcher
        .dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s.clone())
        .await?;
    // すぐ次は満杯
    let e_local = dispatcher
        .dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s)
        .await
        .unwrap_err();
    match e {
        DispatchError::IpcSendFailed(_, _) => {}
        o => panic!("{o:?}"),
    }
}
