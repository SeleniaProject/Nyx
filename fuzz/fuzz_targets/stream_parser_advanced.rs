#![no_main]

use libfuzzer_sys::fuzz_target;
use nyx_stream::frame::{Frame, FrameType, FrameCodec};
use nyx_stream::parser::{StreamParser, ParseResult, ParseError};
use nyx_stream::capability::{Capability, negotiate};

fuzz_target!(|data: &[u8]| {
    // Skip inputs that are too small
    if data.is_empty() {
        return;
    }

    // Create stream parser
    let mut parser = StreamParser::new();

    // Test frame parsing with input data
    match parser.parse_frame(data) {
        Ok(ParseResult::Complete(frame)) => {
            // Successfully parsed frame, test frame operations
            
            // Test frame validation
            let _ = frame.validate();
            
            // Test frame serialization
            if let Ok(serialized) = frame.serialize() {
                // Test round-trip parsing
                let _ = parser.parse_frame(&serialized);
            }

            // Test frame type-specific operations
            match frame.frame_type() {
                FrameType::Data => {
                    let _ = frame.payload();
                    let _ = frame.stream_id();
                }
                FrameType::Control => {
                    let _ = frame.control_flags();
                    let _ = frame.sequence_number();
                }
                FrameType::Capability => {
                    if let Ok(cap) = Capability::from_frame(&frame) {
                        let _ = cap.validate();
                    }
                }
                FrameType::Heartbeat => {
                    let _ = frame.timestamp();
                }
                FrameType::Close => {
                    let _ = frame.close_reason();
                }
            }
        }
        Ok(ParseResult::Incomplete(needed)) => {
            // Parser needs more data
            if data.len() + needed <= 65536 {
                // Simulate providing additional data
                let additional_data = vec![0u8; needed];
                let mut extended_data = data.to_vec();
                extended_data.extend_from_slice(&additional_data);
                let _ = parser.parse_frame(&extended_data);
            }
        }
        Err(ParseError::InvalidFormat) => {
            // Expected for most random inputs
        }
        Err(ParseError::UnsupportedVersion) => {
            // Expected for some inputs
        }
        Err(ParseError::ChecksumMismatch) => {
            // Expected for corrupted data
        }
        Err(_) => {
            // Other errors are acceptable
        }
    }

    // Test streaming parser with fragmented input
    if data.len() > 4 {
        let mut streaming_parser = StreamParser::new_streaming();
        
        // Split input into small chunks to test streaming
        let chunk_size = (data.len() / 8).max(1).min(16);
        for chunk in data.chunks(chunk_size) {
            match streaming_parser.feed_data(chunk) {
                Ok(frames) => {
                    // Process any complete frames
                    for frame in frames {
                        let _ = frame.validate();
                    }
                }
                Err(_) => {
                    // Parser error, reset and continue
                    streaming_parser.reset();
                }
            }
        }
    }

    // Test frame construction from fuzzing data
    if data.len() >= 8 {
        let frame_type_byte = data[0] % 5; // 5 frame types
        let frame_type = match frame_type_byte {
            0 => FrameType::Data,
            1 => FrameType::Control,
            2 => FrameType::Capability,
            3 => FrameType::Heartbeat,
            4 => FrameType::Close,
            _ => FrameType::Data,
        };

        let stream_id = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
        let sequence = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        // Use remaining data as payload
        let payload = if data.len() > 8 { &data[8..] } else { &[] };

        // Test frame builder
        let frame_result = Frame::builder()
            .frame_type(frame_type)
            .stream_id(stream_id)
            .sequence_number(sequence)
            .payload(payload.to_vec())
            .build();

        if let Ok(frame) = frame_result {
            // Test frame codec operations
            let mut codec = FrameCodec::new();
            if let Ok(encoded) = codec.encode(&frame) {
                let _ = codec.decode(&encoded);
            }
        }
    }

    // Test capability negotiation with fuzzing data
    if data.len() >= 16 {
        // Create mock capabilities from input data
        let mut local_caps = Vec::new();
        let mut peer_caps = Vec::new();

        // Extract capability IDs from input
        for chunk in data.chunks(4) {
            if chunk.len() == 4 {
                let cap_id = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let cap = Capability::required(cap_id, vec![]);
                
                if local_caps.len() < 5 {
                    local_caps.push(cap_id);
                }
                if peer_caps.len() < 5 {
                    peer_caps.push(cap.clone());
                }
            }
        }

        // Test capability negotiation
        if !local_caps.is_empty() && !peer_caps.is_empty() {
            let _ = negotiate(&local_caps, &peer_caps);
        }
    }

    // Test error handling with various malformed inputs
    let error_test_cases = [
        &data[..data.len().min(1)],  // Too short
        &vec![0xFF; data.len().min(1024)], // All 0xFF
        &vec![0x00; data.len().min(1024)], // All zeros
    ];

    for test_case in &error_test_cases {
        let _ = parser.parse_frame(test_case);
    }
});
