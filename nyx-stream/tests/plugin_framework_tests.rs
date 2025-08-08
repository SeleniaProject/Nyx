#![forbid(unsafe_code)]

//! Comprehensive test suite for Plugin Framework implementation.
//!
//! These tests validate the complete Plugin Framework implementation including:
//! - Frame Type 0x50-0x5F parsing and processing
//! - CBOR header validation and schema compliance
//! - Plugin capability negotiation and handshake
//! - Permission enforcement and security policies
//! - IPC transport configuration and data flow
//! - Error handling and edge cases

use std::collections::HashMap;
use tokio::sync::mpsc;

#[cfg(feature = "plugin")]
use nyx_stream::{
    PluginHeader, PluginFrameProcessor, PluginFrameResult, PluginFrameError,
    ParsedPluginFrame, build_plugin_frame, validate_plugin_frame_type,
    PluginHandshakeCoordinator, PluginHandshakeResult, PluginHandshakeError,
    PluginRegistry, PluginInfo, Permission,
    Setting, SettingsFrame, setting_ids, plugin_support_flags, plugin_security_flags,
    PLUGIN_FRAME_TYPE_MIN, PLUGIN_FRAME_TYPE_MAX,
};

use nyx_stream::{FrameHeader, build_header_ext};

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_frame_type_validation() {
    // Test valid plugin frame types (0x50-0x5F)
    for frame_type in PLUGIN_FRAME_TYPE_MIN..=PLUGIN_FRAME_TYPE_MAX {
        assert!(
            PluginFrameProcessor::is_plugin_frame_type(frame_type),
            "Frame type 0x{:02X} should be valid plugin frame",
            frame_type
        );
        assert!(
            validate_plugin_frame_type(frame_type).is_ok(),
            "Validation should pass for frame type 0x{:02X}",
            frame_type
        );
    }

    // Test invalid frame types
    let invalid_types = [0x00, 0x01, 0x30, 0x4F, 0x60, 0x70, 0xFF];
    for &frame_type in &invalid_types {
        assert!(
            !PluginFrameProcessor::is_plugin_frame_type(frame_type),
            "Frame type 0x{:02X} should not be valid plugin frame",
            frame_type
        );
        assert!(
            validate_plugin_frame_type(frame_type).is_err(),
            "Validation should fail for frame type 0x{:02X}",
            frame_type
        );
    }
}

#[cfg(feature = "plugin")]
#[tokio::test] 
async fn test_plugin_header_cbor_encoding() {
    let test_cases = vec![
        // Basic header with minimal data
        PluginHeader {
            id: 1001,
            flags: 0x01,
            data: b"".to_vec(),
        },
        // Header with control data
        PluginHeader {
            id: 2002,
            flags: 0x03,
            data: b"control_data".to_vec(),
        },
        // Header with larger payload
        PluginHeader {
            id: 99999,
            flags: 0xFF,
            data: vec![0xAA; 256],
        },
    ];

    for header in test_cases {
        // Test encoding
        let encoded = header.encode();
        assert!(!encoded.is_empty(), "Encoded CBOR should not be empty");

        // Test decoding
        let decoded = PluginHeader::decode(&encoded)
            .expect("Should decode successfully");
        
        assert_eq!(decoded.id, header.id, "Plugin ID should match");
        assert_eq!(decoded.flags, header.flags, "Flags should match");
        assert_eq!(decoded.data, header.data, "Data should match");

        // Test validation (includes schema validation)
        let validated = PluginHeader::validate(&encoded)
            .expect("Should validate successfully");
        
        assert_eq!(validated.id, header.id, "Validated ID should match");
        assert_eq!(validated.flags, header.flags, "Validated flags should match");
        assert_eq!(validated.data, header.data, "Validated data should match");
    }
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_frame_building_and_parsing() {
    let plugin_header = PluginHeader {
        id: 12345,
        flags: 0x01, // Required plugin
        data: b"test_control_data".to_vec(),
    };

    let payload = b"This is the plugin payload data";
    let frame_type = 0x52;
    let frame_flags = 0x00;
    let path_id = Some(7u8);

    // Build complete plugin frame
    let frame_bytes = build_plugin_frame(
        frame_type,
        frame_flags,
        path_id,
        &plugin_header,
        payload,
    ).expect("Frame building should succeed");

    assert!(!frame_bytes.is_empty(), "Frame should not be empty");

    // Parse the frame back
    let processor = PluginFrameProcessor::new();
    let parsed_frame = processor.parse_plugin_frame(&frame_bytes)
        .expect("Frame parsing should succeed");

    // Validate parsed frame structure
    assert_eq!(parsed_frame.frame_header.frame_type, frame_type);
    assert_eq!(parsed_frame.frame_header.flags, frame_flags);
    assert_eq!(parsed_frame.path_id, path_id);
    assert_eq!(parsed_frame.plugin_header.id, plugin_header.id);
    assert_eq!(parsed_frame.plugin_header.flags, plugin_header.flags);
    assert_eq!(parsed_frame.plugin_header.data, plugin_header.data);
    assert_eq!(parsed_frame.payload, payload);
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_frame_size_limits() {
    let plugin_header = PluginHeader {
        id: 1001,
        flags: 0x00,
        data: b"small".to_vec(),
    };

    // Test maximum size limit
    let max_payload = vec![0x42; nyx_stream::plugin_frame::MAX_PLUGIN_PAYLOAD_SIZE - 100]; // Leave room for CBOR header
    let result = build_plugin_frame(0x51, 0x00, None, &plugin_header, &max_payload);
    assert!(result.is_ok(), "Maximum size payload should be accepted");

    // Test oversized payload
    let oversized_payload = vec![0x42; nyx_stream::plugin_frame::MAX_PLUGIN_PAYLOAD_SIZE + 1000];
    let result = build_plugin_frame(0x51, 0x00, None, &plugin_header, &oversized_payload);
    assert!(result.is_err(), "Oversized payload should be rejected");
    
    match result.unwrap_err() {
        PluginFrameError::FrameTooLarge { size, max } => {
            assert!(size > max, "Reported size should exceed maximum");
        }
        _ => panic!("Should return FrameTooLarge error"),
    }
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_permission_enforcement() {
    let mut registry = PluginRegistry::new();
    
    // Register plugin with limited permissions
    let plugin_info = PluginInfo {
        id: 3001,
        name: "TestPlugin".to_string(),
        version: semver::Version::new(1, 0, 0),
        permissions: Permission::RECEIVE_FRAMES, // Only receive permission
        description: "Test plugin for permission enforcement".to_string(),
        author: "Test Suite".to_string(),
    };
    
    registry.register(&plugin_info).expect("Plugin registration should succeed");
    
    // Create dispatcher and processor
    let dispatcher = nyx_stream::plugin_dispatch::PluginDispatcher::new(registry.clone());
    let mut processor = PluginFrameProcessor::new(registry, dispatcher);

    // Test frame with network access flag (should be denied)
    let plugin_header = PluginHeader {
        id: 3001,
        flags: 0x02, // Network access flag
        data: b"network_request".to_vec(),
    };

    let frame_bytes = build_plugin_frame(0x53, 0x00, None, &plugin_header, b"payload")
        .expect("Frame building should succeed");

    let parsed_frame = processor.parse_plugin_frame(&frame_bytes)
        .expect("Frame parsing should succeed");

    // Processing should fail due to insufficient permissions
    let result = processor.process_plugin_frame(parsed_frame).await;
    assert!(result.is_err(), "Should reject frame due to insufficient permissions");
    
    match result.unwrap_err() {
        PluginFrameError::PermissionDenied(plugin_id) => {
            assert_eq!(plugin_id, 3001, "Should report correct plugin ID");
        }
        _ => panic!("Should return PermissionDenied error"),
    }
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_handshake_capability_negotiation() {
    // Create coordinator with plugin requirements
    let registry = PluginRegistry::new();
    let mut coordinator = PluginHandshakeCoordinator::new(
        registry,
        plugin_support_flags::BASIC_FRAMES | plugin_support_flags::DYNAMIC_LOADING,
        plugin_security_flags::REQUIRE_SIGNATURES,
        vec![1001, 1002], // Required plugins
        vec![2001, 2002], // Optional plugins
    );

    // Build our plugin settings
    let our_settings = coordinator.build_plugin_settings();
    assert!(!our_settings.is_empty(), "Should generate plugin settings");

    // Verify our settings contain expected values
    let support_setting = our_settings.iter()
        .find(|s| s.id == setting_ids::PLUGIN_SUPPORT)
        .expect("Should have plugin support setting");
    assert_eq!(
        support_setting.value,
        plugin_support_flags::BASIC_FRAMES | plugin_support_flags::DYNAMIC_LOADING
    );

    // Test compatible peer settings
    let compatible_peer_settings = SettingsFrame {
        settings: vec![
            Setting {
                id: setting_ids::PLUGIN_SUPPORT,
                value: plugin_support_flags::BASIC_FRAMES | plugin_support_flags::DYNAMIC_LOADING,
            },
            Setting {
                id: setting_ids::PLUGIN_SECURITY_POLICY,
                value: plugin_security_flags::REQUIRE_SIGNATURES,
            },
        ],
    };

    let result = coordinator.process_peer_settings(&compatible_peer_settings).await
        .expect("Handshake processing should succeed");

    match result {
        PluginHandshakeResult::Success { required_plugins, optional_plugins, .. } => {
            assert_eq!(required_plugins, 2, "Should negotiate 2 required plugins");
            assert_eq!(optional_plugins, 2, "Should negotiate 2 optional plugins");
        }
        _ => panic!("Expected successful handshake result"),
    }
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_handshake_incompatible_requirements() {
    let registry = PluginRegistry::new();
    let mut coordinator = PluginHandshakeCoordinator::new(
        registry,
        plugin_support_flags::BASIC_FRAMES,
        plugin_security_flags::REQUIRE_SIGNATURES,
        vec![9001], // Required plugin
        vec![],
    );

    // Peer that doesn't support plugin frames at all
    let incompatible_peer_settings = SettingsFrame {
        settings: vec![
            Setting {
                id: setting_ids::PLUGIN_SUPPORT,
                value: 0, // No plugin support
            },
        ],
    };

    let result = coordinator.process_peer_settings(&incompatible_peer_settings).await
        .expect("Handshake processing should succeed");

    match result {
        PluginHandshakeResult::Failed { error, close_payload } => {
            assert!(!close_payload.is_empty(), "Should provide close payload");
            match error {
                PluginHandshakeError::UnsupportedRequiredPlugin { plugin_id } => {
                    assert_eq!(plugin_id, 9001, "Should report unsupported plugin ID");
                }
                _ => panic!("Expected UnsupportedRequiredPlugin error"),
            }
        }
        _ => panic!("Expected failed handshake result"),
    }
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_frame_processing_telemetry() {
    let registry = PluginRegistry::new();
    let dispatcher = nyx_stream::plugin_dispatch::PluginDispatcher::new(registry.clone());
    let mut processor = PluginFrameProcessor::new(registry, dispatcher);

    // Initially no stats
    let stats = processor.get_stats();
    assert!(stats.is_empty(), "Initial stats should be empty");

    // Process some frames (they will be ignored due to unregistered plugins, but stats should update)
    let plugin_header = PluginHeader {
        id: 9999,
        flags: 0x00,
        data: b"test".to_vec(),
    };

    for i in 0..5 {
        let frame_bytes = build_plugin_frame(0x55, 0x00, None, &plugin_header, &format!("payload_{}", i).as_bytes())
            .expect("Frame building should succeed");

        let parsed_frame = processor.parse_plugin_frame(&frame_bytes)
            .expect("Frame parsing should succeed");

        let _ = processor.process_plugin_frame(parsed_frame).await; // Ignore result
    }

    // Check stats updated
    let stats = processor.get_stats();
    assert_eq!(*stats.get(&9999).unwrap_or(&0), 5, "Should record 5 frames for plugin 9999");

    // Reset stats
    processor.reset_stats();
    let stats = processor.get_stats();
    assert!(stats.is_empty(), "Stats should be empty after reset");
}

#[cfg(not(feature = "plugin"))]
#[tokio::test]
async fn test_plugin_framework_disabled() {
    // When plugin feature is disabled, basic functionality should still work
    // but all plugin operations should gracefully fail or be no-ops
    
    let coordinator = nyx_stream::PluginHandshakeCoordinator::new();
    let settings = coordinator.build_plugin_settings();
    
    // Should still build settings (with zero capabilities)
    assert!(!settings.is_empty(), "Should build minimal settings even without plugin support");
    
    // Should report no plugin support active
    assert!(!coordinator.is_plugin_support_active(), "Plugin support should be inactive");
}

#[tokio::test]
async fn test_plugin_frame_type_constants() {
    // Verify frame type constants are correct
    assert_eq!(PLUGIN_FRAME_TYPE_MIN, 0x50, "Minimum plugin frame type should be 0x50");
    assert_eq!(PLUGIN_FRAME_TYPE_MAX, 0x5F, "Maximum plugin frame type should be 0x5F");
    
    // Verify range covers exactly 16 frame types
    assert_eq!(PLUGIN_FRAME_TYPE_MAX - PLUGIN_FRAME_TYPE_MIN + 1, 16, "Should have exactly 16 plugin frame types");
}

#[tokio::test]
async fn test_settings_id_constants() {
    // Verify plugin-related SETTINGS IDs are in correct ranges
    assert_eq!(setting_ids::PLUGIN_SUPPORT, 0x0010);
    assert_eq!(setting_ids::PLUGIN_REQUIRED, 0x0011);
    assert_eq!(setting_ids::PLUGIN_OPTIONAL, 0x0012);
    assert_eq!(setting_ids::PLUGIN_SECURITY_POLICY, 0x0013);
    
    // Verify flags are powers of 2 (valid bitmasks)
    let support_flags = [
        plugin_support_flags::BASIC_FRAMES,
        plugin_support_flags::DYNAMIC_LOADING,
        plugin_support_flags::SANDBOXED_EXECUTION,
        plugin_support_flags::INTER_PLUGIN_IPC,
        plugin_support_flags::PLUGIN_PERSISTENCE,
    ];
    
    for &flag in &support_flags {
        assert!(flag.is_power_of_two(), "Support flag 0x{:08X} should be power of 2", flag);
    }
}
