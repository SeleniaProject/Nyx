
use nyx_mix::cmix::{Batcher, CmixError};
use std::time::Duration;

#[test]
fn cmix_verify_rejects_tampered_batch() {
    let mut batcher = Batcher::new(10, Duration::from_millis(50));
    
    // Create a valid batch
    batcher.push(b"original_packet".to_vec()).expect("Failed to add packet");
    let mut batch = batcher.force_flush().expect("Failed to create batch");
    
    // Tamper with the batch by modifying packet contents
    batch.packets[0] = b"tampered_packet".to_vec();
    
    // Verification should fail due to tampering
    let result = batcher.verify_batch(&batch);
    assert!(result.is_err());
    
    match result.unwrap_err() {
        CmixError::TamperedBatch { batch_id, .. } => {
            assert_eq!(batch_id, batch.id);
            println!("✓ Correctly detected tampered batch {}", batch_id);
        }
        other => panic!("Expected TamperedBatch error, got: {:?}", other),
    }
}

#[test]
fn cmix_verify_rejects_invalid_witness() {
    let mut batcher = Batcher::new(10, Duration::from_millis(50));
    
    // Create a valid batch
    batcher.push(b"test_packet".to_vec()).expect("Failed to add packet");
    let mut batch = batcher.force_flush().expect("Failed to create batch");
    
    // Corrupt the accumulator witness
    batch.accumulator_witness = vec![0xFF; 32]; // Invalid witness
    
    // Verification should fail due to invalid witness
    let result = batcher.verify_batch(&batch);
    assert!(result.is_err());
    
    match result.unwrap_err() {
        CmixError::InvalidWitness { element, witness } => {
            assert_eq!(element, batch.id.to_le_bytes().to_vec());
            assert_eq!(witness, vec![0xFF; 32]);
            println!("✓ Correctly detected invalid witness for batch {}", batch.id);
        }
        other => panic!("Expected InvalidWitness error, got: {:?}", other),
    }
}

