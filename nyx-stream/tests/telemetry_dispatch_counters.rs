#![forbid(unsafe_code)]

// テレメトリ機能の統合テスト（feature = "telemetry" のみ実行）
#![cfg(feature = "telemetry")]

use std::sync::Arc;
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_DATA};
use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{PluginRegistry, PluginInfo, Permission};

fn header_bytes(id: PluginId) -> Vec<u8> {
    let h = PluginHeader { id, flags: 0, data: vec![] };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out).expect("serialize header");
    out
}

#[tokio::test]
async fn telemetry_counts_nowait_channel_full() {
    nyx_telemetry::init(&nyx_telemetry::Config::default()).unwrap();

    let registry = Arc::new(PluginRegistry::new());
    let pid = PluginId(4242);
    let info = PluginInfo::new(pid, "p", [Permission::DataAccess]);
    registry.register(info.clone()).await.unwrap();
    let dispatcher = PluginDispatcher::new(registry.clone());
    dispatcher.load_plugin_with_capacity(info, 1).await.unwrap();

    let bytes = header_bytes(pid);
    // 最初の nowait でチャネルを満杯にする
    dispatcher.dispatch_plugin_frame_nowait(FRAME_TYPE_PLUGIN_DATA, bytes.clone()).await.unwrap();
    // 次の nowait で満杯エラー（IpcSendFailed）を発生させる
    let _ = dispatcher.dispatch_plugin_frame_nowait(FRAME_TYPE_PLUGIN_DATA, bytes).await;

    let dump = nyx_telemetry::dump_prometheus();
    assert!(dump.contains("nyx_stream_dispatch_nowait_channel_full"), "prometheus dump: {dump}");
}

#[tokio::test]
async fn telemetry_counts_invalid_type_both_paths() {
    nyx_telemetry::init(&nyx_telemetry::Config::default()).unwrap();

    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry);

    // nowait パス: 不正フレームタイプ
    let _ = dispatcher.dispatch_plugin_frame_nowait(0x40, vec![0]).await;
    // await パス: 不正フレームタイプ
    let _ = dispatcher.dispatch_plugin_frame(0x40, vec![0]).await;

    let dump = nyx_telemetry::dump_prometheus();
    assert!(dump.contains("nyx_stream_dispatch_nowait_invalid_type"), "dump: {dump}");
    assert!(dump.contains("nyx_stream_dispatch_invalid_type"), "dump: {dump}");
}
