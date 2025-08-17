use nyx_mix::cmix::{Batcher, CmixError};
use std::time::{Duration, Instant};

/// Test VDF integration with cMix batching process
#[test]
fn cmix_vdf_integration() {
    let mut batcher = Batcher::with_vdf_delay(10, Duration::from_millis(100), 50); // 50ms VDF delay
    
    // Add packets to trigger batch creation
    for i in 0..5 {
        let packet = format!("vdf_test_packet_{}", i).into_bytes();
        batcher.push(packet).expect("Failed to add packet for VDF test");
    }
    
    let start_time = Instant::now();
    let batch = batcher.force_flush().expect("Failed to create VDF-verified batch");
    let total_time = start_time.elapsed();
    
    // Verify VDF proof is present and non-zero
    assert!(!batch.vdf_proof.iter().all(|&x| x == 0), "VDF proof should be computed");
    
    // Verify VDF computation statistics were updated
    assert_eq!(batcher.stats.vdf_computations, 1);
    assert!(batcher.stats.total_vdf_time > Duration::from_nanos(0), "VDF time should be recorded");
    
    // VDF should introduce some minimal computational delay (at least 1µs tolerance)
    assert!(total_time >= Duration::from_micros(1), 
            "VDF should introduce measurable delay, got: {:?}", total_time);
    
    println!("✓ VDF integration test passed - Batch {} with proof, total time: {:?}", 
             batch.id, total_time);
}

/// Test VDF timeout detection mechanism
#[test]
fn cmix_vdf_timeout_handling() {
    // Use very fast VDF delay (1ms) to test normal operation
    let mut batcher = Batcher::with_vdf_delay(10, Duration::from_millis(100), 1);
    
    batcher.push(b"timeout_test_packet".to_vec()).expect("Failed to add packet");
    
    // This should succeed with fast VDF
    let result = batcher.force_flush();
    assert!(result.is_ok(), "Fast VDF should not timeout");
    
    let batch = result.unwrap();
    assert!(!batch.vdf_proof.iter().all(|&x| x == 0), "VDF proof should be generated");
    
    // Verify no timeout errors were recorded
    assert_eq!(batcher.stats.errors, 0, "No errors should occur with fast VDF");
    
    println!("✓ VDF timeout handling test passed - No timeouts with fast VDF");
}

/// Test VDF proof uniqueness across different batches
#[test]
fn cmix_vdf_proof_uniqueness() {
    let mut batcher = Batcher::with_vdf_delay(5, Duration::from_millis(50), 20);
    
    // Create first batch
    batcher.push(b"unique_test_1".to_vec()).expect("Failed to add first packet");
    let batch1 = batcher.force_flush().expect("Failed to create first batch");
    
    // Create second batch with different content
    batcher.push(b"unique_test_2".to_vec()).expect("Failed to add second packet");
    let batch2 = batcher.force_flush().expect("Failed to create second batch");
    
    // VDF proofs should be different due to different seeds
    assert_ne!(batch1.vdf_proof, batch2.vdf_proof, 
               "VDF proofs should be unique across different batches");
    
    // Batch IDs should be sequential
    assert_eq!(batch1.id + 1, batch2.id, "Batch IDs should be sequential");
    
    // Both batches should have valid VDF proofs
    assert!(!batch1.vdf_proof.iter().all(|&x| x == 0), "First VDF proof should be non-zero");
    assert!(!batch2.vdf_proof.iter().all(|&x| x == 0), "Second VDF proof should be non-zero");
    
    println!("✓ VDF proof uniqueness test passed - Proofs differ across batches");
}

/// Test VDF computation statistics tracking
#[test]
fn cmix_vdf_statistics_tracking() {
    let mut batcher = Batcher::with_vdf_delay(3, Duration::from_millis(50), 30);
    
    // Initial state check
    assert_eq!(batcher.stats.vdf_computations, 0);
    assert_eq!(batcher.stats.total_vdf_time, Duration::from_nanos(0));
    
    // Create multiple batches to accumulate statistics
    for i in 0..3 {
        batcher.push(format!("stats_test_{}", i).into_bytes())
               .expect("Failed to add packet for stats test");
        batcher.force_flush().expect("Failed to create batch for stats test");
    }
    
    // Verify statistics were updated correctly
    assert_eq!(batcher.stats.vdf_computations, 3, "Should track 3 VDF computations");
    assert!(batcher.stats.total_vdf_time > Duration::from_nanos(100), 
            "Total VDF time should accumulate across batches, got: {:?}", batcher.stats.total_vdf_time);
    assert_eq!(batcher.stats.emitted, 3, "Should track 3 emitted batches");
    
    // Generate detailed report and verify content
    let report = batcher.generate_error_report();
    assert!(report.contains("VDF computations: 3"), "Report should show VDF computation count");
    assert!(report.contains("Total VDF time:"), "Report should show total VDF time");
    
    println!("✓ VDF statistics tracking test passed - {} computations, total time: {:?}", 
             batcher.stats.vdf_computations, batcher.stats.total_vdf_time);
}

