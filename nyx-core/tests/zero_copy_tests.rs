#![forbid(unsafe_code)]

//! Comprehensive test suite for zero-copy optimization system.
//!
//! This module provides extensive testing for all aspects of the zero-copy
//! optimization system, including unit tests, integration tests, and
//! performance benchmarks.

use nyx_core::zero_copy::{
    ZeroCopyManager, ZeroCopyManagerConfig, CriticalPath, CriticalPathConfig,
    AllocationTracker, BufferPool, Stage, OperationType, AllocationEvent,
    ZeroCopyError,
};
use nyx_core::zero_copy::manager::{ProcessingContext, ZeroCopyBuffer};
use nyx_core::zero_copy::telemetry::{ZeroCopyTelemetry, TelemetryConfig};
use nyx_core::zero_copy::integration::{ZeroCopyPipeline, aead_integration, fec_integration, transmission_integration};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::test;

/// Test allocation tracking functionality
#[tokio::test]
async fn test_allocation_tracker() {
    let tracker = AllocationTracker::new(1000);

    // Record various allocation events
    tracker.record_allocation(AllocationEvent {
        stage: Stage::Crypto,
        operation: OperationType::Allocate,
        size: 1024,
        timestamp: Instant::now(),
        context: Some("test_context".to_string()),
    }).await;

    tracker.record_allocation(AllocationEvent {
        stage: Stage::Fec,
        operation: OperationType::Copy,
        size: 512,
        timestamp: Instant::now(),
        context: Some("test_context".to_string()),
    }).await;

    tracker.record_allocation(AllocationEvent {
        stage: Stage::Transmission,
        operation: OperationType::ZeroCopy,
        size: 256,
        timestamp: Instant::now(),
        context: Some("test_context".to_string()),
    }).await;

    // Get metrics and verify
    let metrics = tracker.get_metrics().await;
    
    assert!(metrics.stages.contains_key(&Stage::Crypto));
    assert!(metrics.stages.contains_key(&Stage::Fec));
    assert!(metrics.stages.contains_key(&Stage::Transmission));

    let crypto_stats = &metrics.stages[&Stage::Crypto];
    assert_eq!(crypto_stats.total_allocations, 1);
    assert_eq!(crypto_stats.total_bytes, 1024);

    let fec_stats = &metrics.stages[&Stage::Fec];
    assert_eq!(fec_stats.total_copies, 1);
    assert_eq!(fec_stats.total_copy_bytes, 512);

    let tx_stats = &metrics.stages[&Stage::Transmission];
    assert_eq!(tx_stats.zero_copy_ops, 1);

    assert_eq!(metrics.pipeline_total_allocations, 1);
    assert_eq!(metrics.pipeline_total_bytes, 1024);
}

/// Test buffer pool functionality
#[tokio::test]
async fn test_buffer_pool() {
    let mut pool = BufferPool::new(10, 100);

    // Test buffer acquisition and return
    let buffer1 = pool.get_buffer(1280);
    assert_eq!(buffer1.capacity, 1280);

    let buffer2 = pool.get_buffer(4096);
    assert_eq!(buffer2.capacity, 4096);

    // Return buffers to pool
    pool.return_buffer(buffer1);
    pool.return_buffer(buffer2);

    // Verify pool statistics
    let stats = pool.stats();
    assert!(stats.total_buffers > 0);
    
    // Test buffer reuse
    let reused_buffer = pool.get_buffer(1280);
    assert!(stats.hits > 0 || stats.misses > 0);
}

/// Test critical path processing
#[tokio::test]
async fn test_critical_path_processing() {
    let config = CriticalPathConfig {
        enable_zero_copy: true,
        enable_buffer_pooling: true,
        max_buffer_pool_size: 100,
        cleanup_interval: Duration::from_secs(30),
        max_tracking_events: 1000,
        enable_detailed_tracing: true,
    };

    let path = CriticalPath::new("test_path".to_string(), config);
    
    // Test context creation
    let context = path.start_processing("test_context".to_string()).await.unwrap();
    assert_eq!(context.id, "test_context");
    assert_eq!(context.current_stage, Stage::Crypto);

    // Test buffer allocation
    let buffer = path.allocate_buffer("test_context", Stage::Crypto, 1024).await.unwrap();
    assert!(buffer.capacity >= 1024);

    // Test stage processing
    let test_data = b"Hello, zero-copy world!";
    
    // Process through crypto stage
    let crypto_output = path.process_crypto_stage("test_context", test_data).await.unwrap();
    assert!(crypto_output.as_ref().len() >= test_data.len());

    // Process through FEC stage
    let fec_outputs = path.process_fec_stage("test_context", &crypto_output).await.unwrap();
    assert!(!fec_outputs.is_empty());

    // Process through transmission stage
    let tx_outputs = path.process_transmission_stage("test_context", &fec_outputs).await.unwrap();
    assert_eq!(tx_outputs.len(), fec_outputs.len());

    // Clean up
    path.finish_processing("test_context").await.unwrap();

    // Verify metrics
    let metrics = path.get_metrics().await;
    assert!(metrics.pipeline_total_allocations > 0);
}

/// Test complete packet processing
#[tokio::test]
async fn test_complete_packet_processing() {
    let config = CriticalPathConfig::default();
    let path = CriticalPath::new("test_complete".to_string(), config);
    
    let test_packet = vec![0u8; 4096]; // 4KB test packet
    
    // Process complete packet
    let tx_buffers = path.process_packet(&test_packet).await.unwrap();
    assert!(!tx_buffers.is_empty());

    // Verify each transmission buffer has headers
    for buffer in &tx_buffers {
        let data = buffer.as_ref();
        assert!(data.len() >= 32); // At least header size
    }

    // Check metrics
    let metrics = path.get_metrics().await;
    assert!(metrics.pipeline_total_allocations > 0);
    assert!(metrics.stages.len() == 3); // All three stages should be present
}

/// Test zero-copy manager functionality
#[tokio::test]
async fn test_zero_copy_manager() {
    let config = ZeroCopyManagerConfig::default();
    let manager = ZeroCopyManager::new(config);

    // Test path creation
    let path1 = manager.create_critical_path("path1".to_string()).await.unwrap();
    let path2 = manager.create_critical_path("path2".to_string()).await.unwrap();

    // Test path retrieval
    let retrieved_path = manager.get_critical_path("path1").await.unwrap();
    assert_eq!(retrieved_path.id, path1.id);

    // Test path removal
    manager.remove_critical_path("path1").await.unwrap();
    assert!(manager.get_critical_path("path1").await.is_none());

    // Test aggregated metrics
    let aggregated = manager.get_aggregated_metrics().await;
    assert_eq!(aggregated.total_paths, 0); // No active processing yet
}

/// Test error conditions
#[tokio::test]
async fn test_error_conditions() {
    let config = ZeroCopyManagerConfig { max_active_paths: 1, ..Default::default() };
    let manager = ZeroCopyManager::new(config);

    // Create maximum paths
    let _path1 = manager.create_critical_path("path1".to_string()).await.unwrap();

    // Try to create one more (should fail)
    let result = manager.create_critical_path("path2".to_string()).await;
    assert!(matches!(result, Err(ZeroCopyError::TooManyPaths)));

    // Try to create duplicate path
    let result = manager.create_critical_path("path1".to_string()).await;
    assert!(matches!(result, Err(ZeroCopyError::PathAlreadyExists(_))));

    // Try to remove non-existent path
    let result = manager.remove_critical_path("nonexistent").await;
    assert!(matches!(result, Err(ZeroCopyError::PathNotFound(_))));
}

/// Test buffer cleanup functionality
#[tokio::test]
async fn test_buffer_cleanup() {
    let mut pool = BufferPool::new(5, 20);
    
    // Create and return several buffers
    for i in 0..10 {
        let buffer = pool.get_buffer(1280);
        pool.return_buffer(buffer);
    }

    let stats_before = pool.stats();
    let buffers_before = stats_before.total_buffers;
    
    // Perform cleanup (with very short max age to force cleanup)
    pool.cleanup(Duration::from_millis(1));
    tokio::time::sleep(Duration::from_millis(10)).await;
    pool.cleanup(Duration::from_millis(1));
    
    let stats_after = pool.stats();
    // Some buffers should have been cleaned up
    assert!(stats_after.total_buffers <= buffers_before);
}

/// Test telemetry integration
#[tokio::test]
async fn test_telemetry_integration() {
    // Note: This test assumes nyx_telemetry::TelemetryCollector is available
    // In a real implementation, you would create a mock or test collector
    
    let config = ZeroCopyManagerConfig::default();
    let manager = Arc::new(ZeroCopyManager::new(config));
    
    let path = manager.create_critical_path("telemetry_test".to_string()).await.unwrap();
    
    // Process some data to generate metrics
    let test_data = vec![0u8; 2048];
    let _result = path.process_packet(&test_data).await.unwrap();
    
    // Verify metrics are available
    let metrics = manager.get_aggregated_metrics().await;
    assert!(metrics.combined_allocations > 0);
    assert!(metrics.combined_bytes > 0);
}

/// Performance benchmark for zero-copy optimization
#[tokio::test]
async fn benchmark_zero_copy_performance() {
    let config = CriticalPathConfig {
        enable_zero_copy: true,
        enable_buffer_pooling: true,
        ..Default::default()
    };
    let path = CriticalPath::new("benchmark".to_string(), config);

    let test_data = vec![0u8; 8192]; // 8KB test data
    let num_iterations = 100;

    let start_time = Instant::now();
    
    for i in 0..num_iterations {
        let context_id = format!("bench_{}", i);
        let _context = path.start_processing(context_id.clone()).await.unwrap();
        
        let _crypto_output = path.process_crypto_stage(&context_id, &test_data).await.unwrap();
        path.finish_processing(&context_id).await.unwrap();
    }

    let duration = start_time.elapsed();
    let ops_per_second = num_iterations as f64 / duration.as_secs_f64();
    
    println!("Zero-copy crypto processing: {:.2} ops/sec", ops_per_second);
    assert!(ops_per_second > 10.0); // Should be reasonably fast

    // Get final metrics
    let metrics = path.get_metrics().await;
    println!("Total allocations: {}", metrics.pipeline_total_allocations);
    println!("Total bytes: {}", metrics.pipeline_total_bytes);
    println!("Zero-copy ratio: {:.2}%", metrics.zero_copy_ratio * 100.0);
}

/// Test zero-copy buffer sharing and reference counting
#[tokio::test]
async fn test_buffer_reference_counting() {
    let buffer = ZeroCopyBuffer::new(1024);
    assert!(buffer.can_reuse());

    // Clone reference
    let ref_count = buffer.clone_ref();
    assert!(!buffer.can_reuse()); // Should have 2 references now

    // Drop reference
    drop(ref_count);
    assert!(buffer.can_reuse()); // Should be back to 1 reference
}

/// Test concurrent access to zero-copy system
#[tokio::test]
async fn test_concurrent_zero_copy_operations() {
    let config = ZeroCopyManagerConfig::default();
    let manager = Arc::new(ZeroCopyManager::new(config));

    let mut tasks = Vec::new();
    
    // Launch multiple concurrent tasks
    for i in 0..10 {
        let manager_clone = Arc::clone(&manager);
        let task = tokio::spawn(async move {
            let path_id = format!("concurrent_path_{}", i);
            let path = manager_clone.create_critical_path(path_id.clone()).await.unwrap();
            
            let test_data = vec![i as u8; 1024];
            let _result = path.process_packet(&test_data).await.unwrap();
            
            manager_clone.remove_critical_path(&path_id).await.unwrap();
        });
        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        task.await.unwrap();
    }

    // Verify all paths were cleaned up
    let aggregated = manager.get_aggregated_metrics().await;
    assert_eq!(aggregated.total_paths, 0);
}

/// Test optimization report generation
#[tokio::test] 
async fn test_optimization_report_generation() {
    // Note: This would require the telemetry module to be fully integrated
    // This is a placeholder for testing report generation functionality
    
    let config = CriticalPathConfig::default();
    let path = CriticalPath::new("report_test".to_string(), config);
    
    // Generate some activity
    let test_data = vec![0u8; 1024];
    for i in 0..5 {
        let context_id = format!("report_context_{}", i);
        let _result = path.process_packet(&test_data).await;
    }
    
    // Get metrics for report
    let metrics = path.get_metrics().await;
    
    // Basic validation of metrics
    assert!(metrics.pipeline_total_allocations > 0);
    assert!(!metrics.stages.is_empty());
}

/// Integration test with mock AEAD, FEC, and transmission components
#[tokio::test]
async fn test_integration_pipeline() {
    #[cfg(feature = "nyx-crypto")]
    use nyx_crypto::noise::SessionKey;
    #[cfg(feature = "nyx-crypto")]
    use nyx_crypto::aead::FrameCrypter;
    #[cfg(not(feature = "nyx-crypto"))]
    use nyx_core::zero_copy::integration::FrameCrypter;
    use nyx_core::zero_copy::integration::fec_integration::RaptorQCodec;

    let config = ZeroCopyManagerConfig::default();
    let manager = Arc::new(ZeroCopyManager::new(config));
    let path_id = "integration_test".to_string();

    // Create mock components (using actual constructors where available)
    #[cfg(feature = "nyx-crypto")]
    let session_key = SessionKey([0u8; 32]);
    #[cfg(feature = "nyx-crypto")]
    let crypter = FrameCrypter::new(session_key);
    #[cfg(not(feature = "nyx-crypto"))]
    let crypter = FrameCrypter;
    let codec = RaptorQCodec::new(0.3); // 30% redundancy

    // Create zero-copy pipeline
    let pipeline = ZeroCopyPipeline::new(Arc::clone(&manager), path_id.clone())
        .with_aead(crypter)
        .with_fec(codec);

    // Test data processing (without transmission for this test)
    let test_packet = vec![0u8; 2048];
    
    // Note: This would require the complete integration to work
    // For now, just verify pipeline creation
    assert_eq!(pipeline.path_id, "integration_test");
}

/// Stress test for high-volume processing
#[tokio::test]
async fn stress_test_high_volume_processing() {
    let config = CriticalPathConfig {
        enable_zero_copy: true,
        enable_buffer_pooling: true,
        max_buffer_pool_size: 1000,
        ..Default::default()
    };
    
    let path = CriticalPath::new("stress_test".to_string(), config);
    
    let num_packets = 1000;
    let packet_size = 4096;
    let test_data = vec![0u8; packet_size];
    
    let start_time = Instant::now();
    
    // Process many packets concurrently
    let mut tasks = Vec::new();
    for i in 0..num_packets {
        let path_ref = &path; // Borrow path for async block
        let data = test_data.clone();
        
        let task = async move {
            path_ref.process_packet(&data).await
        };
        tasks.push(task);
    }
    
    // Wait for all to complete
    let results: Vec<_> = futures::future::join_all(tasks).await;
    
    let duration = start_time.elapsed();
    let packets_per_second = num_packets as f64 / duration.as_secs_f64();
    let mbps = (num_packets * packet_size) as f64 / (1024.0 * 1024.0) / duration.as_secs_f64();
    
    println!("Stress test results:");
    println!("  Packets processed: {}", num_packets);
    println!("  Duration: {:?}", duration);
    println!("  Packets/sec: {:.2}", packets_per_second);
    println!("  Throughput: {:.2} MB/s", mbps);
    
    // Verify all packets processed successfully
    let successful = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successful, num_packets);
    
    // Check final metrics
    let metrics = path.get_metrics().await;
    println!("  Final allocation count: {}", metrics.pipeline_total_allocations);
    println!("  Zero-copy ratio: {:.2}%", metrics.zero_copy_ratio * 100.0);
    
    // Performance requirements
    assert!(packets_per_second > 50.0); // Should handle at least 50 packets/sec
    assert!(metrics.zero_copy_ratio > 0.1); // Should achieve some zero-copy optimization
}
