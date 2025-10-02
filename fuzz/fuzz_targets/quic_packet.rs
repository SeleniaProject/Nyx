#![no_main]

use libfuzzer_sys::{arbitrary::Arbitrary, fuzz_target};
use bytes::{Bytes, BytesMut, BufMut};

/// QUIC packet header types
#[derive(Debug, Clone, Copy, Arbitrary)]
enum QuicPacketType {
    Initial,
    ZeroRTT,
    Handshake,
    Retry,
    ShortHeader,
    VersionNegotiation,
    Invalid(u8), // Edge case: invalid type
}

/// QUIC frame types (simplified)
#[derive(Debug, Clone, Copy, Arbitrary)]
enum QuicFrameType {
    Padding,
    Ping,
    Ack,
    ResetStream,
    StopSending,
    Crypto,
    NewToken,
    Stream,
    MaxData,
    MaxStreamData,
    MaxStreams,
    DataBlocked,
    StreamDataBlocked,
    StreamsBlocked,
    NewConnectionId,
    RetireConnectionId,
    PathChallenge,
    PathResponse,
    ConnectionClose,
    HandshakeDone,
    Invalid(u8), // Edge case: invalid frame type
}

/// Fuzz input structure for QUIC packet
#[derive(Debug, Arbitrary)]
struct FuzzQuicPacket {
    packet_type: QuicPacketType,
    connection_id_len: u8, // 0-20 bytes
    connection_id: Vec<u8>,
    version: u32,
    packet_number: u64,
    payload_len: u16,
    payload: Vec<u8>,
    frames: Vec<FuzzQuicFrame>,
}

/// Fuzz input for QUIC frames
#[derive(Debug, Arbitrary)]
struct FuzzQuicFrame {
    frame_type: QuicFrameType,
    stream_id: u64,
    offset: u64,
    length: u16,
    data: Vec<u8>,
}

impl FuzzQuicPacket {
    /// Serialize fuzz packet to bytes (simulating packet construction)
    fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::new();
        
        // Edge case: Header flags
        let header_byte = match self.packet_type {
            QuicPacketType::Initial => 0xc0,
            QuicPacketType::ZeroRTT => 0xd0,
            QuicPacketType::Handshake => 0xe0,
            QuicPacketType::Retry => 0xf0,
            QuicPacketType::ShortHeader => 0x40,
            QuicPacketType::VersionNegotiation => 0x80,
            QuicPacketType::Invalid(b) => b,
        };
        buf.put_u8(header_byte);
        
        // Edge case: Version (long header only)
        if matches!(
            self.packet_type,
            QuicPacketType::Initial | QuicPacketType::ZeroRTT | QuicPacketType::Handshake
        ) {
            buf.put_u32(self.version);
        }
        
        // Edge case: Connection ID length (0-20, or invalid)
        let cid_len = self.connection_id_len.min(20);
        buf.put_u8(cid_len);
        
        // Edge case: Connection ID (may be truncated or oversized)
        let cid_bytes: Vec<u8> = self
            .connection_id
            .iter()
            .take(cid_len as usize)
            .copied()
            .collect();
        buf.put_slice(&cid_bytes);
        
        // Edge case: Packet number (variable length encoding)
        if self.packet_number < 256 {
            buf.put_u8(self.packet_number as u8);
        } else if self.packet_number < 65536 {
            buf.put_u16(self.packet_number as u16);
        } else {
            buf.put_u64(self.packet_number);
        }
        
        // Edge case: Payload length (may be inconsistent with actual payload)
        buf.put_u16(self.payload_len);
        
        // Edge case: Payload (may be empty, truncated, or oversized)
        let payload_bytes: Vec<u8> = self
            .payload
            .iter()
            .take(self.payload_len as usize)
            .copied()
            .collect();
        buf.put_slice(&payload_bytes);
        
        // Edge case: Frames (malformed or incomplete)
        for frame in &self.frames {
            frame.append_to_buf(&mut buf);
        }
        
        buf.freeze()
    }
}

impl FuzzQuicFrame {
    fn append_to_buf(&self, buf: &mut BytesMut) {
        // Edge case: Frame type byte
        let frame_type_byte = match self.frame_type {
            QuicFrameType::Padding => 0x00,
            QuicFrameType::Ping => 0x01,
            QuicFrameType::Ack => 0x02,
            QuicFrameType::ResetStream => 0x04,
            QuicFrameType::StopSending => 0x05,
            QuicFrameType::Crypto => 0x06,
            QuicFrameType::NewToken => 0x07,
            QuicFrameType::Stream => 0x08,
            QuicFrameType::MaxData => 0x10,
            QuicFrameType::MaxStreamData => 0x11,
            QuicFrameType::MaxStreams => 0x12,
            QuicFrameType::DataBlocked => 0x14,
            QuicFrameType::StreamDataBlocked => 0x15,
            QuicFrameType::StreamsBlocked => 0x16,
            QuicFrameType::NewConnectionId => 0x18,
            QuicFrameType::RetireConnectionId => 0x19,
            QuicFrameType::PathChallenge => 0x1a,
            QuicFrameType::PathResponse => 0x1b,
            QuicFrameType::ConnectionClose => 0x1c,
            QuicFrameType::HandshakeDone => 0x1e,
            QuicFrameType::Invalid(b) => b,
        };
        buf.put_u8(frame_type_byte);
        
        // Edge case: Stream ID (variable length integer)
        if matches!(
            self.frame_type,
            QuicFrameType::Stream | QuicFrameType::ResetStream | QuicFrameType::StopSending
        ) {
            buf.put_u64(self.stream_id);
        }
        
        // Edge case: Offset (may be invalid)
        if matches!(self.frame_type, QuicFrameType::Stream | QuicFrameType::Crypto) {
            buf.put_u64(self.offset);
        }
        
        // Edge case: Length (may not match data size)
        buf.put_u16(self.length);
        
        // Edge case: Data (may be empty or oversized)
        let data_bytes: Vec<u8> = self.data.iter().take(self.length as usize).copied().collect();
        buf.put_slice(&data_bytes);
    }
}

/// Parse QUIC packet header (simplified for fuzzing)
fn parse_quic_packet_header(data: &[u8]) -> Result<(u8, u32, Vec<u8>, u64), String> {
    // Edge case: Empty packet
    if data.is_empty() {
        return Err("Empty packet".to_string());
    }
    
    // Edge case: Packet too large (DoS prevention)
    if data.len() > 65536 {
        return Err("Packet too large".to_string());
    }
    
    let mut offset = 0;
    
    // Header byte
    let header_byte = data[offset];
    offset += 1;
    
    // Edge case: Long header vs short header
    let is_long_header = (header_byte & 0x80) != 0;
    
    let version = if is_long_header {
        // Edge case: Insufficient data for version
        if data.len() < offset + 4 {
            return Err("Truncated version field".to_string());
        }
        let v = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]);
        offset += 4;
        v
    } else {
        0 // Short header has no version
    };
    
    // Edge case: Connection ID length
    if data.len() < offset + 1 {
        return Err("Truncated connection ID length".to_string());
    }
    let cid_len = data[offset] as usize;
    offset += 1;
    
    // Edge case: Invalid connection ID length (>20)
    if cid_len > 20 {
        return Err("Invalid connection ID length".to_string());
    }
    
    // Edge case: Insufficient data for connection ID
    if data.len() < offset + cid_len {
        return Err("Truncated connection ID".to_string());
    }
    let connection_id = data[offset..offset + cid_len].to_vec();
    offset += cid_len;
    
    // Edge case: Packet number (simplified - assume 1 byte)
    if data.len() < offset + 1 {
        return Err("Truncated packet number".to_string());
    }
    let packet_number = data[offset] as u64;
    
    Ok((header_byte, version, connection_id, packet_number))
}

/// Parse QUIC frames from payload (simplified)
fn parse_quic_frames(payload: &[u8]) -> Result<Vec<u8>, String> {
    // Edge case: Empty payload
    if payload.is_empty() {
        return Ok(Vec::new());
    }
    
    let mut frame_types = Vec::new();
    let mut offset = 0;
    
    // Edge case: Malformed frames (iterate until end or error)
    while offset < payload.len() {
        // Edge case: Insufficient data for frame type
        if offset >= payload.len() {
            break;
        }
        
        let frame_type = payload[offset];
        frame_types.push(frame_type);
        offset += 1;
        
        // Edge case: Frame-specific parsing (simplified)
        // Skip frame data based on type (very simplified)
        match frame_type {
            0x00 => {}, // Padding - no data
            0x01 => {}, // Ping - no data
            0x02 => {
                // ACK - skip variable length
                if offset + 8 < payload.len() {
                    offset += 8;
                }
            }
            0x08 => {
                // Stream - skip stream ID, offset, length, data
                if offset + 10 < payload.len() {
                    let data_len = u16::from_be_bytes([payload[offset + 8], payload[offset + 9]]);
                    offset += 10 + data_len as usize;
                } else {
                    break;
                }
            }
            _ => {
                // Unknown frame - skip 8 bytes or until end
                offset += 8.min(payload.len() - offset);
            }
        }
    }
    
    Ok(frame_types)
}

fuzz_target!(|data: &[u8]| {
    // Strategy 1: Fuzz QUIC packet construction with Arbitrary
    if data.len() > 128 {
        if let Ok(fuzz_packet) = FuzzQuicPacket::arbitrary(&mut arbitrary::Unstructured::new(data))
        {
            // Edge cases covered:
            // - Invalid packet types
            // - Oversized/undersized connection IDs
            // - Mismatched payload lengths
            // - Malformed frames
            let packet_bytes = fuzz_packet.to_bytes();
            
            // Ensure packet serialization doesn't panic
            assert!(packet_bytes.len() < 100000); // Sanity check
        }
    }
    
    // Strategy 2: Fuzz QUIC packet header parsing
    // Edge cases covered:
    // - Empty packets
    // - Truncated headers
    // - Invalid connection ID lengths
    // - Oversized packets (DoS)
    let _ = parse_quic_packet_header(data);
    
    // Strategy 3: Fuzz QUIC frame parsing
    // Edge cases covered:
    // - Malformed frames
    // - Incomplete frame data
    // - Unknown frame types
    let _ = parse_quic_frames(data);
    
    // Strategy 4: Fuzz protocol violations
    // Edge case: Version negotiation with invalid version
    if data.len() >= 8 {
        let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        // Versions 0x00000000 and 0x00000001 are reserved
        if version == 0 || version == 1 {
            // This should be handled gracefully, not panic
            let _ = parse_quic_packet_header(data);
        }
    }
    
    // Strategy 5: Fuzz fragmentation edge cases
    // Edge case: Packet split across multiple datagrams
    if data.len() >= 2 {
        let split_point = (data[0] as usize) % data.len();
        let _ = parse_quic_packet_header(&data[..split_point]);
        let _ = parse_quic_packet_header(&data[split_point..]);
    }
    
    // All branches should handle errors gracefully without panicking
});
