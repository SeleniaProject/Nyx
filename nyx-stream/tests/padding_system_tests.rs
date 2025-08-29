#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_collect,
    clippy::explicit_into_iter_loop,
    clippy::uninlined_format_args,
    clippy::unreachable
)]

//! Basic Tests for Padding System
//!
//! This test suite validates the core padding system functionality.

use nyx_stream::frame::{Frame, FrameHeader, FrameType};
use nyx_stream::padding_system::{FramePaddingProcessor, PaddingConfig, PaddingManager};
use std::time::{Duration, Instant};
use tokio::test;
use tracing_test::traced_test;

#[test]
#[traced_test]
async fn test_padding_system_basic_functionality() {
    let config = PaddingConfig::new()
        .target_packet_size(1280)
        .enable_fixed_size(true)
        .burst_protection(true);

    let mut manager = PaddingManager::new(config).unwrap();

    // Test basic padding
    let original_data = b"Hello, Nyx Protocol v1.0!".to_vec();
    let padded_data = manager.pad_data(original_data.clone()).unwrap();

    assert_eq!(padded_data.len(), 1280, "Padded data should be 1280 bytes");
    assert_eq!(
        &padded_data[..original_data.len()],
        &original_data[..],
        "Original data should be preserved"
    );

    // Verify metrics
    let metrics = manager.metrics();
    assert_eq!(metrics.packets_processed, 1);
    assert_eq!(metrics.original_bytes, original_data.len() as u64);
    assert_eq!(metrics.padded_bytes, 1280);
    assert!(metrics.overhead_percentage() > 0.0);
}

#[test]
#[traced_test]
async fn test_timing_obfuscation() {
    let config = PaddingConfig::new()
        .min_delay(Duration::from_millis(5))
        .max_delay(Duration::from_millis(25));

    let mut manager = PaddingManager::new(config).unwrap();

    let start = Instant::now();
    manager.apply_timing_obfuscation().await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed >= Duration::from_millis(5),
        "Timing obfuscation should apply minimum delay"
    );
    assert!(
        elapsed <= Duration::from_millis(100),
        "Timing obfuscation should not exceed reasonable maximum"
    );

    assert_eq!(manager.metrics().timing_obfuscations, 1);
}

#[test]
#[traced_test]
async fn test_burst_detection_and_protection() {
    let config = PaddingConfig::new()
        .burst_protection(true)
        .burst_threshold(5.0);

    let mut manager = PaddingManager::new(config).unwrap();

    // Generate burst traffic
    let mut burst_detected = false;
    for _ in 0..10 {
        if manager.check_burst_protection() {
            burst_detected = true;
        }
    }

    assert!(burst_detected, "Burst should be detected");
    assert!(
        manager.metrics().burst_events > 0,
        "Burst events should be recorded"
    );
}

#[test]
#[traced_test]
async fn test_dummy_traffic_generation() {
    let config = PaddingConfig::new()
        .enable_dummy_traffic(true)
        .dummy_traffic_rate(1000.0); // High rate for testing

    let mut manager = PaddingManager::new(config).unwrap();

    let mut dummy_generated = false;
    for _ in 0..100 {
        if manager.should_generate_dummy() {
            dummy_generated = true;
            break;
        }
    }

    assert!(
        dummy_generated,
        "Dummy traffic should be generated with high rate"
    );

    if dummy_generated {
        let dummy_frame = manager.create_dummy_frame(12345, 1).unwrap();
        assert_eq!(dummy_frame.header.stream_id, 12345);
        assert_eq!(dummy_frame.header.seq, 1);
        assert!(!dummy_frame.payload.is_empty());
    }
}

#[test]
#[traced_test]
async fn test_frame_padding_processor() {
    let config = PaddingConfig::new()
        .target_packet_size(1280)
        .min_delay(Duration::from_millis(1))
        .max_delay(Duration::from_millis(5));

    let mut processor = FramePaddingProcessor::new(config).unwrap();

    // Create test frame
    let frame = Frame {
        header: FrameHeader {
            stream_id: 42,
            seq: 1,
            ty: FrameType::Data,
        },
        payload: b"Test frame data for padding".to_vec(),
    };

    processor.queue_frame(frame);

    let start = Instant::now();
    let processed_frames = processor.process_frames().await.unwrap();
    let elapsed = start.elapsed();

    assert!(
        !processed_frames.is_empty(),
        "Should process at least one frame"
    );
    assert_eq!(
        processed_frames[0].len(),
        1280,
        "Processed frame should be padded"
    );
    assert!(
        elapsed >= Duration::from_millis(1),
        "Should apply timing obfuscation"
    );
}

#[test]
#[traced_test]
async fn test_adaptive_configuration_updates() {
    let initial_config = PaddingConfig::new()
        .target_packet_size(1000)
        .burst_threshold(5.0);

    let mut manager = PaddingManager::new(initial_config).unwrap();

    // Process some data with initial config
    let data1 = vec![0u8; 500];
    let padded1 = manager.pad_data(data1).unwrap();
    assert_eq!(padded1.len(), 1000);

    // Update configuration
    let new_config = PaddingConfig::new()
        .target_packet_size(1500)
        .burst_threshold(10.0);

    manager.update_config(new_config).unwrap();

    // Process data with new config
    let data2 = vec![0u8; 500];
    let padded2 = manager.pad_data(data2).unwrap();
    assert_eq!(padded2.len(), 1500);

    assert_eq!(manager.config().target_packet_size, 1500);
    assert_eq!(manager.config().burst_threshold, 10.0);
}

#[test]
#[traced_test]
async fn test_high_load_performance() {
    let config = PaddingConfig::new()
        .target_packet_size(1280)
        .enable_fixed_size(true);

    let mut manager = PaddingManager::new(config).unwrap();

    let start = Instant::now();

    // Process high volume of data
    for i in 0..1000 {
        let data = vec![0u8; i % 1000 + 100];
        let _padded = manager.pad_data(data).unwrap();
    }

    let elapsed = start.elapsed();

    // Should complete within reasonable time
    assert!(
        elapsed < Duration::from_secs(5),
        "High load processing should complete within 5 seconds, took: {elapsed:?}",
    );

    let metrics = manager.metrics();
    assert_eq!(metrics.packets_processed, 1000);
    assert!(metrics.overhead_ratio > 0.0);
}

#[test]
#[traced_test]
async fn test_error_handling_and_recovery() {
    // Test invalid configuration
    let invalid_config = PaddingConfig::new()
        .target_packet_size(1) // Too small
        .min_delay(Duration::from_millis(100))
        .max_delay(Duration::from_millis(50)); // min > max

    assert!(
        invalid_config.validate().is_err(),
        "Should reject invalid configuration"
    );

    // Test packet too large
    let valid_config = PaddingConfig::new().target_packet_size(100);
    let mut manager = PaddingManager::new(valid_config).unwrap();

    let oversized_data = vec![0u8; 200];
    let result = manager.pad_data(oversized_data);

    assert!(
        result.is_err(),
        "Should reject data larger than target packet size"
    );
}

#[test]
#[traced_test]
async fn test_padding_uniformity() {
    let config = PaddingConfig::new()
        .target_packet_size(1280)
        .enable_fixed_size(true);

    let mut manager = PaddingManager::new(config).unwrap();

    // Test various data sizes
    let test_sizes = [10, 100, 500, 1000, 1200];

    for &size in &test_sizes {
        let data = vec![0u8; size];
        let padded = manager.pad_data(data).unwrap();

        assert_eq!(
            padded.len(),
            1280,
            "All packets should be padded to uniform size"
        );
    }

    let metrics = manager.metrics();
    assert_eq!(metrics.packets_processed, test_sizes.len() as u64);
    assert!(metrics.overhead_ratio > 0.0);
}

#[test]
#[traced_test]
async fn test_anonymity_assessment() {
    let config = PaddingConfig::new()
        .enable_dummy_traffic(true)
        .dummy_traffic_rate(5.0);

    let mut manager = PaddingManager::new(config).unwrap();

    // Process some data to build metrics
    for i in 0..10 {
        let data = vec![0u8; 100 + i * 10];
        manager.pad_data(data).unwrap();
        manager.update_metrics();
    }

    let metrics = manager.metrics();
    // Should have some overhead and timing variance
    assert!(metrics.overhead_ratio > 0.0);
    assert!(manager.is_anonymity_adequate());
}

#[test]
#[traced_test]
async fn test_padding_content_integrity() {
    let config = PaddingConfig::new()
        .target_packet_size(1280)
        .enable_fixed_size(true);

    let mut manager = PaddingManager::new(config).unwrap();

    // Test with various data patterns
    let test_data = vec![
        b"Short".to_vec(),
        (0..255u8).collect::<Vec<u8>>(), // Sequential bytes
        vec![0xAA; 500],                 // Repeated pattern
        vec![0x55; 1000],                // Different repeated pattern
    ];

    for original in test_data {
        let padded = manager.pad_data(original.clone()).unwrap();

        // Verify original data is preserved at the beginning
        assert_eq!(
            &padded[..original.len()],
            &original[..],
            "Original data should be preserved exactly"
        );

        // Verify total size
        assert_eq!(padded.len(), 1280, "Padded size should be consistent");
    }
}

#[test]
#[traced_test]
async fn test_timing_variance() {
    let config = PaddingConfig::new()
        .min_delay(Duration::from_millis(1))
        .max_delay(Duration::from_millis(10));

    let mut manager = PaddingManager::new(config).unwrap();

    let mut delays = Vec::new();

    // Measure multiple timing obfuscation delays
    for _ in 0..20 {
        let start = Instant::now();
        manager.apply_timing_obfuscation().await.unwrap();
        let elapsed = start.elapsed();
        delays.push(elapsed.as_millis() as u64);
    }

    // Should have variance in delays
    let min_delay = *delays.iter().min().unwrap();
    let max_delay = *delays.iter().max().unwrap();

    assert!(min_delay >= 1, "Should respect minimum delay");
    // Allow a bit more headroom on Windows due to coarser timer resolution and scheduler jitter
    let reasonable_max: u64 = if cfg!(windows) { 50 } else { 20 };
    assert!(
        max_delay <= reasonable_max,
        "Should not exceed reasonable maximum"
    );
    assert!(max_delay > min_delay, "Should have timing variance");

    assert_eq!(manager.metrics().timing_obfuscations, 20);
}

#[test]
#[traced_test]
async fn test_comprehensive_padding_scenario() {
    // Test realistic usage scenario

    let config = PaddingConfig::new()
        .target_packet_size(1280)
        .burst_protection(true)
        .enable_dummy_traffic(true)
        .dummy_traffic_rate(2.0)
        .min_delay(Duration::from_millis(1))
        .max_delay(Duration::from_millis(20));

    let mut manager = PaddingManager::new(config).unwrap();

    // Simulate realistic traffic pattern
    let traffic_pattern = [
        (64, 5),   // Small packets (e.g., control messages)
        (200, 3),  // Medium packets (e.g., text messages)
        (800, 2),  // Large packets (e.g., images)
        (1200, 1), // Very large packets (e.g., file chunks)
    ];

    let mut total_processed = 0;

    for &(packet_size, count) in &traffic_pattern {
        for _ in 0..count {
            let data = vec![0u8; packet_size];
            let _padded = manager.pad_data(data).unwrap();
            total_processed += 1;

            // Check burst protection occasionally
            manager.check_burst_protection();

            // Update metrics
            manager.update_metrics();

            // Generate dummy traffic occasionally
            if manager.should_generate_dummy() {
                let _dummy_frame = manager.create_dummy_frame(999, total_processed).unwrap();
            }

            // Small delay between packets
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    // Final validation
    let metrics = manager.metrics();
    assert_eq!(metrics.packets_processed, total_processed);
    assert!(metrics.overhead_ratio > 0.0);
    assert!(metrics.padded_bytes > metrics.original_bytes);

    // Anonymity should be adequate
    assert!(manager.is_anonymity_adequate());

    println!("Comprehensive padding test completed:");
    println!("  - Packets processed: {}", metrics.packets_processed);
    println!("  - Original bytes: {}", metrics.original_bytes);
    println!("  - Padded bytes: {}", metrics.padded_bytes);
    println!("  - Overhead: {:.2}%", metrics.overhead_percentage());
    println!("  - Burst events: {}", metrics.burst_events);
    println!("  - Timing obfuscations: {}", metrics.timing_obfuscations);
}
