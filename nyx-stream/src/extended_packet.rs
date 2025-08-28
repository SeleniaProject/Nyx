//! Extended Packet Format for Nyx Protocol v1.0
//!
//! This module implements the extended packet format as specified in
//! `spec/Nyx_Protocol_v1.0_Spec_EN.md` Section 7.
//!
//! # Packet Format
//! | Byte | Name | Description |
//! |------|------|-------------|
//! | 0–11 | CID | Connection ID |
//! | 12 | Type(2) + Flags(6) |
//! | 13 | PathID (8) |
//! | 14–15 | Length |
//! | 16–... | Payload |
//!
//! # Security Features
//! - Comprehensive validation of all fields
//! - Protection against buffer overflow attacks
//! - Strict bounds checking for packet sizes
//! - Secure handling of untrusted input data

use crate::errors::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Connection ID length (96-bit)
pub const CID_LENGTH: usize = 12;

/// Extended packet header size
pub const EXTENDED_HEADER_SIZE: usize = 16;

/// Maximum packet size (1280 bytes as specified)
pub const MAX_PACKET_SIZE: usize = 1280;

/// Maximum payload size
pub const MAX_PAYLOAD_SIZE: usize = MAX_PACKET_SIZE - EXTENDED_HEADER_SIZE;

/// Path ID type for multi-path communication
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct PathId(pub u8);

impl fmt::Display for PathId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Path{}", self.0)
    }
}

/// Connection ID for session identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId([u8; CID_LENGTH]);

impl ConnectionId {
    /// Create a new connection ID from bytes
    pub fn new(bytes: [u8; CID_LENGTH]) -> Self {
        Self(bytes)
    }

    /// Create a random connection ID using cryptographically secure randomness
    pub fn random() -> Self {
        let mut bytes = [0u8; CID_LENGTH];
        getrandom::getrandom(&mut bytes).expect("Failed to generate random CID");
        Self(bytes)
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Create from slice with validation
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != CID_LENGTH {
            return Err(Error::Protocol(format!(
                "Invalid CID length: expected {}, got {}",
                CID_LENGTH,
                bytes.len()
            )));
        }
        let mut cid = [0u8; CID_LENGTH];
        cid.copy_from_slice(bytes);
        Ok(Self(cid))
    }
}

impl Default for ConnectionId {
    fn default() -> Self {
        Self([0u8; CID_LENGTH])
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CID:")?;
        for byte in &self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Packet type definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum PacketType {
    /// Initial handshake packet
    Initial = 0,
    /// Retry packet for connection establishment
    Retry = 1,
    /// Handshake data packet
    Handshake = 2,
    /// Application data packet
    Application = 3,
}

impl PacketType {
    /// Convert from u8 with validation
    pub fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(PacketType::Initial),
            1 => Ok(PacketType::Retry),
            2 => Ok(PacketType::Handshake),
            3 => Ok(PacketType::Application),
            _ => Err(Error::Protocol(format!("Invalid packet type: {value}"))),
        }
    }

    /// Convert to u8
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// Packet flags for extended functionality
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PacketFlags(pub u8);

impl PacketFlags {
    /// Create new packet flags
    pub fn new(flags: u8) -> Self {
        // Mask to ensure only 6 bits are used (upper 2 bits reserved for type)
        Self(flags & 0x3F)
    }

    /// Check if a specific flag is set
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    /// Set a specific flag
    pub fn set_flag(&mut self, flag: u8) {
        self.0 |= flag & 0x3F;
    }

    /// Clear a specific flag
    pub fn clear_flag(&mut self, flag: u8) {
        self.0 &= !(flag & 0x3F);
    }

    /// Get raw flag value
    pub fn value(&self) -> u8 {
        self.0
    }
}

// Default is derived

/// Extended packet header according to v1.0 specification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendedPacketHeader {
    /// Connection ID (96-bit)
    pub cid: ConnectionId,
    /// Packet type (2 bits) + flags (6 bits)
    pub packet_type: PacketType,
    pub flags: PacketFlags,
    /// Path ID for multipath support
    pub path_id: PathId,
    /// Payload length
    pub length: u16,
}

impl ExtendedPacketHeader {
    /// Create a new extended packet header
    pub fn new(
        cid: ConnectionId,
        packet_type: PacketType,
        flags: PacketFlags,
        path_id: PathId,
        length: u16,
    ) -> Result<Self> {
        // SECURITY: Validate length to prevent overflow attacks
        if length as usize > MAX_PAYLOAD_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Payload length {length} exceeds maximum allowed {MAX_PAYLOAD_SIZE}"
            )));
        }

        Ok(Self {
            cid,
            packet_type,
            flags,
            path_id,
            length,
        })
    }

    /// Encode header to bytes with comprehensive validation
    pub fn encode(&self) -> Result<[u8; EXTENDED_HEADER_SIZE]> {
        let mut header = [0u8; EXTENDED_HEADER_SIZE];

        // Bytes 0-11: Connection ID
        header[0..CID_LENGTH].copy_from_slice(self.cid.as_bytes());

        // Byte 12: Type (2 bits) + Flags (6 bits)
        let type_flags = (self.packet_type.to_u8() << 6) | (self.flags.value() & 0x3F);
        header[12] = type_flags;

        // Byte 13: Path ID
        header[13] = self.path_id.0;

        // Bytes 14-15: Length (big-endian)
        header[14..16].copy_from_slice(&self.length.to_be_bytes());

        Ok(header)
    }

    /// Decode header from bytes with comprehensive security validation
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        // SECURITY: Validate input size to prevent buffer underflow
        if bytes.len() < EXTENDED_HEADER_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Header too short: expected {EXTENDED_HEADER_SIZE}, got {}",
                bytes.len()
            )));
        }

        // Parse Connection ID
        let cid = ConnectionId::from_slice(&bytes[0..CID_LENGTH])?;

        // Parse Type and Flags
        let type_flags = bytes[12];
        let packet_type = PacketType::from_u8(type_flags >> 6)?;
        let flags = PacketFlags::new(type_flags & 0x3F);

        // Parse Path ID
        let path_id = PathId(bytes[13]);

        // Parse Length
        let length = u16::from_be_bytes([bytes[14], bytes[15]]);

        // SECURITY: Additional validation
        if length as usize > MAX_PAYLOAD_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Declared payload length {length} exceeds maximum {MAX_PAYLOAD_SIZE}"
            )));
        }

        Ok(Self {
            cid,
            packet_type,
            flags,
            path_id,
            length,
        })
    }
}

/// Complete extended packet with header and payload
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtendedPacket {
    /// Packet header
    pub header: ExtendedPacketHeader,
    /// Packet payload
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

impl ExtendedPacket {
    /// Create a new extended packet with comprehensive validation
    pub fn new(header: ExtendedPacketHeader, payload: Vec<u8>) -> Result<Self> {
        // SECURITY: Validate payload size matches header
        if payload.len() != header.length as usize {
            return Err(Error::Protocol(format!(
                "SECURITY: Payload size mismatch: header declares {}, actual {}",
                header.length,
                payload.len()
            )));
        }

        // SECURITY: Validate total packet size
        let total_size = EXTENDED_HEADER_SIZE + payload.len();
        if total_size > MAX_PACKET_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Total packet size {total_size} exceeds maximum {MAX_PACKET_SIZE}"
            )));
        }

        Ok(Self { header, payload })
    }

    /// Encode packet to bytes with zero-copy optimization when possible
    pub fn encode(&self) -> Result<Bytes> {
        let header_bytes = self.header.encode()?;
        let total_size = EXTENDED_HEADER_SIZE + self.payload.len();

        // Use BytesMut for efficient concatenation
        let mut buf = BytesMut::with_capacity(total_size);
        buf.put_slice(&header_bytes);
        buf.put_slice(&self.payload);

        Ok(buf.freeze())
    }

    /// Decode packet from bytes with comprehensive security validation
    pub fn decode(mut bytes: Bytes) -> Result<Self> {
        // SECURITY: Validate minimum size
        if bytes.len() < EXTENDED_HEADER_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Packet too short: minimum {EXTENDED_HEADER_SIZE}, got {}",
                bytes.len()
            )));
        }

        // SECURITY: Validate maximum size
        if bytes.len() > MAX_PACKET_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Packet too large: maximum {MAX_PACKET_SIZE}, got {}",
                bytes.len()
            )));
        }

        // Decode header
        let header = ExtendedPacketHeader::decode(&bytes[0..EXTENDED_HEADER_SIZE])?;

        // Extract payload with validation
        let expected_payload_size = header.length as usize;
        let actual_payload_size = bytes.len() - EXTENDED_HEADER_SIZE;

        if actual_payload_size != expected_payload_size {
            return Err(Error::Protocol(format!(
                "SECURITY: Payload size mismatch: header declares {expected_payload_size}, actual {actual_payload_size}"
            )));
        }

        // Extract payload efficiently
        bytes.advance(EXTENDED_HEADER_SIZE);
        let payload = bytes.to_vec();

        Ok(Self { header, payload })
    }

    /// Get the total packet size
    pub fn size(&self) -> usize {
        EXTENDED_HEADER_SIZE + self.payload.len()
    }

    /// Check if packet is valid according to security constraints
    pub fn validate(&self) -> Result<()> {
        // Comprehensive validation
        if self.payload.len() != self.header.length as usize {
            return Err(Error::Protocol(
                "SECURITY: Payload size doesn't match header".to_string(),
            ));
        }

        if self.size() > MAX_PACKET_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Packet size {} exceeds maximum {MAX_PACKET_SIZE}",
                self.size(),
            )));
        }

        Ok(())
    }
}

/// Builder for constructing extended packets efficiently
pub struct ExtendedPacketBuilder {
    /// Reusable buffer for encoding
    encode_buffer: BytesMut,
}

impl ExtendedPacketBuilder {
    /// Create a new packet builder
    pub fn new() -> Self {
        Self {
            encode_buffer: BytesMut::with_capacity(MAX_PACKET_SIZE),
        }
    }

    /// Build a data packet with efficient memory management
    pub fn build_data_packet(
        &mut self,
        cid: ConnectionId,
        path_id: PathId,
        payload: &[u8],
    ) -> Result<ExtendedPacket> {
        // SECURITY: Validate payload size before processing
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Payload too large: {} > {}",
                payload.len(),
                MAX_PAYLOAD_SIZE
            )));
        }

        let header = ExtendedPacketHeader::new(
            cid,
            PacketType::Application,
            PacketFlags::default(),
            path_id,
            payload.len() as u16,
        )?;

        ExtendedPacket::new(header, payload.to_vec())
    }

    /// Build a handshake packet
    pub fn build_handshake_packet(
        &mut self,
        cid: ConnectionId,
        path_id: PathId,
        payload: &[u8],
    ) -> Result<ExtendedPacket> {
        // SECURITY: Validate payload size before processing
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(Error::Protocol(format!(
                "SECURITY: Handshake payload too large: {} > {}",
                payload.len(),
                MAX_PAYLOAD_SIZE
            )));
        }

        let header = ExtendedPacketHeader::new(
            cid,
            PacketType::Handshake,
            PacketFlags::default(),
            path_id,
            payload.len() as u16,
        )?;

        ExtendedPacket::new(header, payload.to_vec())
    }

    /// Encode packet with buffer reuse for maximum performance
    pub fn encode_reuse(&mut self, packet: &ExtendedPacket) -> Result<Vec<u8>> {
        // Validate packet before encoding
        packet.validate()?;

        // Clear and prepare buffer
        self.encode_buffer.clear();
        self.encode_buffer.reserve(packet.size());

        // Encode header
        let header_bytes = packet.header.encode()?;
        self.encode_buffer.put_slice(&header_bytes);

        // Encode payload
        self.encode_buffer.put_slice(&packet.payload);

        Ok(self.encode_buffer.to_vec())
    }
}

impl Default for ExtendedPacketBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_id_creation() {
        let cid = ConnectionId::random();
        assert_eq!(cid.as_bytes().len(), CID_LENGTH);

        let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let cid2 = ConnectionId::new(bytes);
        assert_eq!(cid2.as_bytes(), &bytes);
    }

    #[test]
    fn test_path_id() {
        let path = PathId(42);
        assert_eq!(path.0, 42);
        assert_eq!(format!("{path}"), "Path42");
    }

    #[test]
    fn test_packet_type_conversion() {
        assert_eq!(PacketType::Initial.to_u8(), 0);
        assert_eq!(PacketType::from_u8(0).unwrap(), PacketType::Initial);
        assert!(PacketType::from_u8(99).is_err());
    }

    #[test]
    fn test_packet_flags() {
        let mut flags = PacketFlags::new(0x0F);
        assert_eq!(flags.value(), 0x0F);

        flags.set_flag(0x20);
        assert!(flags.has_flag(0x20));

        flags.clear_flag(0x0F);
        assert!(!flags.has_flag(0x0F));
    }

    #[test]
    fn test_header_encoding_decoding() -> Result<()> {
        let cid = ConnectionId::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        let header = ExtendedPacketHeader::new(
            cid,
            PacketType::Application,
            PacketFlags::new(0x15),
            PathId(42),
            1024,
        )?;

        let encoded = header.encode()?;
        let decoded = ExtendedPacketHeader::decode(&encoded)?;

        assert_eq!(header, decoded);
        Ok(())
    }

    #[test]
    fn test_packet_creation_and_validation() -> Result<()> {
        let cid = ConnectionId::random();
        let payload = b"Hello, World!".to_vec();

        let header = ExtendedPacketHeader::new(
            cid,
            PacketType::Application,
            PacketFlags::default(),
            PathId(1),
            payload.len() as u16,
        )?;

        let packet = ExtendedPacket::new(header, payload)?;
        packet.validate()?;

        Ok(())
    }

    #[test]
    fn test_packet_encoding_decoding() -> Result<()> {
        let cid = ConnectionId::random();
        let payload = b"Test payload data".to_vec();

        let header = ExtendedPacketHeader::new(
            cid,
            PacketType::Application,
            PacketFlags::default(),
            PathId(3),
            payload.len() as u16,
        )?;

        let packet = ExtendedPacket::new(header, payload.clone())?;
        let encoded = packet.encode()?;
        let decoded = ExtendedPacket::decode(encoded)?;

        assert_eq!(packet, decoded);
        assert_eq!(decoded.payload, payload);
        Ok(())
    }

    #[test]
    fn test_packet_builder() -> Result<()> {
        let mut builder = ExtendedPacketBuilder::new();
        let cid = ConnectionId::random();
        let payload = b"Builder test payload";

        let packet = builder.build_data_packet(cid, PathId(5), payload)?;

        assert_eq!(packet.header.cid, cid);
        assert_eq!(packet.header.path_id, PathId(5));
        assert_eq!(packet.header.packet_type, PacketType::Application);
        assert_eq!(packet.payload, payload);

        Ok(())
    }

    #[test]
    fn test_security_validation() {
        // Test oversized payload
        let cid = ConnectionId::random();
        let oversized_payload = vec![0u8; MAX_PAYLOAD_SIZE + 1];

        let result = ExtendedPacketHeader::new(
            cid,
            PacketType::Application,
            PacketFlags::default(),
            PathId(1),
            oversized_payload.len() as u16,
        );
        assert!(result.is_err());

        // Test packet size validation during decode
        let invalid_data = vec![0u8; EXTENDED_HEADER_SIZE - 1];
        let result = ExtendedPacketHeader::decode(&invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_encode_reuse() -> Result<()> {
        let mut builder = ExtendedPacketBuilder::new();
        let cid = ConnectionId::random();
        let payload = b"Reuse test";

        let packet = builder.build_data_packet(cid, PathId(7), payload)?;
        let encoded1 = builder.encode_reuse(&packet)?;
        let encoded2 = builder.encode_reuse(&packet)?;

        // Should produce identical results
        assert_eq!(encoded1, encoded2);
        assert_eq!(encoded1.len(), EXTENDED_HEADER_SIZE + payload.len());

        Ok(())
    }
}
