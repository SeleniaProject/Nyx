use crate::frame::{Frame, FrameBuilder, FrameHeader, FrameType};
use crate::frame_codec::FrameCodec;
use crate::errors::{Error, Result};
use bytes::BytesMut;
use std::time::Duration;

#[tokio::test]
async fn test_frame_serialization_roundtrip() -> Result<()> {
    let builder = FrameBuilder::new();
    let original_frame = builder.build_data_frame(42, 1234, b"test payload");
    
    let mut buffer = BytesMut::new();
    FrameCodec::encode(&original_frame, &mut buffer)?;
    
    let decoded_frame = FrameCodec::decode(&mut buffer)?
        .ok_or_else(|| Error::InvalidFrame("Failed to decode frame".to_string()))?;
    
    assert_eq!(original_frame.header.stream_id, decoded_frame.header.stream_id);
    assert_eq!(original_frame.header.seq, decoded_frame.header.seq);
    assert_eq!(original_frame.payload, decoded_frame.payload);
    Ok(())
}

#[tokio::test]
async fn test_frame_handler_performance() -> Result<()> {
    let builder = FrameBuilder::new();
    
    // Create 1000 test frames
    let frames: Vec<Frame> = (0..1000)
        .map(|i| {
            let payload_str = format!("payload {}", i);
            builder.build_data_frame(1, i, payload_str.as_bytes())
        })
        .collect();
    
    let start = std::time::Instant::now();
    
    for frame in &frames {
        let mut buffer = BytesMut::new();
        FrameCodec::encode(frame, &mut buffer)?;
        
        let _decoded = FrameCodec::decode(&mut buffer)?;
    }
    
    let elapsed = start.elapsed();
    println!("Processed 1000 frames in {:?}", elapsed);
    
    // Performance requirement: should process 1000 frames in less than 10ms
    assert!(elapsed < Duration::from_millis(10));
    Ok(())
}

#[tokio::test]
async fn test_frame_handler_error_cases() -> Result<()> {
    let mut empty_buffer = BytesMut::new();
    let result = FrameCodec::decode(&mut empty_buffer)?;
    assert!(result.is_none());
    
    // Test invalid frame data
    let mut invalid_buffer = BytesMut::from(&b"invalid frame data"[..]);
    let result = FrameCodec::decode(&mut invalid_buffer);
    assert!(result.is_err());
    
    Ok(())
}
