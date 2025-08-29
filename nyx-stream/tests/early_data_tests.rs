#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_collect,
    clippy::explicit_into_iter_loop,
    clippy::uninlined_format_args,
    clippy::unreachable
)]

//! Comprehensive test suite for Early Data and 0-RTT Reception implementation
//!
//! This test suite covers all aspects of the Early-Data and 0-RTT Reception
//! requirements from Nyx Protocol v1.0 specification.

use nyx_stream::early_data::*;
use nyx_stream::errors::Result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[test]
fn test_direction_id_basic_operations() {
    let client_dir = DirectionId::CLIENT_TO_SERVER;
    let server_dir = DirectionId::SERVER_TO_CLIENT;

    // Test basic properties
    assert_eq!(client_dir.value(), 1);
    assert_eq!(server_dir.value(), 2);
    assert_ne!(client_dir, server_dir);

    // Test bidirectional mapping
    assert_eq!(client_dir.opposite(), server_dir);
    assert_eq!(server_dir.opposite(), client_dir);
    assert_eq!(client_dir.opposite().opposite(), client_dir);

    // Test display formatting
    assert_eq!(format!("{client_dir}"), "C→S");
    assert_eq!(format!("{server_dir}"), "S→C");
}

#[test]
fn test_direction_id_custom_values() {
    let custom1 = DirectionId::new(0x11111111);
    let custom2 = DirectionId::new(0x22222222);

    assert_eq!(custom1.value(), 0x11111111);
    assert_eq!(custom2.value(), 0x22222222);
    assert_ne!(custom1, custom2);

    // Test custom opposite calculation
    assert_eq!(custom1.opposite().value(), !0x11111111);
    assert_eq!(custom2.opposite().value(), !0x22222222);

    // Test display for custom values
    assert_eq!(format!("{custom1}"), "Dir(11111111)");
}

#[test]
fn test_nonce_basic_operations() {
    let mut nonce = Nonce::new(0);
    assert_eq!(nonce.value(), 0);

    // Test increment
    nonce.increment();
    assert_eq!(nonce.value(), 1);

    nonce.increment();
    assert_eq!(nonce.value(), 2);

    // Test wrapping increment at maximum
    let mut max_nonce = Nonce::new(u64::MAX);
    max_nonce.increment();
    assert_eq!(max_nonce.value(), 0); // Should wrap around
}

#[test]
fn test_nonce_aead_construction() {
    let nonce = Nonce::new(0x123456789ABCDEF0);
    let direction = DirectionId::CLIENT_TO_SERVER;

    let aead_nonce = nonce.to_aead_nonce(direction);

    // Verify structure: first 4 bytes = direction, next 8 bytes = nonce
    assert_eq!(aead_nonce.len(), 12);
    assert_eq!(&aead_nonce[0..4], &direction.value().to_be_bytes());
    assert_eq!(&aead_nonce[4..12], &nonce.value().to_be_bytes());

    // Test different direction produces different nonce
    let server_aead = nonce.to_aead_nonce(DirectionId::SERVER_TO_CLIENT);
    assert_ne!(aead_nonce, server_aead);
    assert_eq!(&server_aead[4..12], &nonce.value().to_be_bytes()); // Nonce part same
    assert_ne!(&aead_nonce[0..4], &server_aead[0..4]); // Direction part different
}

#[test]
fn test_anti_replay_window_basic() {
    let direction = DirectionId::CLIENT_TO_SERVER;
    let mut window = AntiReplayWindow::new(direction);

    // Test accepting first nonce
    assert!(window.check_and_update(Nonce::new(1)));

    // Test rejecting replay
    assert!(!window.check_and_update(Nonce::new(1)));

    // Test accepting new nonce
    assert!(window.check_and_update(Nonce::new(2)));
    assert!(window.check_and_update(Nonce::new(3)));
}

#[test]
fn test_anti_replay_window_ordering() {
    let direction = DirectionId::CLIENT_TO_SERVER;
    let mut window = AntiReplayWindow::new(direction);

    // Accept nonces out of order within window
    assert!(window.check_and_update(Nonce::new(10)));
    assert!(window.check_and_update(Nonce::new(5))); // Earlier nonce, should work
    assert!(window.check_and_update(Nonce::new(15))); // Later nonce, should work
    assert!(window.check_and_update(Nonce::new(7))); // Middle nonce, should work

    // Test replay detection for out-of-order nonces
    assert!(!window.check_and_update(Nonce::new(5))); // Replay
    assert!(!window.check_and_update(Nonce::new(10))); // Replay
}

#[test]
fn test_anti_replay_window_size_limits() {
    let direction = DirectionId::CLIENT_TO_SERVER;
    let window_size = 1000; // Smaller window for testing
    let mut window = AntiReplayWindow::with_size(direction, window_size);

    // Fill window with sequential nonces
    for i in 1..=window_size {
        assert!(window.check_and_update(Nonce::new(i)));
    }

    // Nonce 0 should be rejected (special case after higher nonces processed)
    assert!(!window.check_and_update(Nonce::new(0)));

    // Nonce too far in future should be rejected
    assert!(!window.check_and_update(Nonce::new(window_size + window_size + 1)));

    // Nonce just at edge should work
    assert!(window.check_and_update(Nonce::new(window_size + 1)));
}

#[test]
fn test_anti_replay_window_statistics() {
    let direction = DirectionId::CLIENT_TO_SERVER;
    let mut window = AntiReplayWindow::new(direction);

    // Process some valid nonces
    assert!(window.check_and_update(Nonce::new(1)));
    assert!(window.check_and_update(Nonce::new(2)));
    assert!(window.check_and_update(Nonce::new(3)));

    // Process some replays
    assert!(!window.check_and_update(Nonce::new(1)));
    assert!(!window.check_and_update(Nonce::new(2)));

    let stats = window.stats();
    assert_eq!(stats.direction_id, direction);
    assert_eq!(stats.total_processed, 5);
    assert_eq!(stats.replay_blocks, 2);
    assert_eq!(stats.seen_count, 3);
    assert_eq!(stats.window_base, 3);
}

#[test]
fn test_anti_replay_window_reset() {
    let direction = DirectionId::CLIENT_TO_SERVER;
    let mut window = AntiReplayWindow::new(direction);

    // Add some nonces
    assert!(window.check_and_update(Nonce::new(5)));
    assert!(window.check_and_update(Nonce::new(10)));

    let stats_before = window.stats();
    assert_eq!(stats_before.seen_count, 2);
    assert_eq!(stats_before.window_base, 10);

    // Reset window
    window.reset();

    let stats_after = window.stats();
    assert_eq!(stats_after.seen_count, 0);
    assert_eq!(stats_after.window_base, 0);
    assert!(stats_after.last_reset.is_some());

    // Should be able to reuse nonces after reset
    assert!(window.check_and_update(Nonce::new(5)));
    assert!(window.check_and_update(Nonce::new(10)));
}

#[test]
fn test_early_data_manager_lifecycle() -> Result<()> {
    let mut manager = EarlyDataManager::new();

    // Initial state should be disabled
    assert_eq!(manager.state(), EarlyDataState::Disabled);

    // Enable early data
    manager.enable_early_data()?;
    assert_eq!(manager.state(), EarlyDataState::Enabled);

    // Complete handshake
    manager.complete_handshake();
    assert_eq!(manager.state(), EarlyDataState::Completed);

    // Check metrics reflect state changes
    let metrics = manager.metrics()?;
    assert_eq!(metrics.early_data_enabled_count, 1);
    assert_eq!(metrics.handshake_completed_count, 1);
    assert!(metrics.session_duration.is_some());

    Ok(())
}

#[test]
fn test_early_data_validation_basic() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    let direction = DirectionId::CLIENT_TO_SERVER;
    let test_data = b"Hello, early data world!";

    // Should reject when disabled
    let result = manager.validate_early_data(direction, Nonce::new(1), test_data)?;
    assert!(!result);

    // Enable and test acceptance
    manager.enable_early_data()?;
    let result = manager.validate_early_data(direction, Nonce::new(1), test_data)?;
    assert!(result);

    // Test replay rejection
    let result = manager.validate_early_data(direction, Nonce::new(1), test_data)?;
    assert!(!result);

    // Test different nonce acceptance
    let result = manager.validate_early_data(direction, Nonce::new(2), test_data)?;
    assert!(result);

    Ok(())
}

#[test]
fn test_early_data_size_limits() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    manager.enable_early_data()?;

    let direction = DirectionId::CLIENT_TO_SERVER;

    // Test maximum packet size rejection
    let oversized_data = vec![0u8; MAX_EARLY_DATA_SIZE + 1];
    let result = manager.validate_early_data(direction, Nonce::new(1), &oversized_data);
    assert!(result.is_err()); // Should return error for oversized packet

    // Test acceptable size
    let acceptable_data = vec![0u8; MAX_EARLY_DATA_SIZE];
    let result = manager.validate_early_data(direction, Nonce::new(1), &acceptable_data)?;
    assert!(result);

    Ok(())
}

#[test]
fn test_early_data_total_session_limits() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    manager.enable_early_data()?;

    let direction = DirectionId::CLIENT_TO_SERVER;
    let data_chunk = vec![0u8; 64 * 1024]; // 64KB chunks

    // Fill up to just under the limit
    // MAX_TOTAL_EARLY_DATA = 1MB = 1024*1024 = 16 * 64KB
    let max_chunks = (MAX_TOTAL_EARLY_DATA / (64 * 1024)) - 1; // 15 chunks = 960KB

    for i in 0..max_chunks {
        let result =
            manager.validate_early_data(direction, Nonce::new(i as u64 + 1), &data_chunk)?;
        assert!(result, "Chunk {i} should be accepted");
    }

    // Add one more chunk that fits within limit (total = 1MB exactly)
    let result =
        manager.validate_early_data(direction, Nonce::new(max_chunks as u64 + 1), &data_chunk)?;
    assert!(
        result,
        "Final chunk bringing total to exactly 1MB should be accepted"
    );

    // This chunk should exceed the total limit
    let result =
        manager.validate_early_data(direction, Nonce::new(max_chunks as u64 + 2), &data_chunk)?;
    assert!(!result); // Should be rejected due to total limit

    Ok(())
}

#[test]
fn test_early_data_security_disable() -> Result<()> {
    let mut manager = EarlyDataManager::new();

    // Enable early data first
    manager.enable_early_data()?;
    assert_eq!(manager.state(), EarlyDataState::Enabled);

    // Disable for security reasons
    let security_reason = "Suspicious replay pattern detected";
    manager.disable_for_security(security_reason.to_string());
    assert_eq!(manager.state(), EarlyDataState::SecurityDisabled);

    // Try to enable again - should fail
    let result = manager.enable_early_data();
    assert!(result.is_err());

    // Verify metrics
    let metrics = manager.metrics()?;
    assert_eq!(metrics.security_disable_count, 1);
    assert!(metrics.security_disable_reasons.contains(security_reason));

    Ok(())
}

#[test]
fn test_early_data_rekey_operation() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    manager.enable_early_data()?;

    let direction = DirectionId::CLIENT_TO_SERVER;
    let test_data = b"test data";

    // Accept some early data
    assert!(manager.validate_early_data(direction, Nonce::new(1), test_data)?);
    assert!(manager.validate_early_data(direction, Nonce::new(2), test_data)?);

    // Perform rekey
    manager.reset_for_rekey()?;

    // Should be able to reuse nonces after rekey
    assert!(manager.validate_early_data(direction, Nonce::new(1), test_data)?);
    assert!(manager.validate_early_data(direction, Nonce::new(2), test_data)?);

    // Verify rekey metrics
    let metrics = manager.metrics()?;
    assert_eq!(metrics.rekey_count, 1);
    assert!(metrics.last_rekey_timestamp.is_some());

    Ok(())
}

#[test]
fn test_early_data_bidirectional() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    manager.enable_early_data()?;

    let client_dir = DirectionId::CLIENT_TO_SERVER;
    let server_dir = DirectionId::SERVER_TO_CLIENT;
    let test_data = b"bidirectional test";

    // Both directions should work independently
    assert!(manager.validate_early_data(client_dir, Nonce::new(1), test_data)?);
    assert!(manager.validate_early_data(server_dir, Nonce::new(1), test_data)?);

    // Replay should be detected per direction
    assert!(!manager.validate_early_data(client_dir, Nonce::new(1), test_data)?);
    assert!(!manager.validate_early_data(server_dir, Nonce::new(1), test_data)?);

    // New nonces should work in both directions
    assert!(manager.validate_early_data(client_dir, Nonce::new(2), test_data)?);
    assert!(manager.validate_early_data(server_dir, Nonce::new(2), test_data)?);

    Ok(())
}

#[test]
fn test_nonce_constructor_utilities() -> Result<()> {
    let direction = DirectionId::SERVER_TO_CLIENT;
    let nonce = Nonce::new(0x123456789ABCDEF0);

    // Test AEAD nonce construction
    let aead_nonce = NonceConstructor::construct_aead_nonce(direction, nonce);
    assert_eq!(aead_nonce.len(), 12);
    assert_eq!(&aead_nonce[0..4], &direction.value().to_be_bytes());
    assert_eq!(&aead_nonce[4..12], &nonce.value().to_be_bytes());

    // Test nonce validation
    assert!(NonceConstructor::validate_nonce(nonce).is_ok());
    assert!(NonceConstructor::validate_nonce(Nonce::new(u64::MAX)).is_err());

    // Test initial nonce
    let initial = NonceConstructor::initial_nonce();
    assert_eq!(initial.value(), 0);

    // Test next nonce generation
    let next = NonceConstructor::next_nonce(nonce)?;
    assert_eq!(next.value(), nonce.value().wrapping_add(1));

    // Test overflow protection
    let max_nonce = Nonce::new(u64::MAX);
    let overflow_result = NonceConstructor::next_nonce(max_nonce);
    assert!(overflow_result.is_err());

    Ok(())
}

#[test]
fn test_early_data_metrics_collection() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    let direction = DirectionId::CLIENT_TO_SERVER;
    let test_data = b"metrics test data";

    // Initially no metrics
    let initial_metrics = manager.metrics()?;
    assert_eq!(initial_metrics.early_data_accepted_count, 0);
    assert_eq!(initial_metrics.early_data_rejected_count, 0);

    // Reject while disabled
    assert!(!manager.validate_early_data(direction, Nonce::new(1), test_data)?);

    let rejected_metrics = manager.metrics()?;
    assert_eq!(rejected_metrics.early_data_rejected_count, 1);
    assert!(rejected_metrics
        .rejection_reasons
        .contains("state_not_enabled"));

    // Enable and accept
    manager.enable_early_data()?;
    assert!(manager.validate_early_data(direction, Nonce::new(1), test_data)?);

    let accepted_metrics = manager.metrics()?;
    assert_eq!(accepted_metrics.early_data_accepted_count, 1);
    assert_eq!(accepted_metrics.total_early_data_bytes, test_data.len());
    assert!(accepted_metrics.last_early_data_timestamp.is_some());

    // Test replay metrics
    assert!(!manager.validate_early_data(direction, Nonce::new(1), test_data)?);

    let replay_metrics = manager.metrics()?;
    assert_eq!(replay_metrics.replay_drops, 1);
    assert_eq!(replay_metrics.early_data_rejected_count, 2);
    assert!(replay_metrics
        .rejection_reasons
        .contains("replay_protection"));

    Ok(())
}

#[test]
fn test_session_statistics() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    let direction = DirectionId::CLIENT_TO_SERVER;
    let test_data = b"session stats test";

    // Check initial session stats
    let initial_stats = manager.session_stats();
    assert_eq!(initial_stats.state, EarlyDataState::Disabled);
    assert_eq!(initial_stats.total_early_data_received, 0);
    assert_eq!(initial_stats.max_early_data_size, MAX_EARLY_DATA_SIZE);
    assert_eq!(initial_stats.max_total_early_data, MAX_TOTAL_EARLY_DATA);

    // Enable and process some data
    manager.enable_early_data()?;
    assert!(manager.validate_early_data(direction, Nonce::new(1), test_data)?);
    assert!(manager.validate_early_data(direction, Nonce::new(2), test_data)?);

    let updated_stats = manager.session_stats();
    assert_eq!(updated_stats.state, EarlyDataState::Enabled);
    assert_eq!(updated_stats.total_early_data_received, test_data.len() * 2);
    assert!(updated_stats.session_uptime > Duration::from_nanos(0));

    Ok(())
}

#[test]
fn test_window_statistics_collection() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    manager.enable_early_data()?;

    let client_dir = DirectionId::CLIENT_TO_SERVER;
    let server_dir = DirectionId::SERVER_TO_CLIENT;
    let test_data = b"window stats test";

    // Process data in both directions
    assert!(manager.validate_early_data(client_dir, Nonce::new(1), test_data)?);
    assert!(manager.validate_early_data(client_dir, Nonce::new(2), test_data)?);
    assert!(manager.validate_early_data(server_dir, Nonce::new(1), test_data)?);

    // Generate replay attempts
    assert!(!manager.validate_early_data(client_dir, Nonce::new(1), test_data)?);
    assert!(!manager.validate_early_data(server_dir, Nonce::new(1), test_data)?);

    let window_stats = manager.window_stats()?;

    // Check client direction stats
    let client_stats = window_stats.get(&client_dir).unwrap();
    assert_eq!(client_stats.direction_id, client_dir);
    assert!(client_stats.total_processed >= 3); // 2 valid + 1 replay
    assert!(client_stats.replay_blocks >= 1);
    assert_eq!(client_stats.seen_count, 2);

    // Check server direction stats
    let server_stats = window_stats.get(&server_dir).unwrap();
    assert_eq!(server_stats.direction_id, server_dir);
    assert!(server_stats.total_processed >= 2); // 1 valid + 1 replay
    assert!(server_stats.replay_blocks >= 1);
    assert_eq!(server_stats.seen_count, 1);

    Ok(())
}

#[test]
fn test_concurrent_access() -> Result<()> {
    let manager = Arc::new(Mutex::new(EarlyDataManager::new()));
    let direction = DirectionId::CLIENT_TO_SERVER;
    let test_data = b"concurrent test";

    // Enable early data
    {
        let mut mgr = manager.lock().unwrap();
        mgr.enable_early_data()?;
    }

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let mgr_clone = Arc::clone(&manager);
            let data = *test_data;
            thread::spawn(move || {
                let mut mgr = mgr_clone.lock().unwrap();
                mgr.validate_early_data(direction, Nonce::new(i + 1), &data)
            })
        })
        .collect();

    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(true)) = handle.join() {
            success_count += 1;
        }
    }

    // All unique nonces should be accepted
    assert_eq!(success_count, 10);

    // Verify final metrics
    let final_metrics = {
        let mgr = manager.lock().unwrap();
        mgr.metrics()?
    };
    assert_eq!(final_metrics.early_data_accepted_count, 10);

    Ok(())
}

#[test]
fn test_edge_case_window_boundaries() {
    let direction = DirectionId::CLIENT_TO_SERVER;
    let window_size = 100;
    let mut window = AntiReplayWindow::with_size(direction, window_size);

    // Test exact window boundary
    assert!(window.check_and_update(Nonce::new(window_size)));

    // Test nonce 0 after higher nonces (should be rejected)
    assert!(!window.check_and_update(Nonce::new(0)));

    // Test advancing window
    assert!(window.check_and_update(Nonce::new(window_size + 1)));

    // Now nonce 1 should be valid (within new window)
    assert!(window.check_and_update(Nonce::new(1)));

    // But nonce 0 should still be invalid (outside window)
    assert!(!window.check_and_update(Nonce::new(0)));
}

#[test]
fn test_comprehensive_rejection_reasons() -> Result<()> {
    let mut manager = EarlyDataManager::new();
    let direction = DirectionId::CLIENT_TO_SERVER;
    let test_data = b"rejection test";

    // Test state rejection
    assert!(!manager.validate_early_data(direction, Nonce::new(1), test_data)?);

    // Test oversized packet rejection
    manager.enable_early_data()?;
    let oversized_data = vec![0u8; MAX_EARLY_DATA_SIZE + 1];
    let result = manager.validate_early_data(direction, Nonce::new(1), &oversized_data);
    assert!(result.is_err());

    // Test replay rejection
    assert!(manager.validate_early_data(direction, Nonce::new(1), test_data)?);
    assert!(!manager.validate_early_data(direction, Nonce::new(1), test_data)?);

    // Check all rejection reasons are captured
    let metrics = manager.metrics()?;
    let expected_reasons = ["state_not_enabled", "replay_protection"];

    for reason in &expected_reasons {
        assert!(
            metrics.rejection_reasons.contains(*reason),
            "Missing rejection reason: {reason}",
        );
    }

    Ok(())
}
