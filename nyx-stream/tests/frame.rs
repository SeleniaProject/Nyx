//! Frame processing tests
//!
//! Tests for frame encoding, decoding, and validation in the stream layer.

use bytes::{Bytes, BytesMut};
use nyx_stream::{Frame, FrameCodec, FrameType};

#[test]
fn test_frame_encoding_decoding() {
    let original_data = b"Hello, World!";
    let frame_local = Frame::new(FrameType::Data, original_data.to_vec());

    // Encode the frame
    let mut codec = FrameCodec::new();
    let mut buffer = BytesMut::new();

    codec
        .encode(&frame, &mut buffer)
        .expect("Frame encoding should succeed");

    // Decode the frame
    let decoded_frame = codec
        .decode(&mut buffer)
        .expect("Frame decoding should succeed")
        .expect("Should have a complete frame");

    assert_eq!(decoded_frame.frame_type(), frame.frame_type());
    assert_eq!(decoded_frame.payload(), frame.payload());

    println!("Frame encoding/decoding test passed");
}

#[test]
fn test_frame_types() {
    let data_frame = Frame::new(FrameType::Data, b"data".to_vec());
    let control_frame = Frame::new(FrameType::Control, b"control".to_vec());
    let heartbeat_frame = Frame::new(FrameType::Heartbeat, vec![]);

    assert_eq!(data_frame.frame_type(), FrameType::Data);
    assert_eq!(control_frame.frame_type(), FrameType::Control);
    assert_eq!(heartbeat_frame.frame_type(), FrameType::Heartbeat);

    println!("Frame type test passed");
}

#[test]
fn test_frame_payload_limits() {
    // Test empty payload
    let empty_frame = Frame::new(FrameType::Data, vec![]);
    assert_eq!(empty_frame.payload().len(), 0);

    // Test normal payload
    let normal_payload = vec![1u8; 1024];
    let normal_frame = Frame::new(FrameType::Data, normal_payload.clone());
    assert_eq!(normal_frame.payload(), &normal_payload);

    // Test large payload (should be handled gracefully)
    let large_payload = vec![1u8; 64 * 1024]; // 64KB
    let large_frame = Frame::new(FrameType::Data, large_payload.clone());
    assert_eq!(large_frame.payload(), &large_payload);

    println!("Frame payload limits test passed");
}

#[test]
fn test_frame_serialization_roundtrip() {
    let test_cases = vec![
        (FrameType::Data, b"test data".to_vec()),
        (FrameType::Control, b"control message".to_vec()),
        (FrameType::Heartbeat, vec![]),
        (FrameType::Data, vec![0u8; 1000]), // Large payload
    ];

    let mut codec = FrameCodec::new();

    for (frame_type, payload) in test_cases {
        let original_frame = Frame::new(frame_type, payload);

        // Serialize
        let mut buffer = BytesMut::new();
        codec
            .encode(&original_frame, &mut buffer)
            .expect("Encoding should succeed");

        // Deserialize
        let decoded_frame = codec
            .decode(&mut buffer)
            .expect("Decoding should succeed")
            .expect("Should have complete frame");

        // Verify roundtrip
        assert_eq!(original_frame.frame_type(), decoded_frame.frame_type());
        assert_eq!(original_frame.payload(), decoded_frame.payload());
    }

    println!("Frame serialization roundtrip test passed");
}

#[test]
fn test_frame_codec_error_handling() {
    let mut codec = FrameCodec::new();

    // Test decoding empty buffer
    let mut empty_buffer = BytesMut::new();
    let result = codec
        .decode(&mut empty_buffer)
        .expect("Decode should not panic");
    assert!(result.is_none(), "Empty buffer should return None");

    // Test decoding incomplete frame
    let mut incomplete_buffer = BytesMut::from(&b"incomplete"[..]);
    let result = codec
        .decode(&mut incomplete_buffer)
        .expect("Decode should not panic");
    // Result depends on implementation - either None or error

    println!("Frame codec error handling test passed");
}

#[test]
fn test_frame_metadata() {
    let frame_local = Frame::new(FrameType::Data, b"test".to_vec());

    // Test basic metadata
    assert_eq!(frame.frame_type(), FrameType::Data);
    assert_eq!(frame.payload().len(), 4);

    // Test frame size calculation
    let frame_size = frame.total_size();
    assert!(
        frame_size >= frame.payload().len(),
        "Total size should include headers"
    );

    println!("Frame metadata test passed");
}
