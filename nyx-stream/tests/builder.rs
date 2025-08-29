#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_collect,
    clippy::explicit_into_iter_loop,
    clippy::uninlined_format_args,
    clippy::unreachable
)]

#![allow(unused_imports)]
use bytes::BytesMut;
use nyx_stream::frame::{Frame, FrameHeader, FrameType};
use nyx_stream::frame_codec::FrameCodec;

// Extended Packet Format: header_roundtrip
// ヘッダの主要フィールドがエンコード/デコードで保存されることを確認
#[test]
fn header_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let f = Frame::data(123, 456, &b"hello"[..]);
    let mut buf = BytesMut::new();
    FrameCodec::encode(&f, &mut buf)?;
    let got = FrameCodec::decode(&mut buf)
        .expect("decode")
        .ok_or("decode failed")?;
    assert_eq!(got.header.stream_id, 123);
    assert_eq!(got.header.seq, 456);
    assert_eq!(got.header.ty, FrameType::Data);
    assert_eq!(got.payload, b"hello");
    Ok(())
}
