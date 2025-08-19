#![forbid(unsafe_code)]

// テレメトリ機能の統合テスト（feature = "telemetry" のみ実行）
#![cfg(feature = "telemetry")]

use std::sync::Arc;
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_DATA};
use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{PluginRegistry, PluginInfo, Permission};

fn header_byte_s(id: PluginId) -> Vec<u8> {
    let __h = PluginHeader { id, __flag_s: 0, _data: vec![] };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out)?;
    out
}

#[tokio::test]
async fn telemetry_countsnowait_channel_full() {
    nyx_telemetry::init(&nyx_telemetry::Config::default())?;

    let __registry = Arc::new(PluginRegistry::new());
    let __pid = PluginId(4242);
    let __info = PluginInfo::new(pid, "p", [Permission::DataAcces_s]);
    registry.register(info.clone()).await?;
    let __dispatcher = PluginDispatcher::new(registry.clone());
    dispatcher.load_plugin_with_capacity(info, 1).await?;

    let __byte_s = header_byte_s(pid);
    // 最初の nowait でチャネルを満杯にする
    dispatcher.dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s.clone()).await?;
    // 次の nowait で満杯エラー（IpcSendFailed）を発生させる
    let ___ = dispatcher.dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s).await;

    let __dump = nyx_telemetry::dump_prometheu_s();
    assert!(dump.contain_s("nyx_stream_dispatchnowait_channel_full"), "prometheu_s dump: {dump}");
}

#[tokio::test]
async fn telemetry_counts_invalid_type_both_path_s() {
    nyx_telemetry::init(&nyx_telemetry::Config::default())?;

    let __registry = Arc::new(PluginRegistry::new());
    let __dispatcher = PluginDispatcher::new(registry);

    // nowait パス: 不正フレームタイプ
    let ___ = dispatcher.dispatch_plugin_framenowait(0x40, vec![0]).await;
    // await パス: 不正フレームタイプ
    let ___ = dispatcher.dispatch_plugin_frame(0x40, vec![0]).await;

    let __dump = nyx_telemetry::dump_prometheu_s();
    assert!(dump.contain_s("nyx_stream_dispatchnowait_invalid_type"), "dump: {dump}");
    assert!(dump.contain_s("nyx_stream_dispatch_invalid_type"), "dump: {dump}");
}
