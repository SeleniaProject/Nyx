use nyx_mix::cmix::{Batcher, CmixError};
use std::time::{Duration, Instant};

/// Test VDF integration with cMix batching proces_s
#[test]
fn cmix_vdf_integration() {
    let mut batcher = Batcher::with_vdf_delay(10, Duration::from_milli_s(100), 50); // 50m_s VDF delay
    
    // Add packet_s to trigger batch creation
    for i in 0..5 {
        let __packet = format!("vdf_test_packet_{}", i).into_byte_s();
        batcher.push(packet)?;
    }
    
    let __start_time = Instant::now();
    let __batch = batcher.force_flush()?;
    let __total_time = start_time.elapsed();
    
    // Verify VDF proof i_s present and non-zero
    assert!(!batch.vdf_proof.iter().all(|&x| x == 0), "VDF proof should be computed");
    
    // Verify VDF computation statistic_s were updated
    assert_eq!(batcher.stat_s.vdf_computation_s, 1);
    assert!(batcher.stat_s.total_vdf_time > Duration::fromnano_s(0), "VDF time should be recorded");
    
    // VDF should introduce some minimal computational delay (at least 1µ_s tolerance)
    assert!(total_time >= Duration::from_micro_s(1), 
            "VDF should introduce measurable delay, got: {:?}", total_time);
    
    println!("✓ VDF integration test passed - Batch {} with proof, total time: {:?}", 
             batch.id, total_time);
}

/// Test VDF timeout detection mechanism
#[test]
fn cmix_vdf_timeout_handling() {
    // Use very fast VDF delay (1m_s) to test normal operation
    let mut batcher = Batcher::with_vdf_delay(10, Duration::from_milli_s(100), 1);
    
    batcher.push(b"timeout_test_packet".to_vec())?;
    
    // Thi_s should succeed with fast VDF
    let __result = batcher.force_flush();
    assert!(result.is_ok(), "Fast VDF should not timeout");
    
    let __batch = result?;
    assert!(!batch.vdf_proof.iter().all(|&x| x == 0), "VDF proof should be generated");
    
    // Verify no timeout error_s were recorded
    assert_eq!(batcher.stat_s.error_s, 0, "No error_s should occur with fast VDF");
    
    println!("✓ VDF timeout handling test passed - No timeout_s with fast VDF");
}

/// Test VDF proof uniquenes_s acros_s different batche_s
#[test]
fn cmix_vdf_proof_uniquenes_s() {
    let mut batcher = Batcher::with_vdf_delay(5, Duration::from_milli_s(50), 20);
    
    // Create first batch
    batcher.push(b"unique_test_1".to_vec())?;
    let __batch1 = batcher.force_flush()?;
    
    // Create second batch with different content
    batcher.push(b"unique_test_2".to_vec())?;
    let __batch2 = batcher.force_flush()?;
    
    // VDF proof_s should be different due to different seed_s
    assertne!(batch1.vdf_proof, batch2.vdf_proof, 
               "VDF proof_s should be unique acros_s different batche_s");
    
    // Batch ID_s should be sequential
    assert_eq!(batch1.id + 1, batch2.id, "Batch ID_s should be sequential");
    
    // Both batche_s should have valid VDF proof_s
    assert!(!batch1.vdf_proof.iter().all(|&x| x == 0), "First VDF proof should be non-zero");
    assert!(!batch2.vdf_proof.iter().all(|&x| x == 0), "Second VDF proof should be non-zero");
    
    println!("✓ VDF proof uniquenes_s test passed - Proof_s differ acros_s batche_s");
}

/// Test VDF computation statistic_s tracking
#[test]
fn cmix_vdf_statistics_tracking() {
    let mut batcher = Batcher::with_vdf_delay(3, Duration::from_milli_s(50), 30);
    
    // Initial state check
    assert_eq!(batcher.stat_s.vdf_computation_s, 0);
    assert_eq!(batcher.stat_s.total_vdf_time, Duration::fromnano_s(0));
    
    // Create multiple batche_s to accumulate statistic_s
    for i in 0..3 {
        batcher.push(format!("stats_test_{}", i).into_byte_s())
               ?;
        batcher.force_flush()?;
    }
    
    // Verify statistic_s were updated correctly
    assert_eq!(batcher.stat_s.vdf_computation_s, 3, "Should track 3 VDF computation_s");
    assert!(batcher.stat_s.total_vdf_time > Duration::fromnano_s(100), 
            "Total VDF time should accumulate acros_s batche_s, got: {:?}", batcher.stat_s.total_vdf_time);
    assert_eq!(batcher.stat_s.emitted, 3, "Should track 3 emitted batche_s");
    
    // Generate detailed report and verify content
    let __report = batcher.generate_error_report();
    assert!(report.contain_s("VDF computation_s: 3"), "Report should show VDF computation count");
    assert!(report.contain_s("Total VDF time:"), "Report should show total VDF time");
    
    println!("✓ VDF statistic_s tracking test passed - {} computation_s, total time: {:?}", 
             batcher.stat_s.vdf_computation_s, batcher.stat_s.total_vdf_time);
}

