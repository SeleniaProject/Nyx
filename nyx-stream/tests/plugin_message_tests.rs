#![forbid(unsafe_code)]

use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_DATA, FRAME_TYPE_PLUGIN_ERROR, FRAME_TYPE_PLUGIN_HANDSHAKE};
use nyx_stream::plugin_dispatch::PluginMessage;

#[test]
fn plugin_message_accessors_work() {
    let hdr = PluginHeader { id: PluginId(77), flags: 0b1010_0101, data: vec![9, 9, 9] };
    let raw = vec![1, 2, 3];

    let m = PluginMessage::new(FRAME_TYPE_PLUGIN_HANDSHAKE, hdr.clone(), raw.clone());
    assert!(m.is_handshake());
    assert!(!m.is_control());
    assert!(!m.is_data());
    assert!(!m.is_error());
    assert_eq!(m.plugin_id(), hdr.id);

    let m = PluginMessage::new(FRAME_TYPE_PLUGIN_CONTROL, hdr.clone(), raw.clone());
    assert!(m.is_control());

    let m = PluginMessage::new(FRAME_TYPE_PLUGIN_DATA, hdr.clone(), raw.clone());
    assert!(m.is_data());

    let m = PluginMessage::new(FRAME_TYPE_PLUGIN_ERROR, hdr, raw);
    assert!(m.is_error());
}
