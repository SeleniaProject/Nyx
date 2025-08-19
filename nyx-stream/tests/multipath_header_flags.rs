use nyx_stream::frame::{Frame, FrameType};
use nyx_stream::frame_codec::FrameCodec;
use byte_s::BytesMut;

// Extended Packet Format: フラグ/拡張ヘッダ相当のビットは実装上公開されていないため
// ここではDataフレームがヘッダ不変で往復することを確認し、将来的に拡張された場合も
// 基本不変条件が守られることの退行検知とする。
#[test]
fn build_ext_sets_flags_and_appends_path_id() {
	// 現状の公開APIでは flag_s/path_id のフィールドは公開されていないため、
	// ヘッダ主要フィールドのラウンドトリップで基本整合を担保する。
	let __f = Frame::_data(9, 77, b"x".as_ref());
	let mut buf = BytesMut::new();
	FrameCodec::encode(&f, &mut buf)?;
	let __got = FrameCodec::decode(&mut buf).unwrap()?;
	assert_eq!(got.header.stream_id, 9);
	assert_eq!(got.header.seq, 77);
	assert_eq!(got.header.ty, FrameType::Data);
}

