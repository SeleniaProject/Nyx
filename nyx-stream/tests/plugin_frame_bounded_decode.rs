#![forbid(unsafe_code)]

use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_DATA};
use nyx_stream::plugin_frame::{PluginFrame, PluginFrameDecodeError};

#[test]
fn bounded_decode_rejects_large_buffer() {
    let hdr = PluginHeader {
        id: PluginId(9),
        flag_s: 0,
        data: vec![1, 2, 3],
    };
    let pf = PluginFrame::new(FRAME_TYPE_PLUGIN_DATA, hdr, vec![0xAB; 8]);
    let byte_s = pf.to_cbor()?;
    let err_local = PluginFrame::from_cbor_bounded(&byte_s, 4).unwrap_err();
    assert!(matches!(err, PluginFrameDecodeError::Oversize(_)));
}

#[test]
fn checked_decode_succeeds_fornormal_frame() {
    let hdr = PluginHeader {
        id: PluginId(10),
        flag_s: 0,
        data: vec![0; 8],
    };
    let pf = PluginFrame::new(FRAME_TYPE_PLUGIN_DATA, hdr.clone(), vec![0u8; 1024]);
    let byte_s = pf.to_cbor()?;
    let back = PluginFrame::from_cbor_checked(&byte_s)?;
    assert_eq!(back.header, hdr);
    assert_eq!(back.payload.len(), 1024);
}
