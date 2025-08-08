#![forbid(unsafe_code)]

//! Integration tests for the complete Plugin Framework implementation.
//!
//! These tests validate the end-to-end functionality of the plugin system,
//! including complex scenarios involving multiple plugins, handshake flows,
//! and error recovery patterns.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tokio::sync::{mpsc, oneshot};

#[cfg(feature = "plugin")]
use nyx_stream::{
    PluginHeader, PluginFrameProcessor, build_plugin_frame,
    PluginHandshakeCoordinator, PluginHandshakeResult, PluginHandshakeError,
    PluginRegistry, PluginInfo, Permission,
    Setting, SettingsFrame, setting_ids, plugin_support_flags, plugin_security_flags,
    plugin_dispatch::PluginDispatcher,
    plugin_ipc::{PluginMessage, PluginResponse},
};

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_end_to_end_plugin_communication() {
    // Set up complete plugin environment
    let mut registry = PluginRegistry::new();
    
    // Register test plugin with comprehensive permissions
    let plugin_info = PluginInfo {
        id: 5001,
        name: "IntegrationTestPlugin".to_string(),
        version: semver::Version::new(1, 0, 0),
        permissions: Permission::RECEIVE_FRAMES | Permission::SEND_FRAMES | Permission::ACCESS_NETWORK,
        description: "Plugin for integration testing".to_string(),
        author: "Integration Test Suite".to_string(),
    };
    
    registry.register(&plugin_info).expect("Plugin registration should succeed");
    
    // Create dispatcher and processor
    let dispatcher = PluginDispatcher::new(registry.clone());
    let mut processor = PluginFrameProcessor::new(registry.clone(), dispatcher);
    
    // Create plugin frame with proper permission flags
    let plugin_header = PluginHeader {
        id: 5001,
        flags: 0x01, // Basic operation flag
        data: b"integration_test_data".to_vec(),
    };
    
    let test_payload = b"This is a comprehensive integration test payload";
    let frame_bytes = build_plugin_frame(
        0x54,
        0x00,
        Some(1u8),
        &plugin_header,
        test_payload,
    ).expect("Frame building should succeed");
    
    // Parse and process the frame
    let parsed_frame = processor.parse_plugin_frame(&frame_bytes)
        .expect("Frame parsing should succeed");
    
    assert_eq!(parsed_frame.plugin_header.id, 5001);
    assert_eq!(parsed_frame.payload, test_payload);
    
    // Process the frame (should succeed with proper permissions)
    let result = processor.process_plugin_frame(parsed_frame).await;
    assert!(result.is_ok(), "Frame processing should succeed: {:?}", result);
    
    // Verify telemetry was updated
    let stats = processor.get_stats();
    assert_eq!(*stats.get(&5001).unwrap_or(&0), 1, "Should record one processed frame");
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_complete_plugin_handshake_flow() {
    // Simulate two peers performing plugin capability negotiation
    
    // Peer A setup
    let registry_a = PluginRegistry::new();
    let mut coordinator_a = PluginHandshakeCoordinator::new(
        registry_a,
        plugin_support_flags::BASIC_FRAMES | plugin_support_flags::DYNAMIC_LOADING,
        plugin_security_flags::REQUIRE_SIGNATURES,
        vec![1001, 1002], // Required plugins
        vec![2001], // Optional plugins
    );
    
    // Peer B setup  
    let registry_b = PluginRegistry::new();
    let mut coordinator_b = PluginHandshakeCoordinator::new(
        registry_b,
        plugin_support_flags::BASIC_FRAMES | plugin_support_flags::DYNAMIC_LOADING,
        plugin_security_flags::REQUIRE_SIGNATURES,
        vec![1001, 1002], // Same required plugins
        vec![2001, 2002], // More optional plugins
    );
    
    // A sends settings to B
    let settings_a = coordinator_a.build_plugin_settings();
    let settings_frame_a = SettingsFrame { settings: settings_a };
    
    // B processes A's settings
    let result_b = coordinator_b.process_peer_settings(&settings_frame_a).await
        .expect("B should process A's settings successfully");
    
    match result_b {
        PluginHandshakeResult::Success { 
            required_plugins, 
            optional_plugins, 
            compatibility_flags 
        } => {
            assert_eq!(required_plugins, 2, "Should negotiate 2 required plugins");
            assert!(optional_plugins >= 1, "Should negotiate at least 1 optional plugin");
            assert!(compatibility_flags & plugin_support_flags::BASIC_FRAMES != 0, 
                   "Should have basic frames compatibility");
        }
        _ => panic!("Expected successful handshake result"),
    }
    
    // B sends settings to A
    let settings_b = coordinator_b.build_plugin_settings();
    let settings_frame_b = SettingsFrame { settings: settings_b };
    
    // A processes B's settings
    let result_a = coordinator_a.process_peer_settings(&settings_frame_b).await
        .expect("A should process B's settings successfully");
    
    match result_a {
        PluginHandshakeResult::Success { .. } => {
            // Both sides should successfully negotiate
        }
        _ => panic!("Expected successful handshake result"),
    }
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_multi_plugin_frame_processing() {
    // Test processing frames from multiple plugins concurrently
    
    let mut registry = PluginRegistry::new();
    
    // Register multiple plugins
    let plugin_configs = vec![
        (6001, "Plugin_A", Permission::RECEIVE_FRAMES),
        (6002, "Plugin_B", Permission::RECEIVE_FRAMES | Permission::SEND_FRAMES),
        (6003, "Plugin_C", Permission::all()),
    ];
    
    for (id, name, permissions) in plugin_configs {
        let plugin_info = PluginInfo {
            id,
            name: name.to_string(),
            version: semver::Version::new(1, 0, 0),
            permissions,
            description: format!("Multi-test plugin {}", name),
            author: "Integration Test Suite".to_string(),
        };
        registry.register(&plugin_info).expect("Plugin registration should succeed");
    }
    
    let dispatcher = PluginDispatcher::new(registry.clone());
    let mut processor = PluginFrameProcessor::new(registry, dispatcher);
    
    // Create frames from each plugin
    let mut test_frames = Vec::new();
    for plugin_id in [6001, 6002, 6003] {
        let plugin_header = PluginHeader {
            id: plugin_id,
            flags: 0x00,
            data: format!("data_from_{}", plugin_id).as_bytes().to_vec(),
        };
        
        let payload = format!("payload_from_plugin_{}", plugin_id);
        let frame_bytes = build_plugin_frame(
            0x55,
            0x00,
            Some((plugin_id % 8) as u8), // Varying path IDs
            &plugin_header,
            payload.as_bytes(),
        ).expect("Frame building should succeed");
        
        test_frames.push((plugin_id, frame_bytes));
    }
    
    // Process all frames
    for (plugin_id, frame_bytes) in test_frames {
        let parsed_frame = processor.parse_plugin_frame(&frame_bytes)
            .expect("Frame parsing should succeed");
        
        assert_eq!(parsed_frame.plugin_header.id, plugin_id);
        
        let result = processor.process_plugin_frame(parsed_frame).await;
        assert!(result.is_ok(), "Processing should succeed for plugin {}", plugin_id);
    }
    
    // Verify all plugins were tracked
    let stats = processor.get_stats();
    for plugin_id in [6001, 6002, 6003] {
        assert_eq!(*stats.get(&plugin_id).unwrap_or(&0), 1, 
                  "Should record one frame for plugin {}", plugin_id);
    }
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_error_recovery() {
    // Test various error conditions and recovery mechanisms
    
    let mut registry = PluginRegistry::new();
    
    // Register plugin with limited permissions
    let plugin_info = PluginInfo {
        id: 7001,
        name: "LimitedPlugin".to_string(),
        version: semver::Version::new(1, 0, 0),
        permissions: Permission::RECEIVE_FRAMES, // No send permission
        description: "Plugin with limited permissions".to_string(),
        author: "Error Recovery Test".to_string(),
    };
    registry.register(&plugin_info).expect("Plugin registration should succeed");
    
    let dispatcher = PluginDispatcher::new(registry.clone());
    let mut processor = PluginFrameProcessor::new(registry, dispatcher);
    
    // Test 1: Frame from unregistered plugin
    let unregistered_header = PluginHeader {
        id: 9999, // Not registered
        flags: 0x00,
        data: b"unregistered".to_vec(),
    };
    
    let frame_bytes = build_plugin_frame(0x56, 0x00, None, &unregistered_header, b"payload")
        .expect("Frame building should succeed");
    
    let parsed_frame = processor.parse_plugin_frame(&frame_bytes)
        .expect("Frame parsing should succeed");
    
    let result = processor.process_plugin_frame(parsed_frame).await;
    assert!(result.is_err(), "Should reject frame from unregistered plugin");
    
    // Test 2: Frame with insufficient permissions
    let limited_header = PluginHeader {
        id: 7001,
        flags: 0x02, // Send flag (not permitted)
        data: b"send_attempt".to_vec(),
    };
    
    let frame_bytes = build_plugin_frame(0x57, 0x00, None, &limited_header, b"payload")
        .expect("Frame building should succeed");
    
    let parsed_frame = processor.parse_plugin_frame(&frame_bytes)
        .expect("Frame parsing should succeed");
    
    let result = processor.process_plugin_frame(parsed_frame).await;
    assert!(result.is_err(), "Should reject frame with insufficient permissions");
    
    // Test 3: Valid frame should still work
    let valid_header = PluginHeader {
        id: 7001,
        flags: 0x00, // No special permissions needed
        data: b"valid_operation".to_vec(),
    };
    
    let frame_bytes = build_plugin_frame(0x58, 0x00, None, &valid_header, b"payload")
        .expect("Frame building should succeed");
    
    let parsed_frame = processor.parse_plugin_frame(&frame_bytes)
        .expect("Frame parsing should succeed");
    
    let result = processor.process_plugin_frame(parsed_frame).await;
    assert!(result.is_ok(), "Valid frame should process successfully");
    
    // Verify only valid frame was recorded in stats
    let stats = processor.get_stats();
    assert_eq!(*stats.get(&7001).unwrap_or(&0), 1, "Should record only valid frame");
    assert_eq!(*stats.get(&9999).unwrap_or(&0), 0, "Should not record invalid frames");
}

#[cfg(feature = "plugin")]
#[tokio::test]
async fn test_plugin_handshake_timeout_and_retry() {
    // Test handshake timeout and retry mechanisms
    
    let registry = PluginRegistry::new();
    let mut coordinator = PluginHandshakeCoordinator::new(
        registry,
        plugin_support_flags::BASIC_FRAMES,
        0,
        vec![1001], // Required plugin
        vec![],
    );
    
    // Create settings that will require plugin negotiation
    let peer_settings = SettingsFrame {
        settings: vec![
            Setting {
                id: setting_ids::PLUGIN_SUPPORT,
                value: plugin_support_flags::BASIC_FRAMES,
            },
            Setting {
                id: setting_ids::PLUGIN_REQUIRED,
                value: 1, // Number of required plugins
            },
        ],
    };
    
    // Process settings multiple times (simulating retries)
    for attempt in 1..=3 {
        let result = coordinator.process_peer_settings(&peer_settings).await
            .expect("Settings processing should succeed");
        
        match result {
            PluginHandshakeResult::Failed { .. } => {
                // Expected since we don't have the required plugin registered
                continue;
            }
            _ => panic!("Expected failed result on attempt {}", attempt),
        }
    }
}

#[cfg(feature = "plugin")]
#[tokio::test] 
async fn test_concurrent_plugin_operations() {
    // Test multiple concurrent plugin operations
    
    let registry = Arc::new(PluginRegistry::new());
    
    // Register test plugin
    let plugin_info = PluginInfo {
        id: 8001,
        name: "ConcurrentTestPlugin".to_string(),
        version: semver::Version::new(1, 0, 0),
        permissions: Permission::all(),
        description: "Plugin for concurrency testing".to_string(),
        author: "Concurrency Test Suite".to_string(),
    };
    registry.register(&plugin_info).expect("Plugin registration should succeed");
    
    let dispatcher = PluginDispatcher::new(registry.clone());
    let processor = Arc::new(PluginFrameProcessor::new(registry, dispatcher));
    
    // Spawn multiple concurrent frame processing tasks
    let mut handles = Vec::new();
    
    for task_id in 0..10 {
        let processor_clone = processor.clone();
        
        let handle = tokio::spawn(async move {
            let plugin_header = PluginHeader {
                id: 8001,
                flags: 0x00,
                data: format!("task_{}", task_id).as_bytes().to_vec(),
            };
            
            let payload = format!("concurrent_payload_{}", task_id);
            let frame_bytes = build_plugin_frame(
                0x59,
                0x00,
                Some(task_id as u8),
                &plugin_header,
                payload.as_bytes(),
            ).expect("Frame building should succeed");
            
            let parsed_frame = processor_clone.parse_plugin_frame(&frame_bytes)
                .expect("Frame parsing should succeed");
            
            processor_clone.process_plugin_frame(parsed_frame).await
                .expect("Frame processing should succeed");
            
            task_id
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    let mut completed_tasks = Vec::new();
    for handle in handles {
        let task_id = handle.await.expect("Task should complete successfully");
        completed_tasks.push(task_id);
    }
    
    // Verify all tasks completed
    assert_eq!(completed_tasks.len(), 10, "All tasks should complete");
    
    // Verify telemetry shows correct number of processed frames
    let stats = processor.get_stats();
    assert_eq!(*stats.get(&8001).unwrap_or(&0), 10, "Should record 10 processed frames");
}

#[cfg(not(feature = "plugin"))]
#[tokio::test]
async fn test_integration_without_plugin_support() {
    // Test that the system works correctly when plugin support is disabled
    
    let coordinator = nyx_stream::PluginHandshakeCoordinator::new();
    let settings = coordinator.build_plugin_settings();
    
    // Should still function but with minimal capabilities
    assert!(!settings.is_empty(), "Should build basic settings");
    assert!(!coordinator.is_plugin_support_active(), "Plugin support should be inactive");
}
