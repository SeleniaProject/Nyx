#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_collect,
    clippy::explicit_into_iter_loop,
    clippy::uninlined_format_args,
    clippy::unreachable
)]

//! Integration tests for Multipath Data Plane (LARMix++)
//!
//! These tests verify the complete multipath data plane functionality including:
//! - LARMix++ latency-aware routing with dynamic hop count adjustment
//! - Weighted Round Robin path scheduler
//! - Dynamic reordering buffer management
//! - Anti-replay protection for early data
//! - Path quality monitoring and failover
//! - Performance metrics tracking

#![forbid(unsafe_code)]

use nyx_stream::{
    frame::{Frame, FrameHeader, FrameType},
    multipath_dataplane::{
        AntiReplayWindow, ConnectionId, MultipathConfig, MultipathDataPlane, PathId, PathInfo,
        PathMetrics, PathScheduler, PathState, ReorderingBuffer,
    },
};
use std::time::{Duration, Instant};

/// Helper function to create test frame
fn create_test_frame(stream_id: u32, seq: u64, payload: Vec<u8>) -> Frame {
    Frame {
        header: FrameHeader {
            stream_id,
            seq,
            ty: FrameType::Data,
        },
        payload,
    }
}

/// Helper function to create test path info
fn create_test_path_info(path_id: PathId, connection_id: ConnectionId, quality: f64) -> PathInfo {
    PathInfo {
        path_id,
        connection_id,
        state: PathState::Active,
        metrics: PathMetrics {
            rtt_ms: 50.0 + (path_id as f64 * 10.0), // Vary RTT by path
            jitter_ms: 5.0,
            loss_rate: 0.01,
            bandwidth_mbps: 100.0,
            quality,
            hop_count: 5,
            last_measurement: Instant::now(),
            failed_probes: 0,
        },
        weight: 1.0,
        created_at: Instant::now(),
        last_activity: Instant::now(),
    }
}

#[tokio::test]
async fn test_multipath_data_plane_creation() {
    let config = MultipathConfig::default();
    let data_plane = MultipathDataPlane::new(config);

    let (active_count, paths) = data_plane.get_scheduler_stats().await;
    assert_eq!(active_count, 0);
    assert_eq!(paths.len(), 0);
}

#[tokio::test]
async fn test_path_addition_and_removal() {
    let config = MultipathConfig::default();
    let data_plane = MultipathDataPlane::new(config);

    // Add first path
    let path1 = create_test_path_info(1, 123, 0.9);
    assert!(data_plane.add_path(path1).await.is_ok());

    // Add second path
    let path2 = create_test_path_info(2, 123, 0.8);
    assert!(data_plane.add_path(path2).await.is_ok());

    let (active_count, paths) = data_plane.get_scheduler_stats().await;
    assert_eq!(active_count, 2);
    assert_eq!(paths.len(), 2);

    // Remove path
    assert!(data_plane.remove_path(1).await);

    let (active_count, _) = data_plane.get_scheduler_stats().await;
    assert_eq!(active_count, 1);
}

#[tokio::test]
async fn test_weighted_round_robin_scheduling() {
    let config = MultipathConfig::default();
    let data_plane = MultipathDataPlane::new(config);

    // Add paths with different RTTs (weights = inverse RTT)
    let path1 = create_test_path_info(1, 123, 0.9); // RTT: 60ms
    let path2 = create_test_path_info(2, 123, 0.8); // RTT: 70ms
    let path3 = create_test_path_info(3, 123, 0.7); // RTT: 80ms

    assert!(data_plane.add_path(path1).await.is_ok());
    assert!(data_plane.add_path(path2).await.is_ok());
    assert!(data_plane.add_path(path3).await.is_ok());

    // Test path selection - should favor lower RTT paths
    let mut selections = std::collections::HashMap::new();
    for _ in 0..100 {
        if let Some(path_id) = data_plane.select_send_path().await {
            *selections.entry(path_id).or_insert(0) += 1;
        }
    }

    // Path 1 (lowest RTT) should be selected most frequently
    assert!(selections.get(&1).unwrap_or(&0) > selections.get(&3).unwrap_or(&0));
}

#[tokio::test]
async fn test_dynamic_path_metrics_update() {
    let config = MultipathConfig {
        dynamic_hop_count: true,
        min_hop_count: 3,
        max_hop_count: 7,
        ..Default::default()
    };
    let data_plane = MultipathDataPlane::new(config);

    let path1 = create_test_path_info(1, 123, 0.9);
    assert!(data_plane.add_path(path1).await.is_ok());

    // Update with poor conditions (should increase hop count)
    let poor_metrics = PathMetrics {
        rtt_ms: 600.0, // High RTT
        jitter_ms: 50.0,
        loss_rate: 0.15, // High loss
        bandwidth_mbps: 10.0,
        quality: 0.4,
        hop_count: 5,
        last_measurement: Instant::now(),
        failed_probes: 0,
    };

    assert!(data_plane
        .update_path_metrics(1, poor_metrics)
        .await
        .is_ok());

    let (_, paths) = data_plane.get_scheduler_stats().await;
    let updated_path = paths.get(&1).unwrap();
    assert_eq!(updated_path.metrics.hop_count, 6); // Should increase

    // Update with good conditions (should decrease hop count)
    let good_metrics = PathMetrics {
        rtt_ms: 80.0, // Low RTT
        jitter_ms: 2.0,
        loss_rate: 0.005, // Low loss
        bandwidth_mbps: 100.0,
        quality: 0.95,
        hop_count: 6,
        last_measurement: Instant::now(),
        failed_probes: 0,
    };

    assert!(data_plane
        .update_path_metrics(1, good_metrics)
        .await
        .is_ok());

    let (_, paths) = data_plane.get_scheduler_stats().await;
    let updated_path = paths.get(&1).unwrap();
    assert_eq!(updated_path.metrics.hop_count, 5); // Should decrease
}

#[tokio::test]
async fn test_frame_reordering() {
    let config = MultipathConfig::default();
    let data_plane = MultipathDataPlane::new(config);

    // Send frames out of order
    let frame2 = create_test_frame(1, 2, b"frame2".to_vec());
    let frame1 = create_test_frame(1, 1, b"frame1".to_vec());
    let frame0 = create_test_frame(1, 0, b"frame0".to_vec());

    // Process frame 2 first (out of order)
    let delivered = data_plane
        .process_incoming_frame(
            123, // connection_id
            frame2, 2,     // sequence_number
            1,     // direction_id
            2,     // nonce
            false, // is_early_data
        )
        .await
        .unwrap();
    assert_eq!(delivered.len(), 0); // Should be buffered

    // Process frame 0 (expected)
    let delivered = data_plane
        .process_incoming_frame(123, frame0, 0, 1, 0, false)
        .await
        .unwrap();
    assert_eq!(delivered.len(), 1); // Should deliver frame 0

    // Process frame 1 (now in order)
    let delivered = data_plane
        .process_incoming_frame(123, frame1, 1, 1, 1, false)
        .await
        .unwrap();
    assert_eq!(delivered.len(), 2); // Should deliver frame 1 and buffered frame 2
}

#[tokio::test]
async fn test_anti_replay_protection() {
    let config = MultipathConfig {
        enable_early_data: true,
        anti_replay_window_size: 1024,
        ..Default::default()
    };
    let data_plane = MultipathDataPlane::new(config);

    let frame = create_test_frame(1, 0, b"early_data".to_vec());

    // First early data frame should be accepted
    let result = data_plane
        .process_incoming_frame(
            123, // connection_id
            frame.clone(),
            0,    // sequence_number
            1,    // direction_id
            100,  // nonce
            true, // is_early_data
        )
        .await;
    assert!(result.is_ok());

    // Replay same nonce should be rejected
    let result = data_plane
        .process_incoming_frame(
            123, frame, 1, 1, 100, // Same nonce
            true,
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_reorder_buffer_timeout() {
    let config = MultipathConfig {
        reorder_timeout_ms: 100, // Short timeout for testing
        ..Default::default()
    };
    let data_plane = MultipathDataPlane::new(config);

    // Send frame out of order
    let frame = create_test_frame(1, 1, b"timeout_frame".to_vec());
    let delivered = data_plane
        .process_incoming_frame(123, frame, 1, 1, 1, false)
        .await
        .unwrap();
    assert_eq!(delivered.len(), 0); // Should be buffered

    // Wait for timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Check for timed-out frames
    let timed_out = data_plane.check_reorder_timeouts().await;
    assert_eq!(timed_out.len(), 1);
    assert_eq!(timed_out.get(&123).unwrap().len(), 1);
}

#[tokio::test]
async fn test_path_quality_monitoring() {
    let config = MultipathConfig {
        min_path_quality: 0.5,
        ..Default::default()
    };
    let data_plane = MultipathDataPlane::new(config);

    // Add path with good quality
    let good_path = create_test_path_info(1, 123, 0.9);
    assert!(data_plane.add_path(good_path).await.is_ok());

    // Update with degraded quality
    let degraded_metrics = PathMetrics {
        rtt_ms: 200.0,
        jitter_ms: 20.0,
        loss_rate: 0.1,
        bandwidth_mbps: 50.0,
        quality: 0.3, // Below threshold
        hop_count: 5,
        last_measurement: Instant::now(),
        failed_probes: 2,
    };

    assert!(data_plane
        .update_path_metrics(1, degraded_metrics)
        .await
        .is_ok());

    let (_, paths) = data_plane.get_scheduler_stats().await;
    let updated_path = paths.get(&1).unwrap();
    assert!(matches!(updated_path.state, PathState::Degraded));
}

#[tokio::test]
async fn test_metrics_collection() {
    let config = MultipathConfig::default();
    let data_plane = MultipathDataPlane::new(config);

    // Process some frames
    let frame1 = create_test_frame(1, 0, b"test1".to_vec());
    let frame2 = create_test_frame(1, 1, b"test2".to_vec());

    let _ = data_plane
        .process_incoming_frame(123, frame1, 0, 1, 0, false)
        .await;
    let _ = data_plane
        .process_incoming_frame(123, frame2, 1, 1, 1, false)
        .await;

    let metrics = data_plane.get_metrics().await;
    assert_eq!(metrics.frames_received, 2);
    assert_eq!(metrics.frames_in_order, 2);
    assert_eq!(metrics.frames_out_of_order, 0);
}

#[tokio::test]
async fn test_path_scheduler_weight_calculation() {
    let config = MultipathConfig::default();
    let mut scheduler = PathScheduler::new(config);

    // Test weight calculation for different RTT values
    let fast_path = create_test_path_info(1, 123, 0.9); // RTT: 60ms (50 + 1*10)
    let slow_path = create_test_path_info(2, 123, 0.8); // RTT: 70ms (50 + 2*10)

    assert!(scheduler.add_path(fast_path).is_ok());
    assert!(scheduler.add_path(slow_path).is_ok());

    let fast_path_info = scheduler.get_path_info(1).unwrap();
    let slow_path_info = scheduler.get_path_info(2).unwrap();

    println!(
        "Fast path RTT: {}, weight: {}",
        fast_path_info.metrics.rtt_ms, fast_path_info.weight
    );
    println!(
        "Slow path RTT: {}, weight: {}",
        slow_path_info.metrics.rtt_ms, slow_path_info.weight
    );

    // Fast path should have higher weight (weight = 1000/RTT * quality * loss_penalty)
    // Fast: 1000/60 * 0.9 * 0.99 ≈ 14.85
    // Slow: 1000/70 * 0.8 * 0.99 ≈ 11.31
    assert!(fast_path_info.weight > slow_path_info.weight);
}

#[test]
fn test_reordering_buffer_standalone() {
    let mut buffer = ReorderingBuffer::new(1000, 100);

    // Add frames in various orders
    let frame2 = create_test_frame(1, 2, b"frame2".to_vec());
    let frame0 = create_test_frame(1, 0, b"frame0".to_vec());
    let frame1 = create_test_frame(1, 1, b"frame1".to_vec());

    // Add frame 2 (out of order)
    let delivered = buffer.add_frame(frame2, 2);
    assert_eq!(delivered.len(), 0);

    // Add frame 0 (expected)
    let delivered = buffer.add_frame(frame0, 0);
    assert_eq!(delivered.len(), 1);

    // Add frame 1 (completes sequence)
    let delivered = buffer.add_frame(frame1, 1);
    assert_eq!(delivered.len(), 2); // Should deliver frame 1 and buffered frame 2

    let (buffer_size, next_seq, _) = buffer.get_stats();
    assert_eq!(buffer_size, 0);
    assert_eq!(next_seq, 3);
}

#[test]
fn test_anti_replay_window_standalone() {
    let mut window = AntiReplayWindow::new(1024);

    // Test normal sequence
    assert!(window.check_nonce(100));
    assert!(window.check_nonce(101));
    assert!(window.check_nonce(102));

    // Test duplicate detection
    assert!(!window.check_nonce(101));

    // Test out-of-order within window
    assert!(window.check_nonce(99));

    // Test very old nonce (outside window)
    window.check_nonce(2000); // Update highest nonce
    assert!(!window.check_nonce(900)); // Too old

    let (window_size, highest) = window.get_stats();
    assert!(window_size > 0);
    assert_eq!(highest, 2000);
}

#[tokio::test]
async fn test_path_failover() {
    let config = MultipathConfig {
        min_path_quality: 0.5,
        failover_timeout_ms: 100,
        ..Default::default()
    };
    let data_plane = MultipathDataPlane::new(config);

    // Add primary path
    let primary_path = create_test_path_info(1, 123, 0.9);
    assert!(data_plane.add_path(primary_path).await.is_ok());

    // Add backup path
    let backup_path = create_test_path_info(2, 123, 0.8);
    assert!(data_plane.add_path(backup_path).await.is_ok());

    // Primary should be selected initially
    let selected = data_plane.select_send_path().await;
    assert_eq!(selected, Some(1)); // Path 1 has better RTT

    // Fail primary path
    let failed_metrics = PathMetrics {
        rtt_ms: 1000.0,
        quality: 0.1, // Very poor quality
        loss_rate: 0.9,
        ..Default::default()
    };

    assert!(data_plane
        .update_path_metrics(1, failed_metrics)
        .await
        .is_ok());

    // Should failover to backup path
    let selected = data_plane.select_send_path().await;
    assert_eq!(selected, Some(2));
}

#[tokio::test]
async fn test_dynamic_buffer_timeout_adjustment() {
    let config = MultipathConfig::default();
    let data_plane = MultipathDataPlane::new(config);

    // Add paths with different RTTs
    let fast_path = PathMetrics {
        rtt_ms: 50.0,
        jitter_ms: 5.0,
        quality: 0.9,
        ..Default::default()
    };

    let slow_path = PathMetrics {
        rtt_ms: 200.0,
        jitter_ms: 20.0,
        quality: 0.8,
        ..Default::default()
    };

    // Add paths
    let path1 = create_test_path_info(1, 123, 0.9);
    let path2 = create_test_path_info(2, 123, 0.8);
    assert!(data_plane.add_path(path1).await.is_ok());
    assert!(data_plane.add_path(path2).await.is_ok());

    // Update metrics
    assert!(data_plane.update_path_metrics(1, fast_path).await.is_ok());
    assert!(data_plane.update_path_metrics(2, slow_path).await.is_ok());

    // Buffer timeout should be adjusted based on RTT difference + jitter
    // RTT diff = 200 - 50 = 150ms, avg jitter = 12.5ms
    // Expected timeout ≈ 150 + 12.5*2 = 175ms

    let frame = create_test_frame(1, 1, b"test".to_vec());
    let _ = data_plane
        .process_incoming_frame(123, frame, 1, 1, 1, false)
        .await;

    // Verify buffer timeout was adjusted (actual verification would require accessing internal state)
    // This is mainly testing that the update doesn't crash
}
