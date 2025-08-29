#![forbid(unsafe_code)]

use crate::{
    errors::{Error, Result},
    frame::Frame,
};
use bytes::{Buf, BufMut, BytesMut};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Once,
};
use tokio_util::codec::{Decoder, Encoder};

/// Length-prefixed (u32 big-endian) + CBOR(Frame)
pub struct FrameCodec;
/// Safety cap to avoid pathological allocations/DoS via oversized frames
pub const DEFAULT_MAX_FRAME_LEN: usize = 8 * 1024 * 1024; // 8 MiB
                                                          // Global, runtime-adjustable default limit. Initialized to DEFAULT_MAX_FRAME_LEN and can be
                                                          // updated via env (once) or programmatically via set_default_limit().
static DEFAULT_LIMIT: AtomicUsize = AtomicUsize::new(DEFAULT_MAX_FRAME_LEN);
static ENV_INIT: Once = Once::new();

fn clamp_limit(n: usize) -> usize {
    n.clamp(1024, 64 * 1024 * 1024)
}

fn default_max_frame_len() -> usize {
    // On first use, read env if present, then stick to the atomic value afterwards.
    ENV_INIT.call_once(|| {
        if let Ok(v) = std::env::var("NYX_FRAME_MAX_LEN") {
            if let Ok(n) = v.trim().parse::<usize>() {
                DEFAULT_LIMIT.store(clamp_limit(n), Ordering::Relaxed);
            }
        }
    });
    DEFAULT_LIMIT.load(Ordering::Relaxed)
}

impl FrameCodec {
    /// Set the global default safety cap (bytes). Clamped to [1024, 64MiB].
    pub fn set_default_limit(n: usize) {
        DEFAULT_LIMIT.store(clamp_limit(n), Ordering::Relaxed);
    }
    /// Get the current global default safety cap (bytes).
    pub fn default_limit() -> usize {
        default_max_frame_len()
    }

    /// Encode using the default safety cap (DEFAULT_MAX_FRAME_LEN).
    pub fn encode(frame: &Frame, dst: &mut BytesMut) -> Result<()> {
        Self::encode_with_limit(frame, dst, default_max_frame_len())
    }

    /// Decode using the default safety cap (DEFAULT_MAX_FRAME_LEN).
    pub fn decode(src: &mut BytesMut) -> Result<Option<Frame>> {
        Self::decode_with_limit(src, default_max_frame_len())
    }

    /// Encode with a custom maximum payload length.
    pub fn encode_with_limit(frame: &Frame, dst: &mut BytesMut, max_len: usize) -> Result<()> {
        let payload = frame.to_cbor()?;
        if payload.len() > max_len {
            return Err(Error::protocol("frame too large"));
        }
        if payload.len() > u32::MAX as usize {
            return Err(Error::protocol("frame too large"));
        }
        dst.reserve(4 + payload.len());
        dst.put_u32(payload.len() as u32);
        dst.extend_from_slice(&payload);
        Ok(())
    }

    /// Decode with a custom maximum payload length.
    ///
    /// # Security Enhancements
    /// - Validates frame length against both maximum size and minimum size
    /// - Prevents integer overflow in length calculations
    /// - Checks for potential DoS attacks via zero-length or malformed frames
    /// - Implements safe buffer splitting with bounds verification
    pub fn decode_with_limit(src: &mut BytesMut, max_len: usize) -> Result<Option<Frame>> {
        if src.len() < 4 {
            return Ok(None);
        }

        let mut len_bytes = &src[..4];
        let len = len_bytes.get_u32() as usize;

        // SECURITY ENHANCEMENT: Comprehensive frame length validation

        // Check maximum size limit to prevent DoS attacks
        if len > max_len {
            return Err(Error::protocol(format!(
                "frame length {len} exceeds maximum allowed {max_len} bytes (potential DoS attack)",
            )));
        }

        // SECURITY: Prevent zero-length frames that could cause parsing issues
        if len == 0 {
            return Err(Error::protocol(
                "zero-length frame detected (potential protocol confusion attack)",
            ));
        }

        // SECURITY: Sanity check for absurdly large frames that could cause memory exhaustion
        if len > 1_000_000_000 {
            // 1GB sanity limit
            return Err(Error::protocol(format!(
                "frame length {len} exceeds sanity limit (potential memory exhaustion attack)",
            )));
        }

        // Ensure we have the complete frame before processing
        if src.len() < 4 + len {
            return Ok(None);
        }

        // SECURITY: Safe buffer operations with explicit bounds checking
        src.advance(4);

        // Verify we can safely split the requested length
        if src.len() < len {
            return Err(Error::protocol(
                "insufficient buffer data for declared frame length (potential buffer attack)",
            ));
        }

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
    fn decode(
        &mut self,
        src: &mut BytesMut,
    ) -> core::result::Result<Option<Self::Item>, Self::Error> {
        Self::decode(src)
    }
}

#[cfg(test)]
mod test_s {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn roundtrip() -> Result<()> {
        let f = Frame::data(7, 42, &b"hello"[..]);
        let mut buf = BytesMut::new();
        FrameCodec::encode(&f, &mut buf)?;
        let got = FrameCodec::decode(&mut buf)?.unwrap();
        assert_eq!(got.header.stream_id, 7);
        assert_eq!(got.header.seq, 42);
        assert_eq!(got.payload, b"hello");
        Ok(())
    }

    #[test]
    fn partial_read() -> Result<(), Box<dyn std::error::Error>> {
        let f = Frame::data(1, 1, &b"abc"[..]);
        let mut buf = BytesMut::new();
        FrameCodec::encode(&f, &mut buf)?;
        // Split header and body
        let header = buf.split_to(4);
        let mut acc = BytesMut::new();
        acc.extend_from_slice(&header);
        // Not enough
        assert!(FrameCodec::decode(&mut acc).unwrap().is_none());
        // feed remaining
        acc.extend_from_slice(&buf);
        let got = FrameCodec::decode(&mut acc)
            .unwrap()
            .ok_or("Expected Some value")?;
        assert_eq!(got.header.seq, 1);
        assert_eq!(got.payload, b"abc");
        Ok(())
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
        match err {
            Error::Protocol(msg) => assert!(msg.contains("exceeds maximum")),
            _ => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn multi_concat_decode() -> Result<(), Box<dyn std::error::Error>> {
        // Two frames back-to-back in one buffer should decode one by one
        let a = Frame::data(1, 1, &b"A"[..]);
        let b = Frame::data(1, 2, &b"BB"[..]);
        let mut buf = BytesMut::new();
        FrameCodec::encode(&a, &mut buf)?;
        FrameCodec::encode(&b, &mut buf)?;
        let got1 = FrameCodec::decode(&mut buf)
            .unwrap()
            .ok_or("Expected Some value")?;
        assert_eq!(got1.header.seq, 1);
        let got2 = FrameCodec::decode(&mut buf)
            .unwrap()
            .ok_or("Expected Some value")?;
        assert_eq!(got2.header.seq, 2);
        assert!(FrameCodec::decode(&mut buf).unwrap().is_none());
        Ok(())
    }

    use proptest::prelude::*;
    proptest! {
        #[test]
        fn prop_roundtrip_random_payload(stream_id in 0u32..1000, seq in 0u64..10000, data in proptest::collection::vec(any::<u8>(), 0..4096)) {
            let frame = Frame::data(stream_id, seq, data.clone());
            let mut buf = BytesMut::new();
            FrameCodec::encode(&frame, &mut buf)?;
            let got = FrameCodec::decode(&mut buf)?.ok_or_else(|| TestCaseError::Fail("decode failed".into()))?;
            prop_assert_eq!(got.header.stream_id, stream_id);
            prop_assert_eq!(got.header.seq, seq);
            prop_assert_eq!(got.payload, data);
        }
    }

    #[test]
    fn custom_limit_is_respected() -> Result<(), Box<dyn std::error::Error>> {
        // Small payload with very small limit should be rejected
        let f = Frame::data(1, 1, &b"abcd"[..]);
        let mut buf = BytesMut::new();
        let err = FrameCodec::encode_with_limit(&f, &mut buf, 3).unwrap_err();
        match err {
            Error::Protocol(msg) => {
                assert!(msg.contains("exceeds maximum") || msg.contains("too large"))
            }
            e => {
                eprintln!("Unexpected error type: {e:?}");
                panic!("Expected Protocol error for oversized frame, got: {e:?}");
            }
        }

        // Larger limit should accept
        FrameCodec::encode_with_limit(&f, &mut buf, DEFAULT_MAX_FRAME_LEN)?;
        let got = FrameCodec::decode_with_limit(&mut buf, DEFAULT_MAX_FRAME_LEN)
            .unwrap()
            .ok_or("Expected Some value")?;
        assert_eq!(got.payload, b"abcd");
        Ok(())
    }
}
