#![forbid(unsafe_code)]

use crate::{errors::{Error, Result}, frame::Frame};
use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

/// Length-prefixed (u32 big-endian) + CBOR(Frame)
pub struct FrameCodec;
/// Safety cap to avoid pathological allocations/DoS via oversized frames
pub const DEFAULT_MAX_FRAME_LEN: usize = 8 * 1024 * 1024; // 8 MiB

impl FrameCodec {
    pub fn encode(frame: &Frame, dst: &mut BytesMut) -> Result<()> {
        let payload = frame.to_cbor()?;
        if payload.len() > DEFAULT_MAX_FRAME_LEN { return Err(Error::protocol("frame too large")); }
        if payload.len() > u32::MAX as usize { return Err(Error::protocol("frame too large")); }
        dst.reserve(4 + payload.len());
        dst.put_u32(payload.len() as u32);
        dst.extend_from_slice(&payload);
        Ok(())
    }

    pub fn decode(src: &mut BytesMut) -> Result<Option<Frame>> {
        if src.len() < 4 { return Ok(None); }
    let mut len_bytes = &src[..4];
    let len = len_bytes.get_u32() as usize;
    if len > DEFAULT_MAX_FRAME_LEN { return Err(Error::protocol("frame too large")); }
        if src.len() < 4 + len { return Ok(None); }
        src.advance(4);
        let data = src.split_to(len);
        let f = Frame::from_cbor(&data)?;
        Ok(Some(f))
    }
}

impl Encoder<Frame> for FrameCodec {
    type Error = Error;
    fn encode(&mut self, item: Frame, dst: &mut BytesMut) -> core::result::Result<(), Self::Error> {
        Self::encode(&item, dst)
    }
}

impl Decoder for FrameCodec {
    type Item = Frame;
    type Error = Error;
    fn decode(&mut self, src: &mut BytesMut) -> core::result::Result<Option<Self::Item>, Self::Error> {
        Self::decode(src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let f = Frame::data(7, 42, b"hello".as_ref());
        let mut buf = BytesMut::new();
        FrameCodec::encode(&f, &mut buf).unwrap();
        let got = FrameCodec::decode(&mut buf).unwrap().unwrap();
        assert_eq!(got.header.stream_id, 7);
        assert_eq!(got.header.seq, 42);
        assert_eq!(got.payload, b"hello");
    }

    #[test]
    fn partial_read() {
        let f = Frame::data(1, 1, b"abc".as_ref());
        let mut buf = BytesMut::new();
        FrameCodec::encode(&f, &mut buf).unwrap();
        // Split header and body
        let header = buf.split_to(4);
        let mut acc = BytesMut::new();
        acc.extend_from_slice(&header);
        // Not enough
        assert!(FrameCodec::decode(&mut acc).unwrap().is_none());
        // Feed remaining
        acc.extend_from_slice(&buf);
        let got = FrameCodec::decode(&mut acc).unwrap().unwrap();
        assert_eq!(got.header.seq, 1);
        assert_eq!(got.payload, b"abc");
    }

    #[test]
    fn too_large_rejected() {
        // Prepare a fake header that declares a huge length beyond DEFAULT_MAX_FRAME_LEN
        let mut acc = BytesMut::new();
        // Use u32::MAX, which surely exceeds DEFAULT_MAX_FRAME_LEN
        acc.put_u32(u32::MAX);
        // Supply a small body; decode should reject early on length check
        acc.extend_from_slice(&[0u8; 4]);
        let err = FrameCodec::decode(&mut acc).unwrap_err();
        match err { Error::Protocol(msg) => assert!(msg.contains("too large")), _ => panic!("unexpected error: {err:?}") }
    }

    #[test]
    fn multi_concat_decode() {
        // Two frames back-to-back in one buffer should decode one by one
        let a = Frame::data(1, 1, b"A".as_ref());
        let b = Frame::data(1, 2, b"BB".as_ref());
        let mut buf = BytesMut::new();
        FrameCodec::encode(&a, &mut buf).unwrap();
        FrameCodec::encode(&b, &mut buf).unwrap();
        let got1 = FrameCodec::decode(&mut buf).unwrap().unwrap();
        assert_eq!(got1.header.seq, 1);
        let got2 = FrameCodec::decode(&mut buf).unwrap().unwrap();
        assert_eq!(got2.header.seq, 2);
        assert!(FrameCodec::decode(&mut buf).unwrap().is_none());
    }

    use proptest::prelude::*;
    proptest! {
        #[test]
        fn prop_roundtrip_random_payload(stream_id in 0u32..1000, seq in 0u64..10000, data in proptest::collection::vec(any::<u8>(), 0..4096)) {
            let f = Frame::data(stream_id, seq, data.clone());
            let mut buf = BytesMut::new();
            FrameCodec::encode(&f, &mut buf).unwrap();
            let got = FrameCodec::decode(&mut buf).unwrap().unwrap();
            prop_assert_eq!(got.header.stream_id, stream_id);
            prop_assert_eq!(got.header.seq, seq);
            prop_assert_eq!(got.payload, data);
        }
    }
}
