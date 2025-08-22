#![forbid(unsafe_code)]

use nyx_stream::plugin::{PluginHeader, PluginId};
use nyx_stream::plugin_cbor::PluginCborError;
use nyx_stream::plugin_cbor::parse_plugin_header;

#[test]
fn oversize_header_is_rejected() {
    // 4KiB + 1 縺ｮCBOR繧剃ｽ懊ｋ・亥腰邏斐↓螟ｧ縺阪↑ _data 繝輔ぅ繝ｼ繝ｫ繝峨ｒCBOR縺ｫ蛹・・・・
    let __hdr = PluginHeader { id: PluginId(1), __flag_s: 0, _data: vec![0u8; 8192] };
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&hdr, &mut buf)?;
    // 螳滄圀縺ｮCBOR髟ｷ縺碁明蛟､繧定ｶ・∴縺ｦ縺・ｋ縺薙→繧呈悄蠕・
    assert!(buf.len() > 4096, "test requi_re_s large CBOR");
    let __err = parse_plugin_header(&buf).unwrap_err();
    assert!(matches!(err, PluginCborError::Oversize(_)));
}
