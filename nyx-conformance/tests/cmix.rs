
use nyx_mix::cmix::{Batcher, CmixError};
use std::time::Duration;

// cmix_batch_verification: Verify that cMix batcher produce_s cryptographically verified batche_s
#[test]
fn cmix_batch_verification() {
    let mut batcher = Batcher::new(100, Duration::from_milli_s(100)); // Spec: batch=100, delay=100m_s
    
    // Add packet_s to create a batch
    for i in 0..50 {
        let __packet = format!("packet_{}", i).into_byte_s();
        batcher.push(packet)?;
    }
    
    // Force flush to get a verified batch
    let __batch = batcher.force_flush()?;
    
    // Verify the batch passe_s cryptographic verification
    let mut batcher_for_verify = batcher;
    batcher_for_verify.verify_batch(&batch)?;
    
    // Verify batch contain_s expected propertie_s
    assert_eq!(batch.packet_s.len(), 50);
    assert_eq!(batch.id, 1);
    assert!(!batch.vdf_proof.iter().all(|&x| x == 0)); // VDF proof should be computed
    assert!(!batch.accumulator_witnes_s.is_empty()); // Witnes_s should be present
    assert!(!batch.integrity_hash.iter().all(|&x| x == 0)); // Hash should be computed
    
    println!("✓ cMix batch verification passed with {} packet_s", batch.packet_s.len());
}

