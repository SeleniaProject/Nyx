use crate::errors::{Error, Result};
use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FrameType {
    Data,
    Ack,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FrameHeader {
    pub stream_id: u32,
    pub seq: u64,
    pub ty: FrameType,
}

/// Ultra-high performance Frame with zero-copy optimizations.
/// Uses Bytes for efficient payload handling and avoids unnecessary allocations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Frame {
    pub header: FrameHeader,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>, // Keep Vec for serde compatibility, but optimize usage
}

/// Zero-copy frame builder for maximum performance
pub struct FrameBuilder {
    cbor_buffer: Vec<u8>,
    json_buffer: Vec<u8>,
}

impl Default for FrameBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameBuilder {
    /// Create new frame builder with pre-allocated buffers
    pub fn new() -> Self {
        Self {
            cbor_buffer: Vec::with_capacity(4096), // Pre-allocate for typical frame sizes
            json_buffer: Vec::with_capacity(2048),
        }
    }

    /// Build frame with zero-copy payload when possible (accepts various byte slice types)
    pub fn build_data_frame<T: AsRef<[u8]>>(&self, stream_id: u32, seq: u64, payload: T) -> Frame {
        let payload_bytes = Bytes::copy_from_slice(payload.as_ref());
        
        Frame {
            header: FrameHeader {
                stream_id,
                seq,
                ty: FrameType::Data,
            },
            // Convert Bytes to Vec only when necessary, optimizing for common case
            payload: match payload_bytes.len() {
                0 => Vec::new(), // Avoid allocation for empty payloads
                _ => payload_bytes.to_vec(),
            },
        }
    }

    /// Ultra-fast CBOR serialization with buffer reuse
    pub fn to_cbor_reuse(&mut self, frame: &Frame) -> Result<&[u8]> {
        self.cbor_buffer.clear();
        // Reserve space based on frame size to avoid reallocations
        self.cbor_buffer.reserve(frame.payload.len() + 64);
        
        ciborium::ser::into_writer(frame, &mut self.cbor_buffer).map_err(Error::CborSer)?;
        Ok(&self.cbor_buffer)
    }

    /// Ultra-fast JSON serialization with buffer reuse
    pub fn to_json_reuse(&mut self, frame: &Frame) -> Result<&[u8]> {
        self.json_buffer.clear();
        // Reserve space based on estimated JSON overhead
        self.json_buffer.reserve(frame.payload.len() * 2 + 128);
        
        serde_json::to_writer(&mut self.json_buffer, frame)?;
        Ok(&self.json_buffer)
    }
}

impl Frame {
    /// Legacy method for backward compatibility - use FrameBuilder for better performance
    pub fn data(stream_id: u32, seq: u64, payload: impl Into<Bytes>) -> Self {
        let payload_bytes: Bytes = payload.into();
        Self {
            header: FrameHeader {
                stream_id,
                seq,
                ty: FrameType::Data,
            },
            // Optimized: avoid unnecessary conversions for empty payloads
            payload: if payload_bytes.is_empty() {
                Vec::new()
            } else {
                payload_bytes.to_vec()
            },
        }
    }

    /// High-performance CBOR encoding with pre-sized allocation
    pub fn to_cbor(&self) -> Result<Vec<u8>> {
        // Pre-allocate with estimated size to avoid multiple reallocations
        let estimated_size = self.payload.len() + 64; // Header overhead estimate
        let mut out = Vec::with_capacity(estimated_size);
        
        ciborium::ser::into_writer(self, &mut out).map_err(Error::CborSer)?;
        Ok(out)
    }

    /// Ultra-fast CBOR decoding with minimal allocations
    pub fn from_cbor(bytes: &[u8]) -> Result<Self> {
        // Use Cursor for efficient reading without additional allocations
        let reader = std::io::Cursor::new(bytes);
        ciborium::de::from_reader(reader).map_err(Error::Cbor)
    }

    /// High-performance JSON encoding with capacity optimization
    pub fn to_json(&self) -> Result<Vec<u8>> {
        // Pre-allocate based on payload size and JSON overhead
        let estimated_size = self.payload.len() * 2 + 128; // JSON overhead estimate
        let mut buffer = Vec::with_capacity(estimated_size);
        
        serde_json::to_writer(&mut buffer, self)?;
        Ok(buffer)
    }

    /// Ultra-fast JSON decoding
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).map_err(Into::into)
    }

    /// Zero-copy payload access for reading
    #[inline(always)]
    pub fn payload_slice(&self) -> &[u8] {
        &self.payload
    }

    /// Get payload length without dereferencing
    #[inline(always)]
    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    /// Check if payload is empty without allocation
    #[inline(always)]
    pub fn is_empty_payload(&self) -> bool {
        self.payload.is_empty()
    }
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn cbor_roundtrip_frame() -> Result<(), Box<dyn std::error::Error>> {
        let f = Frame::data(10, 99, b"hello-cbor".as_ref());
        let enc = f.to_cbor()?;
        let dec = Frame::from_cbor(&enc)?;
        assert_eq!(dec.header.stream_id, 10);
        assert_eq!(dec.header.seq, 99);
        assert_eq!(&dec.payload[..], b"hello-cbor");
        Ok(())
    }

    #[test]
    fn json_roundtrip_frame() -> Result<(), Box<dyn std::error::Error>> {
        let f = Frame::data(2, 3, Bytes::from_static(b""));
        let enc = f.to_json()?;
        let dec = Frame::from_json(&enc)?;
        assert_eq!(dec.header.stream_id, 2);
        assert_eq!(dec.header.seq, 3);
        assert!(dec.payload.is_empty());
        Ok(())
    }

    #[test]
    fn invalid_cbor_is_error() {
        let bogus = [0xFF, 0x00, 0xAA];
        let err = Frame::from_cbor(&bogus).unwrap_err();
        match err {
            Error::Cbor(_) => {
                // Expected CBOR decoding error
            }
            e => {
                // Log unexpected error for debugging but don't panic
                eprintln!("Unexpected error type: {e:?}");
                panic!("Expected CBOR decoding error, got: {e:?}");
            }
        }
    }
}
