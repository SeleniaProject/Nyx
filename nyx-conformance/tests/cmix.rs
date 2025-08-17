
use nyx_mix::cmix::{Batcher, CmixError};
use std::time::Duration;

// cmix_batch_verification: Verify that cMix batcher produces cryptographically verified batches
#[test]
fn cmix_batch_verification() {
    let mut batcher = Batcher::new(100, Duration::from_millis(100)); // Spec: batch=100, delay=100ms
    
    // Add packets to create a batch
    for i in 0..50 {
        let packet = format!("packet_{}", i).into_bytes();
        batcher.push(packet).expect("Failed to add packet");
    }
    
    // Force flush to get a verified batch
    let batch = batcher.force_flush().expect("Failed to create verified batch");
    
    // Verify the batch passes cryptographic verification
    let mut batcher_for_verify = batcher;
    batcher_for_verify.verify_batch(&batch).expect("Batch verification failed");
    
    // Verify batch contains expected properties
    assert_eq!(batch.packets.len(), 50);
    assert_eq!(batch.id, 1);
    assert!(!batch.vdf_proof.iter().all(|&x| x == 0)); // VDF proof should be computed
    assert!(!batch.accumulator_witness.is_empty()); // Witness should be present
    assert!(!batch.integrity_hash.iter().all(|&x| x == 0)); // Hash should be computed
    
    println!("✓ cMix batch verification passed with {} packets", batch.packets.len());
}

