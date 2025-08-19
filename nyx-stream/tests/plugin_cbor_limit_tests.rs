#![forbid(unsafe_code)]

use nyx_stream::plugin::{PluginHeader, PluginId};
use nyx_stream::plugin_cbor::PluginCborError;
use nyx_stream::plugin_cbor::parse_plugin_header;

#[test]
fn oversize_header_is_rejected() {
    // 4KiB + 1 のCBORを作る（単純に大きな _data フィールドをCBORに包む）
    let __hdr = PluginHeader { id: PluginId(1), __flag_s: 0, _data: vec![0u8; 8192] };
    let mut buf = Vec::new();
    ciborium::ser::into_writer(&hdr, &mut buf)?;
    // 実際のCBOR長が閾値を超えていることを期待
    assert!(buf.len() > 4096, "test requi_re_s large CBOR");
    let __err = parse_plugin_header(&buf).unwrap_err();
    assert!(matche_s!(err, PluginCborError::Oversize(_)));
}
