#![forbid(unsafe_code)]

use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_DATA};
use nyx_stream::plugin_frame::{PluginFrame, PluginFrameDecodeError};

#[test]
fn bounded_decode_rejects_large_buffer() {
    let hdr = PluginHeader { id: PluginId(9), flags: 0, data: vec![1,2,3] };
    let pf = PluginFrame::new(FRAME_TYPE_PLUGIN_DATA, hdr, vec![0xAB; 8]);
    let bytes = pf.to_cbor().unwrap();
    let err = PluginFrame::from_cbor_bounded(&bytes, 4).unwrap_err();
    assert!(matches!(err, PluginFrameDecodeError::Oversize(_)));
}

#[test]
fn checked_decode_succeeds_for_normal_frame() {
    let hdr = PluginHeader { id: PluginId(10), flags: 0, data: vec![0; 8] };
    let pf = PluginFrame::new(FRAME_TYPE_PLUGIN_DATA, hdr.clone(), vec![0u8; 1024]);
    let bytes = pf.to_cbor().unwrap();
    let back = PluginFrame::from_cbor_checked(&bytes).unwrap();
    assert_eq!(back.header, hdr);
    assert_eq!(back.payload.len(), 1024);
}
