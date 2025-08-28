#![no_main]

use libfuzzer_sys::fuzz_target;
use nyx_stream::plugin::{PluginMessage, PluginParser, PluginConfig, PluginError};

fuzz_target!(|data: &[u8]| {
    // Skip empty or too small inputs
    if data.is_empty() || data.len() < 4 {
        return;
    }

    // Create plugin parser with default configuration
    let config = PluginConfig {
        max_message_size: 65536,
        timeout_ms: 1000,
        enable_compression: true,
        validate_signatures: false, // Disable for fuzzing performance
        allowed_plugin_ids: vec![], // Allow all for fuzzing
    };

    let mut parser = PluginParser::new(config);

    // Test parsing the input data as a plugin message
    match parser.parse_message(data) {
        Ok(message) => {
            // Successfully parsed, test message operations
            
            // Test message serialization
            if let Ok(serialized) = message.serialize() {
                // Test round-trip parsing
                let _ = parser.parse_message(&serialized);
            }

            // Test message validation
            let _ = message.validate();

            // Test message type handling
            match message {
                PluginMessage::Command { plugin_id, command, payload } => {
                    // Test command message operations
                    let _ = parser.validate_plugin_id(plugin_id);
                    let _ = parser.execute_command(command, &payload);
                }
                PluginMessage::Response { request_id, status, data } => {
                    // Test response message operations
                    let _ = parser.handle_response(request_id, status, &data);
                }
                PluginMessage::Event { event_type, timestamp, data } => {
                    // Test event message operations
                    let _ = parser.process_event(event_type, timestamp, &data);
                }
                PluginMessage::Heartbeat { plugin_id, timestamp } => {
                    // Test heartbeat message operations
                    let _ = parser.update_heartbeat(plugin_id, timestamp);
                }
            }
        }
        Err(PluginError::InvalidFormat) => {
            // Expected for most random inputs
        }
        Err(PluginError::UnsupportedVersion) => {
            // Expected for some inputs
        }
        Err(PluginError::MessageTooLarge) => {
            // Expected for oversized inputs
        }
        Err(_) => {
            // Other errors are acceptable
        }
    }

    // Test streaming parser with fragmented input
    if data.len() > 8 {
        let mut streaming_parser = parser.create_streaming_parser();
        
        // Split input into chunks to simulate streaming
        let chunk_size = (data.len() / 4).max(1);
        for chunk in data.chunks(chunk_size) {
            match streaming_parser.feed_data(chunk) {
                Ok(messages) => {
                    // Process any complete messages
                    for msg in messages {
                        let _ = msg.validate();
                    }
                }
                Err(_) => {
                    // Parser error, reset and continue
                    streaming_parser.reset();
                }
            }
        }
    }

    // Test plugin message construction from fuzzing data
    if data.len() >= 16 {
        // Try to construct different message types
        let plugin_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let timestamp = u64::from_le_bytes([
            data[4], data[5], data[6], data[7],
            data[8], data[9], data[10], data[11],
        ]);

        // Test heartbeat message
        let heartbeat = PluginMessage::Heartbeat { plugin_id, timestamp };
        if let Ok(serialized) = heartbeat.serialize() {
            let _ = parser.parse_message(&serialized);
        }

        // Test command message with remaining data as payload
        if data.len() > 16 {
            let command = data[12] % 10; // Command type 0-9
            let payload = data[16..].to_vec();
            
            let command_msg = PluginMessage::Command { 
                plugin_id, 
                command, 
                payload 
            };
            if let Ok(serialized) = command_msg.serialize() {
                let _ = parser.parse_message(&serialized);
            }
        }
    }

    // Test error handling with malformed data
    let malformed_inputs = [
        &data[..data.len().min(3)], // Too short
        &vec![0xFF; data.len()],     // All 0xFF bytes
        &vec![0x00; data.len()],     // All zero bytes
    ];

    for malformed in &malformed_inputs {
        let _ = parser.parse_message(malformed);
    }
});
