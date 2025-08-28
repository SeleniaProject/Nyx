//! Comprehensive tests for Extended Packet Format implementation
//!
//! This test suite validates the Extended Packet Format implementation
//! according to Nyx Protocol v1.0 specification with focus on:
//! - Security validation and edge cases
//! - Performance characteristics  
//! - Protocol compliance
//! - Memory safety and efficiency

use nyx_stream::extended_packet::*;
use nyx_stream::{Result};
use bytes::Bytes;

#[test]
fn test_connection_id_basic_operations() {
    // Test random generation
    let cid1 = ConnectionId::random();
    let cid2 = ConnectionId::random();
    assert_ne!(cid1, cid2, "Random CIDs should be unique");
    assert_eq!(cid1.as_bytes().len(), CID_LENGTH);

    // Test deterministic creation
    let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let cid3 = ConnectionId::new(bytes);
    let cid4 = ConnectionId::new(bytes);
    assert_eq!(cid3, cid4, "Deterministic CIDs should be equal");
    assert_eq!(cid3.as_bytes(), &bytes);

    // Test from_slice validation
    let valid_slice = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let cid_from_slice = ConnectionId::from_slice(valid_slice).unwrap();
    assert_eq!(cid_from_slice.as_bytes(), valid_slice);

    // Test invalid slice length
    let invalid_slice = &[1, 2, 3];
    assert!(ConnectionId::from_slice(invalid_slice).is_err());
}

#[test]
fn test_path_id_functionality() {
    let path = PathId(42);
    assert_eq!(path.0, 42);
    assert_eq!(format!("{}", path), "Path42");

    let default_path = PathId::default();
    assert_eq!(default_path.0, 0);
    assert_eq!(format!("{}", default_path), "Path0");

    // Test with maximum value
    let max_path = PathId(255);
    assert_eq!(max_path.0, 255);
    assert_eq!(format!("{}", max_path), "Path255");
}

#[test]
fn test_packet_type_validation() -> Result<()> {
    // Test all valid packet types
    assert_eq!(PacketType::Initial.to_u8(), 0);
    assert_eq!(PacketType::Retry.to_u8(), 1);
    assert_eq!(PacketType::Handshake.to_u8(), 2);
    assert_eq!(PacketType::Application.to_u8(), 3);

    // Test valid conversions
    assert_eq!(PacketType::from_u8(0)?, PacketType::Initial);
    assert_eq!(PacketType::from_u8(1)?, PacketType::Retry);
    assert_eq!(PacketType::from_u8(2)?, PacketType::Handshake);
    assert_eq!(PacketType::from_u8(3)?, PacketType::Application);

    // Test invalid conversions
    assert!(PacketType::from_u8(4).is_err());
    assert!(PacketType::from_u8(255).is_err());

    Ok(())
}

#[test]
fn test_packet_flags_operations() {
    let mut flags = PacketFlags::new(0x00);
    assert_eq!(flags.value(), 0x00);
    assert!(!flags.has_flag(0x01));

    // Test setting flags
    flags.set_flag(0x01);
    assert!(flags.has_flag(0x01));
    assert_eq!(flags.value(), 0x01);

    flags.set_flag(0x08);
    assert!(flags.has_flag(0x08));
    assert!(flags.has_flag(0x01));
    assert_eq!(flags.value(), 0x09);

    // Test clearing flags
    flags.clear_flag(0x01);
    assert!(!flags.has_flag(0x01));
    assert!(flags.has_flag(0x08));
    assert_eq!(flags.value(), 0x08);

    // Test flag masking (only 6 bits should be used)
    let masked_flags = PacketFlags::new(0xFF);
    assert_eq!(masked_flags.value(), 0x3F); // Only lower 6 bits
}

#[test]
fn test_extended_packet_header_creation() -> Result<()> {
    let cid = ConnectionId::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    let header = ExtendedPacketHeader::new(
        cid,
        PacketType::Application,
        PacketFlags::new(0x15),
        PathId(42),
        1024,
    )?;

    assert_eq!(header.cid, cid);
    assert_eq!(header.packet_type, PacketType::Application);
    assert_eq!(header.flags.value(), 0x15);
    assert_eq!(header.path_id, PathId(42));
    assert_eq!(header.length, 1024);

    Ok(())
}

#[test]
fn test_header_security_validation() {
    let cid = ConnectionId::random();
    
    // Test oversized payload length
    let result = ExtendedPacketHeader::new(
        cid,
        PacketType::Application,
        PacketFlags::default(),
        PathId(1),
        (MAX_PAYLOAD_SIZE + 1) as u16,
    );
    assert!(result.is_err(), "Should reject oversized payload length");

    // Test maximum valid payload length
    let result = ExtendedPacketHeader::new(
        cid,
        PacketType::Application,
        PacketFlags::default(),
        PathId(1),
        MAX_PAYLOAD_SIZE as u16,
    );
    assert!(result.is_ok(), "Should accept maximum valid payload length");
}

#[test]
fn test_header_encoding_decoding_roundtrip() -> Result<()> {
    let test_cases = vec![
        // Basic case
        (
            ConnectionId::new([0; CID_LENGTH]),
            PacketType::Initial,
            PacketFlags::new(0x00),
            PathId(0),
            0u16,
        ),
        // Maximum values
        (
            ConnectionId::new([0xFF; CID_LENGTH]),
            PacketType::Application,
            PacketFlags::new(0x3F),
            PathId(255),
            MAX_PAYLOAD_SIZE as u16,
        ),
        // Random case
        (
            ConnectionId::random(),
            PacketType::Handshake,
            PacketFlags::new(0x2A),
            PathId(128),
            512u16,
        ),
    ];

    for (cid, packet_type, flags, path_id, length) in test_cases {
        let header = ExtendedPacketHeader::new(cid, packet_type, flags, path_id, length)?;
        let encoded = header.encode()?;
        let decoded = ExtendedPacketHeader::decode(&encoded)?;

        assert_eq!(header, decoded, "Header should roundtrip correctly");
        assert_eq!(encoded.len(), EXTENDED_HEADER_SIZE);
    }

    Ok(())
}

#[test]
fn test_header_decode_security_validation() {
    // Test insufficient data
    let short_data = vec![0u8; EXTENDED_HEADER_SIZE - 1];
    assert!(ExtendedPacketHeader::decode(&short_data).is_err());

    // Test invalid packet type
    let mut invalid_type_data = vec![0u8; EXTENDED_HEADER_SIZE];
    invalid_type_data[12] = 0xFF; // Invalid type (0b11xxxxxx)
    assert!(ExtendedPacketHeader::decode(&invalid_type_data).is_err());

    // Test oversized length declaration
    let mut oversized_data = vec![0u8; EXTENDED_HEADER_SIZE];
    let oversized_length = (MAX_PAYLOAD_SIZE + 1) as u16;
    oversized_data[14..16].copy_from_slice(&oversized_length.to_be_bytes());
    assert!(ExtendedPacketHeader::decode(&oversized_data).is_err());
}

#[test]
fn test_extended_packet_creation() -> Result<()> {
    let cid = ConnectionId::random();
    let payload = b"Hello, Nyx Protocol v1.0!".to_vec();
    
    let header = ExtendedPacketHeader::new(
        cid,
        PacketType::Application,
        PacketFlags::default(),
        PathId(5),
        payload.len() as u16,
    )?;

    let packet = ExtendedPacket::new(header, payload.clone())?;
    
    assert_eq!(packet.header.cid, cid);
    assert_eq!(packet.header.path_id, PathId(5));
    assert_eq!(packet.payload, payload);
    assert_eq!(packet.size(), EXTENDED_HEADER_SIZE + payload.len());

    // Test validation
    packet.validate()?;

    Ok(())
}

#[test]
fn test_packet_security_validation() {
    let cid = ConnectionId::random();
    
    // Test payload size mismatch
    let payload = b"test".to_vec();
    let header = ExtendedPacketHeader::new(
        cid,
        PacketType::Application,
        PacketFlags::default(),
        PathId(1),
        (payload.len() + 1) as u16, // Mismatched length
    ).unwrap();

    let result = ExtendedPacket::new(header, payload);
    assert!(result.is_err(), "Should reject mismatched payload size");

    // Test oversized total packet
    let oversized_payload = vec![0u8; MAX_PAYLOAD_SIZE + 1];
    let header = ExtendedPacketHeader::new(
        cid,
        PacketType::Application,
        PacketFlags::default(),
        PathId(1),
        oversized_payload.len() as u16,
    );
    assert!(header.is_err(), "Should reject oversized payload in header creation");
}

#[test]
fn test_packet_encoding_decoding_roundtrip() -> Result<()> {
    let test_payloads = vec![
        vec![], // Empty payload
        b"Small payload".to_vec(),
        vec![0xAA; 100], // Binary data
        vec![0x55; 1000], // Large payload
        (0..255).collect::<Vec<u8>>(), // Sequential data
    ];

    for payload in test_payloads {
        let cid = ConnectionId::random();
        let path_id = PathId(fastrand::u8(..));
        
        let header = ExtendedPacketHeader::new(
            cid,
            PacketType::Application,
            PacketFlags::new(fastrand::u8(..) & 0x3F),
            path_id,
            payload.len() as u16,
        )?;

        let packet = ExtendedPacket::new(header, payload.clone())?;
        let encoded = packet.encode()?;
        let decoded = ExtendedPacket::decode(encoded)?;

        assert_eq!(packet, decoded, "Packet should roundtrip correctly");
        assert_eq!(decoded.payload, payload, "Payload should be preserved");
    }

    Ok(())
}

#[test]
fn test_packet_decode_security_validation() {
    // Test packet too small
    let tiny_packet = Bytes::from(vec![0u8; EXTENDED_HEADER_SIZE - 1]);
    assert!(ExtendedPacket::decode(tiny_packet).is_err());

    // Test packet too large
    let huge_packet = Bytes::from(vec![0u8; MAX_PACKET_SIZE + 1]);
    assert!(ExtendedPacket::decode(huge_packet).is_err());

    // Test payload size mismatch
    let mut packet_data = vec![0u8; EXTENDED_HEADER_SIZE + 10];
    packet_data[14..16].copy_from_slice(&20u16.to_be_bytes()); // Claims 20 bytes payload
    let malformed_packet = Bytes::from(packet_data);
    assert!(ExtendedPacket::decode(malformed_packet).is_err());
}

#[test]
fn test_extended_packet_builder() -> Result<()> {
    let mut builder = ExtendedPacketBuilder::new();
    let cid = ConnectionId::random();
    let payload = b"Builder test payload data";

    // Test data packet building
    let data_packet = builder.build_data_packet(cid, PathId(10), payload)?;
    assert_eq!(data_packet.header.packet_type, PacketType::Application);
    assert_eq!(data_packet.header.path_id, PathId(10));
    assert_eq!(data_packet.payload, payload);

    // Test handshake packet building
    let handshake_packet = builder.build_handshake_packet(cid, PathId(20), payload)?;
    assert_eq!(handshake_packet.header.packet_type, PacketType::Handshake);
    assert_eq!(handshake_packet.header.path_id, PathId(20));
    assert_eq!(handshake_packet.payload, payload);

    Ok(())
}

#[test]
fn test_builder_security_validation() {
    let mut builder = ExtendedPacketBuilder::new();
    let cid = ConnectionId::random();
    let oversized_payload = vec![0u8; MAX_PAYLOAD_SIZE + 1];

    // Test oversized data packet
    let result = builder.build_data_packet(cid, PathId(1), &oversized_payload);
    assert!(result.is_err(), "Should reject oversized data packet");

    // Test oversized handshake packet
    let result = builder.build_handshake_packet(cid, PathId(1), &oversized_payload);
    assert!(result.is_err(), "Should reject oversized handshake packet");
}

#[test]
fn test_builder_encode_reuse() -> Result<()> {
    let mut builder = ExtendedPacketBuilder::new();
    let cid = ConnectionId::random();
    let payload = b"Reuse test payload";

    let packet = builder.build_data_packet(cid, PathId(15), payload)?;
    
    // Test multiple reuses
    let encoded1 = builder.encode_reuse(&packet)?.clone();
    let encoded2 = builder.encode_reuse(&packet)?.clone();
    let encoded3 = builder.encode_reuse(&packet)?.clone();

    // All should be identical
    assert_eq!(encoded1, encoded2);
    assert_eq!(encoded2, encoded3);
    assert_eq!(encoded1.len(), EXTENDED_HEADER_SIZE + payload.len());

    // Test that buffer reuse works with different packets
    let payload2 = b"Different payload for second test";
    let packet2 = builder.build_data_packet(cid, PathId(25), payload2)?;
    let encoded4 = builder.encode_reuse(&packet2)?;
    
    assert_ne!(encoded1, encoded4);
    assert_eq!(encoded4.len(), EXTENDED_HEADER_SIZE + payload2.len());

    Ok(())
}

#[test]
fn test_performance_characteristics() -> Result<()> {
    const NUM_ITERATIONS: usize = 1000;
    let mut builder = ExtendedPacketBuilder::new();
    let cid = ConnectionId::random();
    let payload = vec![0xAB; 500]; // Medium-sized payload

    // Test encoding performance
    let start = Instant::now();
    for i in 0..NUM_ITERATIONS {
        let packet = builder.build_data_packet(cid, PathId(i as u8), &payload)?;
        let _encoded = builder.encode_reuse(&packet)?;
    }
    let encoding_duration = start.elapsed();

    println!("Encoding {} packets took: {:?}", NUM_ITERATIONS, encoding_duration);
    println!("Average encoding time: {:?}", encoding_duration / NUM_ITERATIONS as u32);

    // Test decoding performance
    let packet = builder.build_data_packet(cid, PathId(100), &payload)?;
    let encoded_bytes = packet.encode()?;

    let start = Instant::now();
    for _ in 0..NUM_ITERATIONS {
        let _decoded = ExtendedPacket::decode(encoded_bytes.clone())?;
    }
    let decoding_duration = start.elapsed();

    println!("Decoding {} packets took: {:?}", NUM_ITERATIONS, decoding_duration);
    println!("Average decoding time: {:?}", decoding_duration / NUM_ITERATIONS as u32);

    Ok(())
}

#[test]
fn test_edge_cases_and_boundary_conditions() -> Result<()> {
    let cid = ConnectionId::random();

    // Test empty payload
    let empty_packet = ExtendedPacketBuilder::new()
        .build_data_packet(cid, PathId(0), &[])?;
    assert_eq!(empty_packet.payload.len(), 0);
    assert_eq!(empty_packet.header.length, 0);

    let encoded = empty_packet.encode()?;
    let decoded = ExtendedPacket::decode(encoded)?;
    assert_eq!(empty_packet, decoded);

    // Test maximum payload size
    let max_payload = vec![0x42; MAX_PAYLOAD_SIZE];
    let max_packet = ExtendedPacketBuilder::new()
        .build_data_packet(cid, PathId(255), &max_payload)?;
    assert_eq!(max_packet.payload.len(), MAX_PAYLOAD_SIZE);
    assert_eq!(max_packet.size(), MAX_PACKET_SIZE);

    let encoded = max_packet.encode()?;
    let decoded = ExtendedPacket::decode(encoded)?;
    assert_eq!(max_packet, decoded);

    // Test all packet types
    let payload = b"test".to_vec();
    for (i, packet_type) in [
        PacketType::Initial,
        PacketType::Retry,
        PacketType::Handshake,
        PacketType::Application,
    ].iter().enumerate() {
        let header = ExtendedPacketHeader::new(
            cid,
            *packet_type,
            PacketFlags::default(),
            PathId(i as u8),
            payload.len() as u16,
        )?;
        let packet = ExtendedPacket::new(header, payload.clone())?;
        
        let encoded = packet.encode()?;
        let decoded = ExtendedPacket::decode(encoded)?;
        assert_eq!(packet, decoded);
        assert_eq!(decoded.header.packet_type, *packet_type);
    }

    Ok(())
}

#[test]
fn test_protocol_compliance() -> Result<()> {
    // Test that packet format matches specification exactly
    let cid = ConnectionId::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    let header = ExtendedPacketHeader::new(
        cid,
        PacketType::Application, // Type = 3 (0b11)
        PacketFlags::new(0x15),  // Flags = 0x15 (0b010101)
        PathId(42),              // Path ID = 42
        1234,                    // Length = 1234
    )?;

    let encoded = header.encode()?;
    
    // Verify byte layout according to specification
    assert_eq!(&encoded[0..12], cid.as_bytes()); // Bytes 0-11: CID
    assert_eq!(encoded[12], 0xD5); // Byte 12: Type(11) + Flags(010101) = 11010101 = 0xD5
    assert_eq!(encoded[13], 42);   // Byte 13: Path ID
    assert_eq!(&encoded[14..16], &1234u16.to_be_bytes()); // Bytes 14-15: Length (big-endian)

    Ok(())
}

#[test]
fn test_memory_safety() -> Result<()> {
    // Test with various buffer sizes to ensure no buffer overflows
    let cid = ConnectionId::random();
    
    for size in [0, 1, 64, 127, 128, 255, 256, 512, 1024, MAX_PAYLOAD_SIZE] {
        let payload = vec![0x55; size];
        let packet = ExtendedPacketBuilder::new()
            .build_data_packet(cid, PathId(size as u8), &payload)?;
        
        // Verify no corruption
        assert_eq!(packet.payload.len(), size);
        assert!(packet.payload.iter().all(|&b| b == 0x55));
        
        // Test roundtrip
        let encoded = packet.encode()?;
        let decoded = ExtendedPacket::decode(encoded)?;
        assert_eq!(packet, decoded);
    }

    Ok(())
}

#[test]
fn test_concurrent_operations() -> Result<()> {
    use std::sync::Arc;
    use std::thread;

    let cid = ConnectionId::random();
    let cid_arc = Arc::new(cid);
    let payload = Arc::new(vec![0xCC; 100]);

    // Test concurrent packet creation and encoding
    let handles: Vec<_> = (0..10).map(|i| {
        let cid = Arc::clone(&cid_arc);
        let payload = Arc::clone(&payload);
        
        thread::spawn(move || -> Result<()> {
            let mut builder = ExtendedPacketBuilder::new();
            
            for j in 0..100 {
                let packet = builder.build_data_packet(*cid, PathId((i * 100 + j) as u8), &payload)?;
                let encoded = packet.encode()?;
                let decoded = ExtendedPacket::decode(encoded)?;
                assert_eq!(packet, decoded);
            }
            
            Ok(())
        })
    }).collect();

    // Wait for all threads and check results
    for handle in handles {
        handle.join().unwrap()?;
    }

    Ok(())
}
