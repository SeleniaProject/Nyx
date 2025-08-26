#![no_main]

use libfuzzer_sys::fuzz_target;
use nyx_core::types::{Timestamp, StreamId, Nonce};

fuzz_target!(|data: &[u8]| {
    // Skip inputs that are too small
    if data.len() < 4 {
        return;
    }

    // Test Timestamp parsing from bytes
    if data.len() >= 8 {
        let timestamp_bytes = &data[0..8];
        let timestamp = u64::from_le_bytes([
            timestamp_bytes[0], timestamp_bytes[1], timestamp_bytes[2], timestamp_bytes[3],
            timestamp_bytes[4], timestamp_bytes[5], timestamp_bytes[6], timestamp_bytes[7],
        ]);
        let ts = Timestamp::new(timestamp);
        
        // Test timestamp operations
        let _ = ts.elapsed_ms();
        let _ = ts.to_bytes();
        let _ = ts.is_expired(3600); // 1 hour expiry
    }

    // Test StreamId generation and validation
    if data.len() >= 4 {
        let stream_id_bytes = &data[0..4];
        let stream_id_val = u32::from_le_bytes([
            stream_id_bytes[0], stream_id_bytes[1], stream_id_bytes[2], stream_id_bytes[3],
        ]);
        let stream_id = StreamId::new(stream_id_val);
        
        // Test stream ID operations
        let _ = stream_id.to_bytes();
        let _ = stream_id.is_valid();
    }

    // Test Nonce generation and validation
    if data.len() >= 12 {
        let nonce_bytes = &data[0..12];
        if let Ok(nonce) = Nonce::from_bytes(nonce_bytes) {
            let _ = nonce.to_bytes();
            let _ = nonce.is_valid();
            
            // Test nonce increment
            let _ = nonce.increment();
        }
    }

    // Test various buffer operations with remaining data
    if data.len() > 12 {
        let buffer = &data[12..];
        
        // Test buffer validation
        if buffer.len() > 0 && buffer.len() <= 65536 {
            // Simulate packet processing
            let checksum = buffer.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
            let _ = checksum;
        }
    }
});
