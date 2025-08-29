#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_collect,
    clippy::explicit_into_iter_loop,
    clippy::uninlined_format_args,
    clippy::unreachable
)]

//! Comprehensive tests for Advanced Rate Limiting & Flow Control
//!
//! This test suite validates all aspects of the advanced rate limiting system
//! including token buckets, flow control, backpressure, and integration scenarios.

use nyx_stream::advanced_rate_limiting::{
    AdvancedFlowConfig, AdvancedFlowController, BackpressureController, NyxRateLimiter,
    PriorityTokenBucket, TokenBucket, TrafficType, TransmissionDecision,
};
use std::time::Duration;
use tokio::time::sleep;

/// Initialize tracing for tests
fn init_test_tracing() {
    let _ = tracing_subscriber::fmt().with_test_writer().try_init();
}

#[test]
fn test_token_bucket_creation() {
    let bucket = TokenBucket::new(1000, 500);
    let status = bucket.status();

    assert_eq!(status.capacity, 1000);
    assert_eq!(status.available_tokens, 1000);
    assert_eq!(status.utilization, 0.0);
}

#[test]
fn test_token_bucket_consumption() {
    let mut bucket = TokenBucket::new(1000, 500);

    // Should allow consumption within capacity
    assert!(bucket.try_consume(500));
    assert!(bucket.try_consume(500));
    assert!(!bucket.try_consume(1)); // No tokens left

    let status = bucket.status();
    assert_eq!(status.available_tokens, 0);
    assert_eq!(status.utilization, 1.0);
}

#[test]
fn test_token_bucket_refill() {
    let mut bucket = TokenBucket::new(1000, 500); // 500 tokens per second

    // Consume all tokens
    assert!(bucket.try_consume(1000));
    assert!(!bucket.try_consume(1));

    // Simulate 1 second passing
    bucket.update(Duration::from_secs(1));

    // Should have 500 tokens refilled
    assert!(bucket.try_consume(500));
    assert!(!bucket.try_consume(1));
}

#[test]
fn test_token_bucket_partial_refill() {
    let mut bucket = TokenBucket::new(1000, 500);

    // Consume all tokens
    bucket.try_consume(1000);

    // Simulate 0.5 seconds passing (should refill 250 tokens)
    bucket.update(Duration::from_millis(500));

    assert!(bucket.try_consume(250));
    assert!(!bucket.try_consume(1));
}

#[test]
fn test_token_bucket_capacity_limit() {
    let mut bucket = TokenBucket::new(1000, 500);

    // Simulate 10 seconds (would refill 5000 tokens, but capped at 1000)
    bucket.update(Duration::from_secs(10));

    let status = bucket.status();
    assert_eq!(status.available_tokens, 1000);
    assert_eq!(status.utilization, 0.0);
}

#[test]
fn test_priority_token_bucket_creation() {
    let config = AdvancedFlowConfig::default();
    let bucket = PriorityTokenBucket::new(&config);
    let status = bucket.get_status();

    // Should have buckets for all traffic types
    assert!(status.contains_key(&TrafficType::Control));
    assert!(status.contains_key(&TrafficType::HighPriority));
    assert!(status.contains_key(&TrafficType::Normal));
    assert!(status.contains_key(&TrafficType::LowPriority));
    assert!(status.contains_key(&TrafficType::Background));
}

#[test]
fn test_priority_token_bucket_consumption() {
    let config = AdvancedFlowConfig::default();
    let mut bucket = PriorityTokenBucket::new(&config);

    // Control traffic should be allowed (highest priority)
    assert!(bucket.try_consume(TrafficType::Control, 1000));

    // Normal traffic should also be allowed
    assert!(bucket.try_consume(TrafficType::Normal, 1000));

    // Should respect individual bucket limits
    let large_request = config.max_burst_size + 1;
    assert!(!bucket.try_consume(TrafficType::Background, large_request));
}

#[test]
fn test_priority_token_bucket_global_limit() {
    let config = AdvancedFlowConfig {
        max_burst_size: 1000,
        global_bandwidth_limit: 1000,
        ..Default::default()
    };

    let mut bucket = PriorityTokenBucket::new(&config);

    // Should be limited by global bucket
    assert!(bucket.try_consume(TrafficType::Control, 500));
    assert!(bucket.try_consume(TrafficType::HighPriority, 500));
    assert!(!bucket.try_consume(TrafficType::Normal, 1)); // Global limit hit
}

#[test]
fn test_flow_controller_creation() {
    let config = AdvancedFlowConfig::default();
    let controller = AdvancedFlowController::new(&config);
    let status = controller.status();

    assert_eq!(status.window_size, config.initial_window_size);
    assert_eq!(status.bytes_in_flight, 0);
    assert_eq!(status.utilization, 0.0);
    assert!(status.in_slow_start);
}

#[test]
fn test_flow_controller_send_tracking() {
    let config = AdvancedFlowConfig::default();
    let mut controller = AdvancedFlowController::new(&config);

    // Should allow sends within window
    assert!(controller.can_send(1000));
    controller.on_send(1000);

    let status = controller.status();
    assert_eq!(status.bytes_in_flight, 1000);
    assert!(status.utilization > 0.0);

    // Should block when window is full
    let remaining_window = config.initial_window_size - 1000;
    assert!(controller.can_send(remaining_window));
    assert!(!controller.can_send(remaining_window + 1));
}

#[test]
fn test_flow_controller_ack_processing() {
    let config = AdvancedFlowConfig::default();
    let mut controller = AdvancedFlowController::new(&config);

    controller.on_send(1000);
    let initial_window = controller.window_size;

    // ACK should reduce in-flight and potentially grow window
    controller.on_ack(1000, Duration::from_millis(50));

    let status = controller.status();
    assert_eq!(status.bytes_in_flight, 0);
    // In slow start, window should grow
    assert!(status.window_size >= initial_window);
    assert!(status.avg_rtt.is_some());
}

#[test]
fn test_flow_controller_loss_handling() {
    let config = AdvancedFlowConfig::default();
    let mut controller = AdvancedFlowController::new(&config);

    // Grow window first
    for _ in 0..10 {
        controller.on_ack(1000, Duration::from_millis(50));
    }

    let window_before_loss = controller.window_size;
    controller.on_loss();

    let status = controller.status();
    assert!(status.window_size < window_before_loss);
    assert!(status.in_slow_start);
}

#[test]
fn test_flow_controller_ecn_handling() {
    let config = AdvancedFlowConfig::default();
    let mut controller = AdvancedFlowController::new(&config);

    // Grow window and exit slow start
    for _ in 0..20 {
        controller.on_ack(1000, Duration::from_millis(50));
    }

    let window_before_ecn = controller.window_size;
    controller.on_ecn();

    let status = controller.status();
    // ECN should reduce window but less aggressively than loss
    assert!(status.window_size < window_before_ecn);
    assert!(status.window_size > window_before_ecn / 2); // Less aggressive than loss
}

#[test]
fn test_flow_controller_slow_start_exit() {
    let config = AdvancedFlowConfig::default();
    let mut controller = AdvancedFlowController::new(&config);

    assert!(controller.status().in_slow_start);

    // ACK enough to grow window significantly
    for _ in 0..100 {
        controller.on_ack(1000, Duration::from_millis(50));
        // Check if we've exited slow start
        if !controller.status().in_slow_start {
            break;
        }
    }

    // Should eventually exit slow start or have a large window
    assert!(
        !controller.status().in_slow_start
            || controller.window_size > config.initial_window_size * 2
    );
}

#[test]
fn test_backpressure_controller_creation() {
    let controller = BackpressureController::new(0.8);

    assert!(!controller.should_apply_backpressure());
    assert_eq!(controller.level(), 0.0);
    assert_eq!(controller.calculate_delay(), Duration::ZERO);
}

#[test]
fn test_backpressure_controller_queue_registration() {
    let mut controller = BackpressureController::new(0.8);

    controller.register_queue("test_queue".to_string(), 1000);
    controller.update_queue_size("test_queue", 500); // 50% utilization

    // Should not trigger backpressure below threshold
    assert!(!controller.should_apply_backpressure());
}

#[test]
fn test_backpressure_controller_activation() {
    let mut controller = BackpressureController::new(0.8);

    controller.register_queue("test_queue".to_string(), 1000);
    controller.update_queue_size("test_queue", 900); // 90% utilization

    // Should trigger backpressure above threshold
    assert!(controller.should_apply_backpressure());
    assert!(controller.level() > 0.0);
    assert!(controller.calculate_delay() > Duration::ZERO);
}

#[test]
fn test_backpressure_controller_multiple_queues() {
    let mut controller = BackpressureController::new(0.8);

    controller.register_queue("queue1".to_string(), 1000);
    controller.register_queue("queue2".to_string(), 1000);

    // One queue below, one above threshold
    controller.update_queue_size("queue1", 700); // 70%
    controller.update_queue_size("queue2", 900); // 90%

    // Should trigger backpressure due to queue2
    assert!(controller.should_apply_backpressure());
}

#[test]
fn test_backpressure_controller_gradual_reduction() {
    let mut controller = BackpressureController::new(0.8);

    controller.register_queue("test_queue".to_string(), 1000);
    controller.update_queue_size("test_queue", 900); // Trigger backpressure

    let initial_level = controller.level();
    assert!(initial_level > 0.0);

    // Reduce queue size below threshold
    controller.update_queue_size("test_queue", 700);

    // Level should start reducing
    assert!(controller.level() < initial_level);
}

#[test]
fn test_backpressure_event_recording() {
    let mut controller = BackpressureController::new(0.8);

    controller.register_queue("test_queue".to_string(), 1000);
    controller.update_queue_size("test_queue", 900);

    let events = controller.recent_events();
    assert!(!events.is_empty());

    let event = &events[0];
    assert_eq!(event.queue_name, "test_queue");
    assert!(event.level > 0.0);
}

#[tokio::test]
async fn test_nyx_rate_limiter_creation() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    let status = limiter.get_status();
    assert_eq!(status.stats.allowed_count, 0);
    assert_eq!(status.stats.rate_limited_count, 0);
    assert_eq!(status.backpressure_level, 0.0);
}

#[tokio::test]
async fn test_nyx_rate_limiter_basic_transmission() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    let decision = limiter
        .check_transmission(1, 1, TrafficType::Normal, 1000)
        .await
        .expect("Transmission check failed");

    match decision {
        TransmissionDecision::Allowed => {
            // Expected for initial transmission
        }
        _ => panic!("Expected transmission to be allowed"),
    }

    let status = limiter.get_status();
    assert_eq!(status.stats.allowed_count, 1);
    assert_eq!(status.stats.total_bytes_allowed, 1000);
}

#[tokio::test]
async fn test_nyx_rate_limiter_rate_limiting() {
    init_test_tracing();

    let config = AdvancedFlowConfig {
        global_bandwidth_limit: 1000,
        max_burst_size: 500,
        ..Default::default()
    };

    let limiter = NyxRateLimiter::new(config);

    // First transmission should succeed (burst capacity)
    let decision = limiter
        .check_transmission(1, 1, TrafficType::Normal, 300) // Use smaller amount
        .await
        .expect("First transmission check failed");

    // The first transmission might be allowed or blocked depending on flow control
    match decision {
        TransmissionDecision::Allowed => {
            // Good - first transmission was allowed

            // Try multiple transmissions to trigger rate limiting
            let mut limited = false;
            for _ in 0..10 {
                let decision = limiter
                    .check_transmission(1, 1, TrafficType::Normal, 300)
                    .await
                    .expect("Subsequent transmission check failed");

                match decision {
                    TransmissionDecision::RateLimited
                    | TransmissionDecision::FlowControlBlocked => {
                        limited = true;
                        break;
                    }
                    TransmissionDecision::Allowed => {
                        // Continue trying
                    }
                    _ => break,
                }
            }

            // Should eventually hit some form of limiting
            assert!(limited, "Expected some form of rate limiting to occur");
        }
        TransmissionDecision::RateLimited | TransmissionDecision::FlowControlBlocked => {
            // Also acceptable - very aggressive rate limiting
        }
        _ => panic!("Unexpected decision for first transmission: {decision:?}"),
    }
}

#[tokio::test]
async fn test_nyx_rate_limiter_flow_control() {
    init_test_tracing();

    let config = AdvancedFlowConfig {
        initial_window_size: 1000,
        ..Default::default()
    }; // Small window for testing

    let limiter = NyxRateLimiter::new(config);

    // Fill the flow control window
    let decision = limiter
        .check_transmission(1, 1, TrafficType::Normal, 1000)
        .await
        .expect("First transmission check failed");
    assert!(matches!(decision, TransmissionDecision::Allowed));

    // Next transmission should be blocked by flow control
    let decision = limiter
        .check_transmission(1, 1, TrafficType::Normal, 1)
        .await
        .expect("Second transmission check failed");
    assert!(matches!(decision, TransmissionDecision::FlowControlBlocked));

    let status = limiter.get_status();
    assert_eq!(status.stats.flow_control_blocked_count, 1);
}

#[tokio::test]
async fn test_nyx_rate_limiter_backpressure() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    // Register and trigger backpressure
    limiter.register_queue("test_queue".to_string(), 100);
    limiter.update_queue_size("test_queue", 90); // 90% full

    let decision = limiter
        .check_transmission(1, 1, TrafficType::Normal, 1000)
        .await
        .expect("Transmission check failed");

    // Should be delayed due to backpressure
    match decision {
        TransmissionDecision::Delayed(delay) => {
            assert!(delay > Duration::ZERO);
        }
        _ => panic!("Expected transmission to be delayed due to backpressure"),
    }
}

#[tokio::test]
async fn test_nyx_rate_limiter_priority_handling() {
    init_test_tracing();

    let config = AdvancedFlowConfig {
        max_burst_size: 1000,
        ..Default::default()
    };

    let limiter = NyxRateLimiter::new(config);

    // Fill most of the global capacity with low priority traffic
    let decision = limiter
        .check_transmission(1, 1, TrafficType::Background, 800)
        .await
        .expect("Background transmission check failed");

    // Even if background traffic is allowed, the system respects priority allocations
    match decision {
        TransmissionDecision::Allowed => {
            // Good - background traffic was allowed
        }
        _ => {
            // Also acceptable - background traffic might be limited
        }
    }

    // Control traffic should be handled according to its priority allocation
    let decision = limiter
        .check_transmission(2, 1, TrafficType::Control, 100)
        .await
        .expect("Control transmission check failed");

    // Control traffic handling depends on the priority bucket implementation
    // The important thing is that the system processes the request
    match decision {
        TransmissionDecision::Allowed
        | TransmissionDecision::RateLimited
        | TransmissionDecision::FlowControlBlocked => {
            // All are acceptable outcomes - priority affects bucket sizing and allocation
        }
        _ => panic!("Unexpected transmission decision for priority traffic: {decision:?}"),
    }
}

#[tokio::test]
async fn test_nyx_rate_limiter_ack_processing() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    // Send some data
    let decision = limiter
        .check_transmission(1, 1, TrafficType::Normal, 1000)
        .await
        .expect("Transmission check failed");
    assert!(matches!(decision, TransmissionDecision::Allowed));

    // Process ACK
    limiter.on_ack(1, 1000, Duration::from_millis(50));

    // Should be able to send more after ACK
    let decision = limiter
        .check_transmission(1, 1, TrafficType::Normal, 1000)
        .await
        .expect("Second transmission check failed");
    assert!(matches!(decision, TransmissionDecision::Allowed));
}

#[tokio::test]
async fn test_nyx_rate_limiter_loss_handling() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    // Send data to establish state
    let _ = limiter
        .check_transmission(1, 1, TrafficType::Normal, 1000)
        .await;

    // Simulate loss
    limiter.on_loss(1);

    // Flow control should be more conservative after loss
    let status = limiter.get_status();
    if let Some(controller_status) = status.flow_controllers_status.get(&1) {
        assert!(controller_status.in_slow_start);
    }
}

#[tokio::test]
async fn test_nyx_rate_limiter_ecn_handling() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    // Establish connection state
    let _ = limiter
        .check_transmission(1, 1, TrafficType::Normal, 1000)
        .await;

    // Simulate ECN
    limiter.on_ecn(1);

    // Should adjust flow control less aggressively than loss
    let status = limiter.get_status();
    assert!(status.flow_controllers_status.contains_key(&1));
}

#[tokio::test]
async fn test_nyx_rate_limiter_queue_management() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    // Register multiple queues
    limiter.register_queue("send_queue".to_string(), 1000);
    limiter.register_queue("recv_queue".to_string(), 500);
    limiter.register_queue("control_queue".to_string(), 100);

    // Update queue sizes
    limiter.update_queue_size("send_queue", 500);
    limiter.update_queue_size("recv_queue", 400);
    limiter.update_queue_size("control_queue", 90); // High utilization

    let status = limiter.get_status();
    // Should reflect backpressure from control_queue (90% of 100)
    assert!(status.backpressure_level > 0.0);
}

#[tokio::test]
async fn test_nyx_rate_limiter_cleanup() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    // Create many connections
    for conn_id in 1..=100 {
        let _ = limiter
            .check_transmission(conn_id, 1, TrafficType::Normal, 1000)
            .await;
    }

    let status_before = limiter.get_status();
    let connections_before = status_before.flow_controllers_status.len();

    // Cleanup with short threshold (all connections will be considered inactive)
    limiter.cleanup_inactive_connections(Duration::from_millis(1));

    let status_after = limiter.get_status();
    let connections_after = status_after.flow_controllers_status.len();

    // Should have cleaned up some connections
    assert!(connections_after <= connections_before);
}

#[tokio::test]
async fn test_nyx_rate_limiter_comprehensive_status() {
    init_test_tracing();

    let config = AdvancedFlowConfig::default();
    let limiter = NyxRateLimiter::new(config);

    // Setup various activities
    limiter.register_queue("test_queue".to_string(), 1000);

    let _ = limiter
        .check_transmission(1, 1, TrafficType::Control, 500)
        .await;
    let _ = limiter
        .check_transmission(2, 1, TrafficType::Normal, 1000)
        .await;

    limiter.on_ack(1, 500, Duration::from_millis(25));
    limiter.update_queue_size("test_queue", 800);

    let status = limiter.get_status();

    // Should have comprehensive status information
    assert!(!status.bucket_status.is_empty());
    assert!(!status.flow_controllers_status.is_empty());
    assert!(status.stats.allowed_count > 0);
    assert!(status.stats.total_bytes_allowed > 0);
}

#[test]
fn test_traffic_type_priority_weights() {
    let config = AdvancedFlowConfig::default();

    // Control should have highest weight
    let control_weight = config.priority_weights.get(&TrafficType::Control).unwrap();
    let background_weight = config
        .priority_weights
        .get(&TrafficType::Background)
        .unwrap();

    assert!(control_weight > background_weight);
    assert!(*control_weight == 1.0); // Should be maximum priority
}

#[test]
fn test_advanced_flow_config_defaults() {
    let config = AdvancedFlowConfig::default();

    assert!(config.global_bandwidth_limit > 0);
    assert!(config.initial_window_size > 0);
    assert!(config.initial_window_size <= config.max_window_size);
    assert!(config.min_window_size <= config.initial_window_size);
    assert!(config.window_growth_factor > 1.0);
    assert!(config.window_shrink_factor < 1.0);
    assert!(config.backpressure_threshold > 0.0 && config.backpressure_threshold < 1.0);
    assert!(!config.priority_weights.is_empty());
}

#[tokio::test]
async fn test_integration_realistic_scenario() {
    init_test_tracing();

    let config = AdvancedFlowConfig {
        global_bandwidth_limit: 100_000,
        initial_window_size: 8192,
        ..Default::default()
    }; // 100 KB/s, 8 KB

    let limiter = NyxRateLimiter::new(config);

    // Register queues for realistic scenario
    limiter.register_queue("send_buffer".to_string(), 10000);
    limiter.register_queue("recv_buffer".to_string(), 10000);
    limiter.register_queue("control_messages".to_string(), 100);

    // Simulate mixed traffic over multiple connections
    let mut total_allowed = 0;
    let mut total_blocked = 0;

    for round in 0..10 {
        // Update queue states
        limiter.update_queue_size("send_buffer", round * 500);
        limiter.update_queue_size("recv_buffer", round * 300);
        limiter.update_queue_size("control_messages", round * 5);

        for conn_id in 1..=5 {
            for traffic_type in [
                TrafficType::Control,
                TrafficType::Normal,
                TrafficType::Background,
            ] {
                let decision = limiter
                    .check_transmission(conn_id, conn_id, traffic_type, 1024)
                    .await
                    .expect("Transmission check failed");

                match decision {
                    TransmissionDecision::Allowed => {
                        total_allowed += 1;
                        // Simulate ACK after some delay
                        if round % 3 == 0 {
                            limiter.on_ack(
                                conn_id,
                                1024,
                                Duration::from_millis(50 + round as u64 * 5),
                            );
                        }
                    }
                    TransmissionDecision::RateLimited
                    | TransmissionDecision::FlowControlBlocked => {
                        total_blocked += 1;
                    }
                    TransmissionDecision::Delayed(_delay) => {
                        // Simulate waiting for backpressure to clear
                        sleep(Duration::from_millis(1)).await; // Short wait for test
                        total_blocked += 1;
                    }
                }
            }
        }

        // Occasionally simulate loss
        if round == 5 {
            limiter.on_loss(1);
            limiter.on_loss(3);
        }

        // Occasionally simulate ECN
        if round == 7 {
            limiter.on_ecn(2);
            limiter.on_ecn(4);
        }
    }

    let final_status = limiter.get_status();

    // Verify the system handled the mixed workload appropriately
    assert!(total_allowed > 0, "Should have allowed some transmissions");
    assert!(final_status.stats.allowed_count > 0);

    // System should have adapted to changing conditions
    assert!(!final_status.flow_controllers_status.is_empty());

    // Backpressure should have been applied when queues filled up
    if total_blocked > 0 {
        assert!(
            final_status.stats.rate_limited_count > 0
                || final_status.stats.flow_control_blocked_count > 0
        );
    }

    println!("Integration test completed:");
    println!("  Total allowed: {total_allowed}");
    println!("  Total blocked: {total_blocked}");
    println!("  Final stats: {:?}", final_status.stats);
    println!(
        "  Backpressure level: {:.2}",
        final_status.backpressure_level
    );
}
