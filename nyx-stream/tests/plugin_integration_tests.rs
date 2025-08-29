#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_collect,
    clippy::explicit_into_iter_loop,
    clippy::uninlined_format_args,
    clippy::unreachable
)]

#![forbid(unsafe_code)]

//! Integration tests for the complete plugin system

use nyx_stream::plugin::{PluginHeader, PluginId};
use nyx_stream::plugin_dispatch::{DispatchError, PluginDispatcher};
use nyx_stream::plugin_registry::{Permission, PluginInfo, PluginRegistry};
use nyx_stream::plugin_sandbox::SandboxPolicy;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_complete_plugin_lifecycle() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    let plugin_id = PluginId(1);
    let info = PluginInfo::new(
        plugin_id,
        "lifecycle_test_plugin",
        [Permission::DataAccess, Permission::Handshake],
    );

    // Test plugin loading
    dispatcher
        .load_plugin(info)
        .await
        .expect("Failed to load plugin");
    assert!(dispatcher.is_plugin_loaded(plugin_id).await);
    assert_eq!(dispatcher.loaded_plugin_count().await, 1);

    // Test plugin messaging
    let header = PluginHeader {
        id: plugin_id,
        flags: 0,
        data: vec![1, 2, 3, 4],
    };

    let mut header_bytes = Vec::new();
    ciborium::ser::into_writer(&header, &mut header_bytes).expect("Failed to serialize header");

    // Test handshake frame dispatch
    dispatcher
        .dispatch_plugin_frame(0x51, header_bytes.clone())
        .await
        .expect("Failed to dispatch handshake frame");

    // Test data frame dispatch
    dispatcher
        .dispatch_plugin_frame(0x52, header_bytes)
        .await
        .expect("Failed to dispatch data frame");

    // Give time for message processing
    sleep(Duration::from_millis(100)).await;

    // Check statistics
    let stats = dispatcher
        .get_plugin_stats(plugin_id)
        .await
        .expect("Failed to get plugin stats");
    assert_eq!(stats.messages_processed, 2);

    let dispatch_stats = dispatcher.get_dispatch_stats().await;
    assert_eq!(dispatch_stats.plugins_loaded, 1);
    assert_eq!(dispatch_stats.frames_dispatched, 2);
    assert_eq!(dispatch_stats.dispatch_errors, 0);

    // Test plugin unloading
    dispatcher
        .unload_plugin(plugin_id)
        .await
        .expect("Failed to unload plugin");
    assert!(!dispatcher.is_plugin_loaded(plugin_id).await);
    assert_eq!(dispatcher.loaded_plugin_count().await, 0);

    let final_stats = dispatcher.get_dispatch_stats().await;
    assert_eq!(final_stats.plugins_unloaded, 1);
}

#[tokio::test]
async fn test_plugin_sandbox_integration() {
    let policy = SandboxPolicy::permissive();

    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new_with_sandbox(registry.clone(), policy);

    let plugin_id = PluginId(2);
    let info = PluginInfo::new(plugin_id, "sandbox_test_plugin", [Permission::DataAccess]);

    // Load plugin with sandbox
    dispatcher
        .load_plugin(info)
        .await
        .expect("Failed to load sandboxed plugin");
    assert!(dispatcher.is_plugin_loaded(plugin_id).await);

    // Test message dispatch
    let header = PluginHeader {
        id: plugin_id,
        flags: 0,
        data: vec![5, 6, 7, 8],
    };

    let mut header_bytes = Vec::new();
    ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

    dispatcher
        .dispatch_plugin_frame(0x52, header_bytes)
        .await
        .expect("Failed to dispatch to sandboxed plugin");

    dispatcher
        .unload_plugin(plugin_id)
        .await
        .expect("Failed to unload sandboxed plugin");
}

#[tokio::test]
async fn test_permission_enforcement() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    let plugin_id = PluginId(3);
    let info = PluginInfo::new(
        plugin_id,
        "limited_permission_plugin",
        [Permission::Handshake], // Only handshake permission
    );

    dispatcher
        .load_plugin(info)
        .await
        .expect("Failed to load plugin");

    let header = PluginHeader {
        id: plugin_id,
        flags: 0,
        data: vec![9, 10, 11, 12],
    };

    let mut header_bytes = Vec::new();
    ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

    // Should succeed for handshake frame
    let result = dispatcher
        .dispatch_plugin_frame(0x51, header_bytes.clone())
        .await;
    assert!(result.is_ok());

    // Should fail for data frame (lacks DataAccess permission)
    let result = dispatcher.dispatch_plugin_frame(0x52, header_bytes).await;
    assert!(matches!(result, Err(DispatchError::PermissionDenied(_))));

    dispatcher
        .unload_plugin(plugin_id)
        .await
        .expect("Failed to unload plugin");
}

#[tokio::test]
async fn test_multiple_plugins_concurrent() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = Arc::new(PluginDispatcher::new(registry.clone()));

    // Load multiple plugins
    let plugin_ids = vec![PluginId(10), PluginId(11), PluginId(12)];

    for &plugin_id in &plugin_ids {
        let info = PluginInfo::new(
            plugin_id,
            format!("concurrent_plugin_{}", plugin_id.0),
            [
                Permission::DataAccess,
                Permission::Handshake,
                Permission::Control,
            ],
        );
        dispatcher
            .load_plugin(info)
            .await
            .expect("Failed to load plugin");
    }

    assert_eq!(dispatcher.loaded_plugin_count().await, 3);

    // Dispatch messages to all plugins concurrently
    let mut handles = Vec::new();

    for &plugin_id in &plugin_ids {
        let dispatcher_clone = dispatcher.clone();

        let handle = tokio::spawn(async move {
            let header = PluginHeader {
                id: plugin_id,
                flags: 0,
                data: vec![13, 14, 15, 16],
            };

            let mut header_bytes = Vec::new();
            ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

            // Send multiple frame types
            for frame_type in [0x51, 0x52, 0x53] {
                dispatcher_clone
                    .dispatch_plugin_frame(frame_type, header_bytes.clone())
                    .await
                    .expect("Failed to dispatch frame");
            }
        });

        handles.push(handle);
    }

    // Wait for all dispatches to complete
    for handle in handles {
        handle.await.expect("Task failed");
    }

    // Give time for message processing
    sleep(Duration::from_millis(200)).await;

    // Check that all plugins processed messages
    for &plugin_id in &plugin_ids {
        let stats = dispatcher
            .get_plugin_stats(plugin_id)
            .await
            .expect("Failed to get plugin stats");
        assert_eq!(stats.messages_processed, 3);
    }

    let dispatch_stats = dispatcher.get_dispatch_stats().await;
    assert_eq!(dispatch_stats.frames_dispatched, 9); // 3 plugins × 3 frames each

    // Unload all plugins
    for &plugin_id in &plugin_ids {
        dispatcher
            .unload_plugin(plugin_id)
            .await
            .expect("Failed to unload plugin");
    }

    assert_eq!(dispatcher.loaded_plugin_count().await, 0);
}

#[tokio::test]
async fn test_unregistered_plugin_dispatch() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    let plugin_id = PluginId(99);
    let header = PluginHeader {
        id: plugin_id,
        flags: 0,
        data: vec![17, 18, 19, 20],
    };

    let mut header_bytes = Vec::new();
    ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

    // Attempt to dispatch to unregistered plugin
    let result = dispatcher.dispatch_plugin_frame(0x52, header_bytes).await;
    assert!(matches!(result, Err(DispatchError::PluginNotRegistered(_))));

    let dispatch_stats = dispatcher.get_dispatch_stats().await;
    assert_eq!(dispatch_stats.dispatch_errors, 1);
}

#[tokio::test]
async fn test_plugin_message_backpressure() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    let plugin_id = PluginId(4);
    let info = PluginInfo::new(
        plugin_id,
        "backpressure_test_plugin",
        [Permission::DataAccess],
    );

    // Load plugin with very small channel capacity
    dispatcher
        .load_plugin_with_capacity(info, 1)
        .await
        .expect("Failed to load plugin");

    let header = PluginHeader {
        id: plugin_id,
        flags: 0,
        data: vec![21, 22, 23, 24],
    };

    let mut header_bytes = Vec::new();
    ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

    // First message should succeed
    let result = dispatcher
        .dispatch_plugin_frame(0x52, header_bytes.clone())
        .await;
    assert!(result.is_ok());

    // Second message might fail due to backpressure with no-wait dispatch
    let _result = dispatcher
        .dispatch_plugin_framenowait(0x52, header_bytes)
        .await;
    // The result could be Ok or Err depending on timing, but it should not panic

    dispatcher
        .unload_plugin(plugin_id)
        .await
        .expect("Failed to unload plugin");
}

#[tokio::test]
async fn test_plugin_stats_tracking() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    let plugin_id = PluginId(5);
    let info = PluginInfo::new(
        plugin_id,
        "stats_test_plugin",
        [Permission::DataAccess, Permission::Control],
    );

    dispatcher
        .load_plugin(info)
        .await
        .expect("Failed to load plugin");

    let header = PluginHeader {
        id: plugin_id,
        flags: 0,
        data: vec![25, 26, 27, 28, 29, 30, 31, 32], // 8 bytes
    };

    let mut header_bytes = Vec::new();
    ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

    let message_size = header_bytes.len() + header.data.len();

    // Send multiple messages
    for _ in 0..5 {
        dispatcher
            .dispatch_plugin_frame(0x52, header_bytes.clone())
            .await
            .expect("Failed to dispatch frame");
    }

    // Give time for processing
    sleep(Duration::from_millis(100)).await;

    // Check plugin stats
    let stats = dispatcher
        .get_plugin_stats(plugin_id)
        .await
        .expect("Failed to get plugin stats");

    assert_eq!(stats.messages_processed, 5);
    assert_eq!(stats.bytes_processed, (message_size * 5) as u64);
    assert_eq!(stats.errors, 0);

    dispatcher
        .unload_plugin(plugin_id)
        .await
        .expect("Failed to unload plugin");
}

#[tokio::test]
async fn test_loaded_plugins_listing() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    let plugin_ids = vec![PluginId(20), PluginId(21), PluginId(22)];

    // Initially no plugins loaded
    assert!(dispatcher.loaded_plugins().await.is_empty());

    // Load plugins
    for &plugin_id in &plugin_ids {
        let info = PluginInfo::new(
            plugin_id,
            format!("listing_test_plugin_{}", plugin_id.0),
            [Permission::DataAccess],
        );
        dispatcher
            .load_plugin(info)
            .await
            .expect("Failed to load plugin");
    }

    // Check loaded plugins list
    let mut loaded = dispatcher.loaded_plugins().await;
    loaded.sort_by_key(|id| id.0);

    assert_eq!(loaded, plugin_ids);

    // Unload one plugin
    dispatcher
        .unload_plugin(plugin_ids[1])
        .await
        .expect("Failed to unload plugin");

    let mut loaded = dispatcher.loaded_plugins().await;
    loaded.sort_by_key(|id| id.0);

    let expected = vec![plugin_ids[0], plugin_ids[2]];
    assert_eq!(loaded, expected);

    // Unload remaining plugins
    for &plugin_id in &[plugin_ids[0], plugin_ids[2]] {
        dispatcher
            .unload_plugin(plugin_id)
            .await
            .expect("Failed to unload plugin");
    }

    assert!(dispatcher.loaded_plugins().await.is_empty());
}
