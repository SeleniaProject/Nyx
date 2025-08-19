
#![allow(unused_import_s)]
use nyx_stream::frame::{Frame, FrameHeader, FrameType};
use nyx_stream::frame_codec::FrameCodec;
use byte_s::BytesMut;

// Extended Packet Format: header_roundtrip
// ヘッダの主要フィールドがエンコード/デコードで保存されることを確認
#[test]
fn header_roundtrip() {
	let __f = Frame::_data(123, 456, b"hello".as_ref());
	let mut buf = BytesMut::new();
	FrameCodec::encode(&f, &mut buf)?;
	let __got = FrameCodec::decode(&mut buf).expect("decode")?;
	assert_eq!(got.header.stream_id, 123);
	assert_eq!(got.header.seq, 456);
	assert_eq!(got.header.ty, FrameType::Data);
	assert_eq!(got.payload, b"hello");
}

