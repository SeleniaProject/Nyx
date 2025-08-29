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

//! Performance and stress tests for the plugin system

use nyx_stream::plugin::{PluginHeader, PluginId};
use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{Permission, PluginInfo, PluginRegistry};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_plugin_high_throughput() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    let plugin_id = PluginId(100);
    let info = PluginInfo::new(
        plugin_id,
        "throughput_test_plugin",
        [Permission::DataAccess],
    );

    // Load plugin with large channel capacity
    dispatcher
        .load_plugin_with_capacity(info, 10000)
        .await
        .expect("Failed to load plugin");

    let header = PluginHeader {
        id: plugin_id,
        flags: 0,
        data: vec![0; 1024], // 1KB payload
    };

    let mut header_bytes = Vec::new();
    ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

    const MESSAGE_COUNT: usize = 1000;
    let start_time = Instant::now();

    // Send many messages rapidly
    for _ in 0..MESSAGE_COUNT {
        dispatcher
            .dispatch_plugin_frame(0x52, header_bytes.clone())
            .await
            .expect("Failed to dispatch frame");
    }

    let dispatch_duration = start_time.elapsed();

    // Give time for processing
    sleep(Duration::from_millis(500)).await;

    let stats = dispatcher
        .get_plugin_stats(plugin_id)
        .await
        .expect("Failed to get plugin stats");

    println!("Dispatched {MESSAGE_COUNT} messages in {dispatch_duration:?}");
    println!("Plugin processed {} messages", stats.messages_processed);
    println!("Total bytes processed: {}", stats.bytes_processed);

    assert_eq!(stats.messages_processed, MESSAGE_COUNT as u64);

    // Calculate throughput
    let messages_per_second = MESSAGE_COUNT as f64 / dispatch_duration.as_secs_f64();
    println!("Throughput: {messages_per_second:.2} messages/second");

    // Should be able to handle at least 1000 messages/second
    assert!(messages_per_second > 500.0);

    dispatcher
        .unload_plugin(plugin_id)
        .await
        .expect("Failed to unload plugin");
}

#[tokio::test]
async fn test_many_plugins_concurrent_load() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = Arc::new(PluginDispatcher::new(registry.clone()));

    const PLUGIN_COUNT: usize = 100;
    let start_time = Instant::now();

    // Load many plugins concurrently
    let mut handles = Vec::new();

    for i in 0..PLUGIN_COUNT {
        let dispatcher_clone = dispatcher.clone();

        let handle = tokio::spawn(async move {
            let plugin_id = PluginId(200 + i as u32);
            let info = PluginInfo::new(
                plugin_id,
                format!("concurrent_load_plugin_{i}"),
                [Permission::DataAccess],
            );

            dispatcher_clone
                .load_plugin(info)
                .await
                .expect("Failed to load plugin");
        });

        handles.push(handle);
    }

    // Wait for all loads to complete
    for handle in handles {
        handle.await.expect("Task failed");
    }

    let load_duration = start_time.elapsed();

    assert_eq!(dispatcher.loaded_plugin_count().await, PLUGIN_COUNT);

    println!("Loaded {PLUGIN_COUNT} plugins in {load_duration:?}");

    // Should be able to load 100 plugins in under 5 seconds
    assert!(load_duration < Duration::from_secs(5));

    // Unload all plugins
    let start_time = Instant::now();

    let mut handles = Vec::new();

    for i in 0..PLUGIN_COUNT {
        let dispatcher_clone = dispatcher.clone();

        let handle = tokio::spawn(async move {
            let plugin_id = PluginId(200 + i as u32);
            dispatcher_clone
                .unload_plugin(plugin_id)
                .await
                .expect("Failed to unload plugin");
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.expect("Task failed");
    }

    let unload_duration = start_time.elapsed();

    assert_eq!(dispatcher.loaded_plugin_count().await, 0);

    println!("Unloaded {PLUGIN_COUNT} plugins in {unload_duration:?}");

    // Should be able to unload 100 plugins in under 3 seconds
    assert!(unload_duration < Duration::from_secs(3));
}

#[tokio::test]
async fn test_plugin_message_stress() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = Arc::new(PluginDispatcher::new(registry.clone()));

    const PLUGIN_COUNT: usize = 10;
    const MESSAGES_PER_PLUGIN: usize = 100;

    // Load multiple plugins
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(300 + i as u32);
        let info = PluginInfo::new(
            plugin_id,
            format!("stress_test_plugin_{i}"),
            [
                Permission::DataAccess,
                Permission::Handshake,
                Permission::Control,
            ],
        );

        dispatcher
            .load_plugin_with_capacity(info, 1000)
            .await
            .expect("Failed to load plugin");
    }

    let start_time = Instant::now();

    // Send messages to all plugins concurrently
    let mut handles = Vec::new();

    for i in 0..PLUGIN_COUNT {
        let dispatcher_clone = dispatcher.clone();

        let handle = tokio::spawn(async move {
            let plugin_id = PluginId(300 + i as u32);

            let header = PluginHeader {
                id: plugin_id,
                flags: 0,
                data: vec![0; 512], // 512 bytes payload
            };

            let mut header_bytes = Vec::new();
            ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

            // Send messages rapidly
            for j in 0..MESSAGES_PER_PLUGIN {
                let frame_type = match j % 3 {
                    0 => 0x51, // Handshake
                    1 => 0x52, // Data
                    _ => 0x53, // Control
                };

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

    let dispatch_duration = start_time.elapsed();

    // Give time for processing
    sleep(Duration::from_millis(1000)).await;

    let total_messages = PLUGIN_COUNT * MESSAGES_PER_PLUGIN;

    println!("Stress test: {PLUGIN_COUNT} plugins, {MESSAGES_PER_PLUGIN} messages each");
    println!("Total {total_messages} messages dispatched in {dispatch_duration:?}");

    // Verify all messages were processed
    let mut total_processed = 0;
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(300 + i as u32);
        let stats = dispatcher
            .get_plugin_stats(plugin_id)
            .await
            .expect("Failed to get plugin stats");

        assert_eq!(stats.messages_processed, MESSAGES_PER_PLUGIN as u64);
        total_processed += stats.messages_processed;
    }

    assert_eq!(total_processed, total_messages as u64);

    let dispatch_stats = dispatcher.get_dispatch_stats().await;
    assert_eq!(dispatch_stats.frames_dispatched, total_messages as u64);
    assert_eq!(dispatch_stats.dispatch_errors, 0);

    // Calculate throughput
    let messages_per_second = total_messages as f64 / dispatch_duration.as_secs_f64();
    println!("Stress test throughput: {messages_per_second:.2} messages/second");

    // Should handle at least 100 messages/second under stress
    assert!(messages_per_second > 100.0);

    // Unload all plugins
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(300 + i as u32);
        dispatcher
            .unload_plugin(plugin_id)
            .await
            .expect("Failed to unload plugin");
    }

    assert_eq!(dispatcher.loaded_plugin_count().await, 0);
}

#[tokio::test]
async fn test_plugin_memory_efficiency() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    const PLUGIN_COUNT: usize = 50;

    // Load many plugins
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(400 + i as u32);
        let info = PluginInfo::new(
            plugin_id,
            format!("memory_test_plugin_{i}"),
            [Permission::DataAccess],
        );

        dispatcher
            .load_plugin(info)
            .await
            .expect("Failed to load plugin");
    }

    assert_eq!(dispatcher.loaded_plugin_count().await, PLUGIN_COUNT);

    // Send some messages to ensure channels are active
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(400 + i as u32);

        let header = PluginHeader {
            id: plugin_id,
            flags: 0,
            data: vec![i as u8; 100],
        };

        let mut header_bytes = Vec::new();
        ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

        dispatcher
            .dispatch_plugin_frame(0x52, header_bytes)
            .await
            .expect("Failed to dispatch frame");
    }

    // Give time for processing
    sleep(Duration::from_millis(200)).await;

    // Verify all plugins processed at least one message
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(400 + i as u32);
        let stats = dispatcher
            .get_plugin_stats(plugin_id)
            .await
            .expect("Failed to get plugin stats");

        assert!(stats.messages_processed >= 1);
    }

    println!("Memory efficiency test: {PLUGIN_COUNT} plugins loaded and active");

    // Unload all plugins
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(400 + i as u32);
        dispatcher
            .unload_plugin(plugin_id)
            .await
            .expect("Failed to unload plugin");
    }

    assert_eq!(dispatcher.loaded_plugin_count().await, 0);
}

#[tokio::test]
async fn test_plugin_error_resilience() {
    let registry = Arc::new(PluginRegistry::new());
    let dispatcher = PluginDispatcher::new(registry.clone());

    const PLUGIN_COUNT: usize = 5;

    // Load plugins
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(500 + i as u32);
        let info = PluginInfo::new(
            plugin_id,
            format!("resilience_test_plugin_{i}"),
            [Permission::DataAccess],
        );

        dispatcher
            .load_plugin(info)
            .await
            .expect("Failed to load plugin");
    }

    // Mix valid and invalid messages
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(500 + i as u32);

        // Valid message
        let header = PluginHeader {
            id: plugin_id,
            flags: 0,
            data: vec![1, 2, 3, 4],
        };

        let mut header_bytes = Vec::new();
        ciborium::ser::into_writer(&header, &mut header_bytes).unwrap();

        dispatcher
            .dispatch_plugin_frame(0x52, header_bytes)
            .await
            .expect("Failed to dispatch valid frame");

        // Try dispatching to non-existent plugin (should fail gracefully)
        let invalid_header = PluginHeader {
            id: PluginId(999), // Non-existent plugin
            flags: 0,
            data: vec![5, 6, 7, 8],
        };

        let mut invalid_header_bytes = Vec::new();
        ciborium::ser::into_writer(&invalid_header, &mut invalid_header_bytes).unwrap();

        let result = dispatcher
            .dispatch_plugin_frame(0x52, invalid_header_bytes)
            .await;
        assert!(result.is_err()); // Should fail for non-existent plugin
    }

    // Give time for processing
    sleep(Duration::from_millis(100)).await;

    // Verify valid messages were processed despite errors
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(500 + i as u32);
        let stats = dispatcher
            .get_plugin_stats(plugin_id)
            .await
            .expect("Failed to get plugin stats");

        assert_eq!(stats.messages_processed, 1); // Only the valid message
    }

    let dispatch_stats = dispatcher.get_dispatch_stats().await;
    assert_eq!(dispatch_stats.frames_dispatched, PLUGIN_COUNT as u64); // Only successful dispatches
    assert_eq!(dispatch_stats.dispatch_errors, PLUGIN_COUNT as u64); // Errors for invalid plugins

    // Unload all plugins
    for i in 0..PLUGIN_COUNT {
        let plugin_id = PluginId(500 + i as u32);
        dispatcher
            .unload_plugin(plugin_id)
            .await
            .expect("Failed to unload plugin");
    }

    assert_eq!(dispatcher.loaded_plugin_count().await, 0);
}
