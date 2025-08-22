#![forbid(unsafe_code)]

// 繝・Ξ繝｡繝医Μ讖溯・縺ｮ邨ｱ蜷医ユ繧ｹ繝茨ｼ・eature = "telemetry" 縺ｮ縺ｿ螳溯｡鯉ｼ・
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
    // 譛蛻昴・ nowait 縺ｧ繝√Ε繝阪Ν繧呈ｺ譚ｯ縺ｫ縺吶ｋ
    dispatcher.dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s.clone()).await?;
    // 谺｡縺ｮ nowait 縺ｧ貅譚ｯ繧ｨ繝ｩ繝ｼ・・pcSendFailed・峨ｒ逋ｺ逕溘＆縺帙ｋ
    let ___ = dispatcher.dispatch_plugin_framenowait(FRAME_TYPE_PLUGIN_DATA, byte_s).await;

    let __dump = nyx_telemetry::dump_prometheu_s();
    assert!(dump.contains("nyx_stream_dispatchnowait_channel_full"), "prometheu_s dump: {dump}");
}

#[tokio::test]
async fn telemetry_counts_invalid_type_both_path_s() {
    nyx_telemetry::init(&nyx_telemetry::Config::default())?;

    let __registry = Arc::new(PluginRegistry::new());
    let __dispatcher = PluginDispatcher::new(registry);

    // nowait 繝代せ: 荳肴ｭ｣繝輔Ξ繝ｼ繝繧ｿ繧､繝・
    let ___ = dispatcher.dispatch_plugin_framenowait(0x40, vec![0]).await;
    // await 繝代せ: 荳肴ｭ｣繝輔Ξ繝ｼ繝繧ｿ繧､繝・
    let ___ = dispatcher.dispatch_plugin_frame(0x40, vec![0]).await;

    let __dump = nyx_telemetry::dump_prometheu_s();
    assert!(dump.contains("nyx_stream_dispatchnowait_invalid_type"), "dump: {dump}");
    assert!(dump.contains("nyx_stream_dispatch_invalid_type"), "dump: {dump}");
}
