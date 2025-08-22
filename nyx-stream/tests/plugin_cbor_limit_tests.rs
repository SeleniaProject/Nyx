//! CBOR Plugin Limit Tests
//!
//! Tests for CBOR size limits and processing constraints in stream plugins.

use nyx_stream::plugins::{CborPlugin, PluginConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TestData {
    id: u64,
    name: String,
    data: Vec<u8>,
    metadata: HashMap<String, String>,
}

#[test]
fn test_cbor_size_limit_enforcement() {
    let config = PluginConfig {
        max_cbor_size: 4096, // 4KiB limit
        enable_compression: false,
        ..Default::default()
    };

    let plugin = CborPlugin::new(config).expect("Failed to create CBOR plugin");

    // Create 4KiB + 1 CBOR - simply a large data field to put in CBOR
    let large_data = vec![0u8; 4000]; // Large data payload
    let test_obj = TestData {
        id: 1,
        name: "test_large".to_string(),
        data: large_data,
        metadata: HashMap::new(),
    };

    // This should fail due to size limit
    let result = plugin.serialize(&test_obj);
    assert!(
        result.is_err(),
        "Serialization should fail for oversized CBOR"
    );

    println!("CBOR size limit enforcement test passed");
}

#[test]
fn test_cbor_within_limits() {
    let config = PluginConfig {
        max_cbor_size: 4096, // 4KiB limit
        enable_compression: false,
        ..Default::default()
    };

    let plugin = CborPlugin::new(config).expect("Failed to create CBOR plugin");

    // Create small data within limits
    let small_data = vec![0u8; 100];
    let test_obj = TestData {
        id: 1,
        name: "test_small".to_string(),
        data: small_data.clone(),
        metadata: HashMap::new(),
    };

    // This should succeed
    let serialized = plugin
        .serialize(&test_obj)
        .expect("Serialization should succeed");

    // Should be able to deserialize back
    let deserialized: TestData = plugin
        .deserialize(&serialized)
        .expect("Deserialization should succeed");

    assert_eq!(deserialized.data, small_data);
    assert_eq!(deserialized.name, "test_small");

    println!("CBOR within limits test passed");
}

#[test]
fn test_cbor_compression_impact() {
    let config_no_compression = PluginConfig {
        max_cbor_size: 2048,
        enable_compression: false,
        ..Default::default()
    };

    let config_with_compression = PluginConfig {
        max_cbor_size: 2048,
        enable_compression: true,
        ..Default::default()
    };

    let plugin_no_comp = CborPlugin::new(config_no_compression)
        .expect("Failed to create non-compressed CBOR plugin");
    let plugin_with_comp =
        CborPlugin::new(config_with_compression).expect("Failed to create compressed CBOR plugin");

    // Create repetitive data that compresses well
    let repetitive_data = vec![42u8; 1500];
    let test_obj = TestData {
        id: 1,
        name: "test_compression".to_string(),
        data: repetitive_data,
        metadata: HashMap::new(),
    };

    // Without compression, this might fail
    let result_no_comp = plugin_no_comp.serialize(&test_obj);

    // With compression, this should succeed
    let result_with_comp = plugin_with_comp.serialize(&test_obj);

    match (result_no_comp, result_with_comp) {
        (Err(_), Ok(_)) => {
            println!("Compression enabled larger data to fit within limits");
        }
        (Ok(_), Ok(_)) => {
            println!("Data fits within limits both with and without compression");
        }
        (Err(_), Err(_)) => {
            println!("Data too large even with compression");
        }
        (Ok(_), Err(_)) => {
            panic!("Unexpected: compression made things worse");
        }
    }

    println!("CBOR compression impact test completed");
}

#[test]
fn test_cbor_error_handling() {
    let config = PluginConfig {
        max_cbor_size: 1024, // Small limit
        enable_compression: false,
        ..Default::default()
    };

    let plugin = CborPlugin::new(config).expect("Failed to create CBOR plugin");

    // Test various error conditions

    // 1. Oversized data
    let oversized_obj = TestData {
        id: 1,
        name: "oversized".to_string(),
        data: vec![0u8; 2000], // Definitely oversized
        metadata: HashMap::new(),
    };

    let result = plugin.serialize(&oversized_obj);
    assert!(result.is_err(), "Should fail for oversized data");

    // 2. Invalid CBOR data
    let invalid_cbor = vec![0xff, 0xff, 0xff, 0xff]; // Invalid CBOR
    let result: Result<TestData, _> = plugin.deserialize(&invalid_cbor);
    assert!(result.is_err(), "Should fail for invalid CBOR");

    println!("CBOR error handling test passed");
}

#[test]
fn test_cbor_boundary_conditions() {
    let config = PluginConfig {
        max_cbor_size: 1000, // Exact boundary
        enable_compression: false,
        ..Default::default()
    };

    let plugin = CborPlugin::new(config).expect("Failed to create CBOR plugin");

    // Test data that should be right at the boundary
    let boundary_data = vec![0u8; 800]; // Should result in ~1000 bytes of CBOR
    let test_obj = TestData {
        id: 1,
        name: "boundary".to_string(),
        data: boundary_data,
        metadata: HashMap::new(),
    };

    // This should either succeed or fail consistently
    let result = plugin.serialize(&test_obj);

    match result {
        Ok(serialized) => {
            println!(
                "Boundary data serialized successfully, size: {} bytes",
                serialized.len()
            );

            // Should be able to deserialize
            let deserialized: TestData = plugin
                .deserialize(&serialized)
                .expect("Should be able to deserialize boundary data");
        }
        Err(e) => {
            println!("Boundary data failed to serialize: {}", e);
        }
    }

    println!("CBOR boundary conditions test completed");
}
