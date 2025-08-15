#![forbid(unsafe_code)]

//! Comprehensive unit tests for Plugin Framework CBOR implementation
//!
//! This test suite verifies the complete CBOR parsing and serialization
//! functionality of the Plugin Framework, including edge cases, security
//! validation, and integration with the dispatcher.

use super::*;
use crate::frame::*;
use crate::plugin_cbor::{
    self, parse_plugin_header, serialize_plugin_header, PluginCborError, PluginHeader,
    MAX_CBOR_HEADER_SIZE, MAX_PLUGIN_DATA_SIZE,
};
use crate::plugin_dispatch::*;
use crate::plugin_registry::{Permission, PluginInfo, PluginRegistry};
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Test fixture for plugin CBOR tests
struct PluginCborTestFixture {
    dispatcher: PluginDispatcher,
    registry: Arc<Mutex<PluginRegistry>>,
}

impl PluginCborTestFixture {
    async fn new() -> Self {
        let registry = Arc::new(Mutex::new(PluginRegistry::new()));
        let dispatcher = PluginDispatcher::new(Arc::clone(&registry));

        Self {
            dispatcher,
            registry,
        }
    }

    async fn register_test_plugin(&self, plugin_id: PluginId) -> PluginInfo {
        let plugin_info = PluginInfo {
            id: plugin_id,
            name: format!("test_plugin_{}", plugin_id),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            permissions: vec![
                Permission::DataAccess,
                Permission::Handshake,
                Permission::Control,
            ],
            author: "test".to_string(),
            config_schema: Default::default(),
            supported_frames: vec![FRAME_TYPE_PLUGIN_DATA],
            required: false,
            signature_b64: None,
            registry_pubkey_b64: None,
        };

        let mut registry = self.registry.lock().await;
        registry
            .register(plugin_info.clone())
            .await
            .expect("Plugin registration");
        plugin_info
    }
}

#[tokio::test]
async fn test_cbor_header_creation_and_validation() {
    // Valid plugin header creation
    let plugin_data = b"Hello, Plugin World!".to_vec();
    let header =
        PluginHeader::new(12345, 0x42, plugin_data.clone()).expect("Valid header creation");

    assert_eq!(header.id, 12345);
    assert_eq!(header.flags, 0x42);
    assert_eq!(header.data, plugin_data);
    assert_eq!(header.data_size(), plugin_data.len());
}

#[tokio::test]
async fn test_cbor_reserved_plugin_id_rejection() {
    let data = b"test data".to_vec();

    // Test lower bound of reserved range
    let result = PluginHeader::new(0xFFFF0000, 0x00, data.clone());
    assert!(matches!(
        result,
        Err(PluginCborError::ReservedPluginId(0xFFFF0000))
    ));

    // Test upper bound of reserved range
    let result = PluginHeader::new(0xFFFFFFFF, 0x00, data.clone());
    assert!(matches!(
        result,
        Err(PluginCborError::ReservedPluginId(0xFFFFFFFF))
    ));

    // Test just below reserved range (should succeed)
    let result = PluginHeader::new(0xFFFEFFFF, 0x00, data);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_cbor_data_size_limits() {
    // Test maximum allowed data size
    let max_data = vec![0u8; MAX_PLUGIN_DATA_SIZE];
    let result = PluginHeader::new(100, 0x00, max_data);
    assert!(result.is_ok());

    // Test oversized data rejection
    let oversized_data = vec![0u8; MAX_PLUGIN_DATA_SIZE + 1];
    let result = PluginHeader::new(100, 0x00, oversized_data);
    assert!(matches!(
        result,
        Err(PluginCborError::DataSizeExceeded(_, _))
    ));
}

#[tokio::test]
async fn test_cbor_serialization_roundtrip() {
    let test_cases = vec![
        (1, 0x00, b"".to_vec()),                                       // Empty data
        (42, 0xFF, b"single byte".to_vec()), // Small data with all flags set
        (65535, 0x55, b"A".repeat(1000).into()), // Medium data
        (1000000, 0xAA, b"X".repeat(MAX_PLUGIN_DATA_SIZE / 2).into()), // Large data
    ];

    for (id, flags, data) in test_cases {
        let original_header = PluginHeader::new(id, flags, data).expect("Valid header");

        // Serialize to CBOR
        let cbor_bytes = serialize_plugin_header(&original_header).expect("Serialization");

        // Verify serialized size is within limits
        assert!(
            cbor_bytes.len() <= MAX_CBOR_HEADER_SIZE,
            "Serialized size {} exceeds limit {}",
            cbor_bytes.len(),
            MAX_CBOR_HEADER_SIZE
        );

        // Parse back from CBOR
        let parsed_header = parse_plugin_header(&cbor_bytes).expect("Parsing");

        // Verify roundtrip integrity
        assert_eq!(original_header, parsed_header);
    }
}

#[tokio::test]
async fn test_cbor_malformed_data_rejection() {
    let malformed_test_cases = vec![
        vec![],                    // Empty data
        vec![0xFF],                // Single invalid byte
        vec![0xFF, 0xFE, 0xFD],    // Invalid CBOR sequence
        vec![0x00; 10],            // Zero padding
        b"not cbor data".to_vec(), // Plain text
    ];

    for malformed_cbor in malformed_test_cases {
        let result = parse_plugin_header(&malformed_cbor);
        assert!(
            matches!(result, Err(PluginCborError::InvalidFormat(_))),
            "Expected InvalidFormat error for data: {:?}",
            malformed_cbor
        );
    }
}

#[tokio::test]
async fn test_cbor_header_size_limits() {
    // Create header that approaches the CBOR header size limit
    let large_data = vec![0u8; 900]; // Just under the practical limit
    let header = PluginHeader::new(999, 0x00, large_data).expect("Valid header");

    let cbor_bytes = serialize_plugin_header(&header).expect("Serialization should work");
    assert!(cbor_bytes.len() <= MAX_CBOR_HEADER_SIZE);

    // Verify parsing works
    let parsed_header = parse_plugin_header(&cbor_bytes).expect("Parsing should work");
    assert_eq!(header, parsed_header);
}

#[tokio::test]
async fn test_plugin_header_flag_operations() {
    let mut header = PluginHeader::new(555, 0x00, vec![1, 2, 3]).expect("Valid header");

    // Test individual flag bit operations
    for bit in 0..8 {
        // Set flag bit
        header.set_flag(bit);
        assert!(header.has_flag(bit), "Flag bit {} should be set", bit);
        assert_eq!(header.flags & (1 << bit), 1 << bit);

        // Clear flag bit
        header.clear_flag(bit);
        assert!(!header.has_flag(bit), "Flag bit {} should be cleared", bit);
        assert_eq!(header.flags & (1 << bit), 0);
    }

    // Test multiple flags
    header.set_flag(0);
    header.set_flag(3);
    header.set_flag(7);

    assert_eq!(header.flags, 0x89); // 0b10001001
    assert!(header.has_flag(0));
    assert!(header.has_flag(3));
    assert!(header.has_flag(7));
    assert!(!header.has_flag(1));
    assert!(!header.has_flag(4));
}

#[tokio::test]
async fn test_bytes_wrapper_parsing() {
    let test_data = b"Bytes wrapper test data".to_vec();
    let header = PluginHeader::new(777, 0x33, test_data).expect("Valid header");

    let cbor_bytes = serialize_plugin_header(&header).expect("Serialization");

    // Test with Bytes wrapper
    let bytes_wrapper = Bytes::from(cbor_bytes.clone());
    let parsed_header = parse_plugin_header_bytes(&bytes_wrapper).expect("Bytes parsing");

    assert_eq!(header, parsed_header);

    // Test with slice
    let parsed_header_slice = parse_plugin_header(&cbor_bytes).expect("Slice parsing");
    assert_eq!(header, parsed_header_slice);

    // Both methods should produce identical results
    assert_eq!(parsed_header, parsed_header_slice);
}

#[tokio::test]
async fn test_plugin_dispatcher_cbor_integration() {
    let fixture = PluginCborTestFixture::new().await;
    let plugin_id = 12345;

    // Register a test plugin
    let plugin_info = fixture.register_test_plugin(plugin_id).await;
    fixture
        .dispatcher
        .load_plugin(plugin_info)
        .await
        .expect("Plugin loading");

    // Create valid plugin frame with CBOR header
    let plugin_data = b"Test plugin message data".to_vec();
    let plugin_header = PluginHeader::new(plugin_id, 0x01, plugin_data).expect("Valid header");
    let cbor_frame_data = serialize_plugin_header(&plugin_header).expect("CBOR serialization");

    // Test dispatching different frame types
    let frame_types = vec![
        FRAME_TYPE_PLUGIN_HANDSHAKE,
        FRAME_TYPE_PLUGIN_DATA,
        FRAME_TYPE_PLUGIN_CONTROL,
        FRAME_TYPE_PLUGIN_ERROR,
    ];

    for frame_type in frame_types {
        let result = fixture
            .dispatcher
            .dispatch_plugin_frame(frame_type, cbor_frame_data.clone())
            .await;
        assert!(
            result.is_ok(),
            "Frame type 0x{:02X} dispatch should succeed",
            frame_type
        );
    }

    // Verify statistics
    let stats = fixture.dispatcher.get_stats().await;
    assert_eq!(stats.total_dispatched_frames, 4);
    assert_eq!(stats.active_plugins, 1);
}

#[tokio::test]
async fn test_plugin_dispatcher_invalid_frame_type() {
    let fixture = PluginCborTestFixture::new().await;
    let plugin_id = 12345;

    // Register a test plugin
    let plugin_info = fixture.register_test_plugin(plugin_id).await;
    fixture
        .dispatcher
        .load_plugin(plugin_info)
        .await
        .expect("Plugin loading");

    // Create plugin frame data
    let plugin_data = b"test data".to_vec();
    let plugin_header = PluginHeader::new(plugin_id, 0x00, plugin_data).expect("Valid header");
    let cbor_frame_data = serialize_plugin_header(&plugin_header).expect("CBOR serialization");

    // Test invalid frame types (outside plugin range)
    let invalid_frame_types = vec![0x00, 0x01, 0x4F, 0x60, 0xFF];

    for frame_type in invalid_frame_types {
        let result = fixture
            .dispatcher
            .dispatch_plugin_frame(frame_type, cbor_frame_data.clone())
            .await;
        assert!(
            matches!(result, Err(DispatchError::InvalidFrameType(_))),
            "Frame type 0x{:02X} should be rejected",
            frame_type
        );
    }
}

#[tokio::test]
async fn test_plugin_dispatcher_unregistered_plugin() {
    let fixture = PluginCborTestFixture::new().await;
    let unregistered_plugin_id = 99999;

    // Create plugin frame for unregistered plugin
    let plugin_data = b"test data".to_vec();
    let plugin_header =
        PluginHeader::new(unregistered_plugin_id, 0x00, plugin_data).expect("Valid header");
    let cbor_frame_data = serialize_plugin_header(&plugin_header).expect("CBOR serialization");

    // Attempt to dispatch to unregistered plugin
    let result = fixture
        .dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, cbor_frame_data)
        .await;
    assert!(matches!(result, Err(DispatchError::PluginNotRegistered(_))));

    // Verify error statistics
    let stats = fixture.dispatcher.get_stats().await;
    assert_eq!(stats.total_errors, 1);
}

#[tokio::test]
async fn test_plugin_message_types() {
    let plugin_data = b"test message".to_vec();
    let plugin_header = PluginHeader::new(123, 0x42, plugin_data.clone()).expect("Valid header");
    let raw_frame = vec![1, 2, 3, 4];

    // Test handshake message
    let handshake_msg = PluginMessage::new(
        FRAME_TYPE_PLUGIN_HANDSHAKE,
        plugin_header.clone(),
        raw_frame.clone(),
    );
    assert!(handshake_msg.is_handshake());
    assert!(!handshake_msg.is_data());
    assert!(!handshake_msg.is_control());
    assert!(!handshake_msg.is_error());
    assert_eq!(handshake_msg.plugin_id(), 123);

    // Test data message
    let data_msg = PluginMessage::new(
        FRAME_TYPE_PLUGIN_DATA,
        plugin_header.clone(),
        raw_frame.clone(),
    );
    assert!(!data_msg.is_handshake());
    assert!(data_msg.is_data());
    assert!(!data_msg.is_control());
    assert!(!data_msg.is_error());

    // Test control message
    let control_msg = PluginMessage::new(
        FRAME_TYPE_PLUGIN_CONTROL,
        plugin_header.clone(),
        raw_frame.clone(),
    );
    assert!(!control_msg.is_handshake());
    assert!(!control_msg.is_data());
    assert!(control_msg.is_control());
    assert!(!control_msg.is_error());

    // Test error message
    let error_msg = PluginMessage::new(FRAME_TYPE_PLUGIN_ERROR, plugin_header, raw_frame);
    assert!(!error_msg.is_handshake());
    assert!(!error_msg.is_data());
    assert!(!error_msg.is_control());
    assert!(error_msg.is_error());
}

#[tokio::test]
async fn test_plugin_dispatcher_permission_validation() {
    let fixture = PluginCborTestFixture::new().await;
    let plugin_id = 12345;

    // Register plugin with limited permissions (no Control permission)
    let mut plugin_info = PluginInfo {
        id: plugin_id,
        name: "limited_plugin".to_string(),
        version: "1.0.0".to_string(),
        description: "test".to_string(),
        permissions: vec![Permission::DataAccess, Permission::Handshake], // No Control permission
        author: "test".to_string(),
        config_schema: Default::default(),
        supported_frames: vec![FRAME_TYPE_PLUGIN_DATA],
        required: false,
        signature_b64: None,
        registry_pubkey_b64: None,
    };

    {
        let mut registry = fixture.registry.lock().await;
        registry
            .register(plugin_info.clone())
            .await
            .expect("Plugin registration");
    }

    fixture
        .dispatcher
        .load_plugin(plugin_info)
        .await
        .expect("Plugin loading");

    // Create plugin frame data
    let plugin_data = b"control command".to_vec();
    let plugin_header = PluginHeader::new(plugin_id, 0x00, plugin_data).expect("Valid header");
    let cbor_frame_data = serialize_plugin_header(&plugin_header).expect("CBOR serialization");

    // Test control frame (should fail due to insufficient permissions)
    let result = fixture
        .dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, cbor_frame_data.clone())
        .await;
    assert!(matches!(
        result,
        Err(DispatchError::InsufficientPermissions(_))
    ));

    // Test data frame (should succeed)
    let result = fixture
        .dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, cbor_frame_data)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_plugin_dispatcher_capacity_limits() {
    let fixture = PluginCborTestFixture::new().await;

    // Load plugins up to the capacity limit (32)
    for i in 1..=32 {
        let plugin_info = fixture.register_test_plugin(i).await;
        let result = fixture.dispatcher.load_plugin(plugin_info).await;
        assert!(result.is_ok(), "Plugin {} should load successfully", i);
    }

    // Attempt to load one more plugin (should fail)
    let plugin_info = fixture.register_test_plugin(33).await;
    let result = fixture.dispatcher.load_plugin(plugin_info).await;
    assert!(matches!(result, Err(DispatchError::CapacityExceeded(32))));

    // Verify statistics
    let stats = fixture.dispatcher.get_stats().await;
    assert_eq!(stats.active_plugins, 32);
}

#[tokio::test]
async fn test_plugin_dispatcher_shutdown() {
    let fixture = PluginCborTestFixture::new().await;

    // Load several test plugins
    let plugin_ids = vec![100, 200, 300];
    for plugin_id in &plugin_ids {
        let plugin_info = fixture.register_test_plugin(*plugin_id).await;
        fixture
            .dispatcher
            .load_plugin(plugin_info)
            .await
            .expect("Plugin loading");
    }

    // Verify plugins are loaded
    let stats = fixture.dispatcher.get_stats().await;
    assert_eq!(stats.active_plugins, 3);

    // Shutdown all plugins
    fixture.dispatcher.shutdown().await;

    // Verify all plugins are unloaded
    let stats = fixture.dispatcher.get_stats().await;
    assert_eq!(stats.active_plugins, 0);
}

#[tokio::test]
async fn test_edge_case_empty_plugin_data() {
    // Test with completely empty plugin data
    let empty_header = PluginHeader::new(42, 0x00, vec![]).expect("Empty data should be allowed");

    assert_eq!(empty_header.data_size(), 0);
    assert!(empty_header.data.is_empty());

    // Test serialization roundtrip with empty data
    let cbor_bytes = serialize_plugin_header(&empty_header).expect("Serialization");
    let parsed_header = parse_plugin_header(&cbor_bytes).expect("Parsing");

    assert_eq!(empty_header, parsed_header);
}

#[tokio::test]
async fn test_concurrent_plugin_operations() {
    use tokio::task::JoinSet;

    let fixture = Arc::new(PluginCborTestFixture::new().await);

    // Spawn multiple concurrent tasks
    let mut join_set = JoinSet::new();

    // Task 1: Load plugins
    let fixture_clone = Arc::clone(&fixture);
    join_set.spawn(async move {
        for i in 1..=10 {
            let plugin_info = fixture_clone.register_test_plugin(i).await;
            fixture_clone
                .dispatcher
                .load_plugin(plugin_info)
                .await
                .expect("Plugin loading");
        }
    });

    // Task 2: Dispatch messages
    let fixture_clone = Arc::clone(&fixture);
    join_set.spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await; // Let some plugins load first

        for i in 1..=5 {
            let plugin_data = format!("concurrent message {}", i).into_bytes();
            let plugin_header = PluginHeader::new(i, 0x00, plugin_data).expect("Valid header");
            let cbor_frame_data =
                serialize_plugin_header(&plugin_header).expect("CBOR serialization");

            let _ = fixture_clone
                .dispatcher
                .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_DATA, cbor_frame_data)
                .await;
        }
    });

    // Wait for all tasks to complete
    while let Some(result) = join_set.join_next().await {
        result.expect("Task should complete successfully");
    }

    // Verify final state
    let stats = fixture.dispatcher.get_stats().await;
    assert!(stats.active_plugins > 0);
    assert!(stats.total_dispatched_frames >= 5);
}
