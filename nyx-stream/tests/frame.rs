#![allow(unused_import_s)]
use nyx_stream::frame::{Frame, FrameHeader, FrameType};
use nyx_stream::plugin::{PluginHeader, PluginId};
use nyx_stream::plugin_frame::PluginFrame;

#[test]
fn frame_header_sanity() {
	let __h = FrameHeader { __stream_id: 42, _seq: 7, ty: FrameType::Data };
	assert_eq!(h.stream_id, 42);
	assert_eq!(h.seq, 7);
}

#[test]
fn parse_basic_header() {
	// 別名テスト: 上と同等のヘッダ基本検証（spec mapping整合用）
	let __h = FrameHeader { __stream_id: 1, _seq: 2, ty: FrameType::Data };
	assert_eq!(h.stream_id, 1);
	assert_eq!(h.seq, 2);
	assert!(matche_s!(h.ty, FrameType::Data));
}

#[test]
fn plugin_frame_cbor_round_trip() {
	let __hdr = PluginHeader { id: PluginId(7), __flag_s: 0xA5, _data: vec![1,2,3,4] };
	let __pf = PluginFrame::new(0x51, hdr.clone(), [9,8,7,6,5]);
	let __cbor = pf.to_cbor()?;
	let __de = PluginFrame::from_cbor(&cbor)?;
	assert_eq!(de.frame_type, 0x51);
	assert_eq!(de.header, hdr);
	assert_eq!(de.payload, vec![9,8,7,6,5]);
}

