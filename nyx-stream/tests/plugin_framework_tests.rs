
#![allow(clippy::needless_collect)]

use nyx_stream::plugin::{
	is_plugin_frame, PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_DATA,
	FRAME_TYPE_PLUGIN_ERROR, FRAME_TYPE_PLUGIN_HANDSHAKE,
};
use nyx_stream::plugin_frame::PluginFrame;

// 1) frame type validation
#[test]
fn test_plugin_frame_type_validation() {
	for t in 0x50u8..=0x5F {
		assert!(is_plugin_frame(t), "0x{t:02X} should be recognized as a plugin frame");
	}
	for t in [0x00u8, 0x10, 0x4F, 0x60, 0xFF] {
		assert!(!is_plugin_frame(t), "0x{t:02X} should NOT be a plugin frame");
	}
}

// 2) header CBOR encode/decode roundtrip
#[test]
fn test_plugin_header_cbor_encoding() {
	let hdr = PluginHeader { id: PluginId(42), flags: 0b1010_0001, data: vec![1, 2, 3, 4, 5] };

	let mut buf = Vec::new();
	ciborium::ser::into_writer(&hdr, &mut buf).expect("serialize header");
	let got: PluginHeader = ciborium::de::from_reader(std::io::Cursor::new(&buf)).expect("decode header");

	assert_eq!(hdr, got);
}

// 3) frame build & parse roundtrip
#[test]
fn test_plugin_frame_building_and_parsing() {
	let hdr = PluginHeader { id: PluginId(7), flags: 0, data: b"hello".to_vec() };
	let payload = b"payload-bytes-for-plugin";

	for ft in [
		FRAME_TYPE_PLUGIN_HANDSHAKE,
		FRAME_TYPE_PLUGIN_DATA,
		FRAME_TYPE_PLUGIN_CONTROL,
		FRAME_TYPE_PLUGIN_ERROR,
	] {
		let f = PluginFrame::new(ft, hdr.clone(), payload);
		let serialized = f.to_cbor().expect("serialize frame");
		let parsed = PluginFrame::from_cbor(&serialized).expect("parse frame");
		assert_eq!(f, parsed, "roundtrip should keep frame identical");
		assert!(is_plugin_frame(parsed.frame_type));
	}
}

// 4) basic size limits (sanity): keep encoded size under a reasonable ceiling
//    NOTE: Library does not enforce a hard limit; this test asserts we can
//    encode/decode moderately large frames used by implementations.
#[test]
fn test_plugin_frame_size_limits() {
	let hdr = PluginHeader { id: PluginId(9), flags: 0, data: vec![0u8; 256] };
	let payload = vec![0xABu8; 64 * 1024]; // 64 KiB payload typical upper-bound for control/data

	let f = PluginFrame::new(FRAME_TYPE_PLUGIN_DATA, hdr, payload);
	let encoded = f.to_cbor().expect("encode large frame");
	// keep within 100 KiB as a sanity envelope for CI boxes
	assert!(encoded.len() < 100 * 1024, "encoded size too large: {} bytes", encoded.len());
	let decoded = PluginFrame::from_cbor(&encoded).expect("decode large frame");
	assert_eq!(f, decoded);
}

