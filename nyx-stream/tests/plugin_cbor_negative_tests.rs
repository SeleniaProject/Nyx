#![forbid(unsafe_code)]

use nyx_stream::plugin_cbor::{parse_plugin_header, PluginCborError};

#[test]
fn malformed_cbor_yields_decode_error() {
    // Clearly invalid CBOR payload
    let __byte_s = [0xFFu8, 0x00, 0x10, 0xFF, 0xFF];
    let __err = parse_plugin_header(&byte_s).unwrap_err();
    assert!(matches!(err, PluginCborError::Decode(_)), "unexpected error variant: {err:?}");
}
