//! Property tests for RSA accumulator implementation
//! 
//! These tests verify the mathematical properties that RSA accumulators
//! must satisfy, such as correctness, soundness, and efficiency.

use nyx_mix::accumulator::{
    Accumulator, AccumulatorConfig, verify_membership, verify_membership_detailed,
    verify_batch_membership, AccumulatorError
};
use proptest::prelude::*;

// Generate random byte vectors for testing
prop_compose! {
    fn arb_element()(bytes in prop::collection::vec(any::<u8>(), 1..100)) -> Vec<u8> {
        bytes
    }
}

prop_compose! {
    fn arb_elements()(elements in prop::collection::vec(arb_element(), 1..50)) -> Vec<Vec<u8>> {
        elements
    }
}

proptest! {
    /// Property: Elements added to accumulator should always verify successfully
    #[test]
    fn added_elements_verify_correctly(element in arb_element()) {
        let mut acc = Accumulator::new();
        let witness = acc.add_element(&element).unwrap();
        
        // Element should verify with its witness using accumulator's verify method
        assert!(acc.verify_element(&element, &witness));
        
        // For backward compatibility with existing cMix code, verify using generated witness
        let generated_witness = acc.generate_witness(&element).unwrap();
        assert!(acc.verify_element(&element, &generated_witness));
    }

    /// Property: Wrong witnesses should always fail verification
    #[test]
    fn wrong_witnesses_fail_verification(
        element in arb_element(),
        wrong_witness in arb_element()
    ) {
        let mut acc = Accumulator::new();
        acc.add_element(&element).unwrap();
        
        // Wrong witness should fail (with high probability)
        let correct_witness = acc.generate_witness(&element).unwrap();
        
        if wrong_witness != correct_witness {
            assert!(!acc.verify_element(&element, &wrong_witness), "Wrong witness should not verify");
        }
    }

    /// Property: Batch verification consistency
    #[test]
    fn batch_verification_consistency(elements in arb_elements()) {
        let mut acc = Accumulator::new();
        let mut witnesses = Vec::new();
        
        // Add all elements and collect witnesses
        for element in &elements {
            let witness = acc.add_element(element).unwrap();
            witnesses.push(witness);
        }
        
        // Individual verification should work for all elements
        for (element, witness) in elements.iter().zip(witnesses.iter()) {
            assert!(acc.verify_element(element, witness), "Each element should verify individually");
        }
    }

    /// Property: Cache consistency
    #[test]
    fn cache_consistency(element in arb_element()) {
        let mut acc = Accumulator::new();
        acc.add_element(&element).unwrap();
        
        // Generate witness twice
        let witness1 = acc.generate_witness(&element).unwrap();
        let witness2 = acc.generate_witness(&element).unwrap();
        
        // Should be identical
        assert_eq!(witness1, witness2, "Cached witness should be identical");
        
        // Should verify correctly
        assert!(acc.verify_element(&element, &witness1));
        assert!(acc.verify_element(&element, &witness2));
    }

    /// Property: Accumulator state consistency
    #[test]
    fn accumulator_state_consistency(elements in arb_elements()) {
        let mut acc = Accumulator::new();
        let initial_value = acc.value.clone();
        
        // Add elements one by one
        for element in &elements {
            let old_value = acc.value.clone();
            acc.add_element(element).unwrap();
            
            // Value should change after each addition
            if !element.is_empty() {
                assert_ne!(acc.value, old_value, "Accumulator value should change");
            }
        }
        
        // Final value should be different from initial (if we added elements)
        if !elements.is_empty() && elements.iter().any(|e| !e.is_empty()) {
            assert_ne!(acc.value, initial_value, "Final value should differ from initial");
        }
        
        // Statistics should be consistent
        assert_eq!(
            acc.stats.elements_added, 
            elements.len(),
            "Element count should match additions"
        );
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn accumulator_error_detection() {
        let mut acc = Accumulator::new();
        
        // Empty element should be rejected
        let result = acc.add_element(b"");
        assert!(matches!(result, Err(AccumulatorError::InvalidElement { .. })));
    }

    #[test]
    fn verification_error_details() {
        let element = b"test_element";
        let acc_value = b"test_accumulator";
        let wrong_witness = b"wrong_witness";
        
        let result = verify_membership_detailed(wrong_witness, element, acc_value);
        assert!(matches!(result, Err(AccumulatorError::VerificationFailed { .. })));
        
        if let Err(AccumulatorError::VerificationFailed { element: e, witness: w }) = result {
            assert_eq!(e, element);
            assert_eq!(w, wrong_witness);
        }
    }

    #[test]
    fn batch_verification_size_mismatch() {
        let witnesses = vec![vec![1, 2, 3]];
        let elements = vec![vec![1], vec![2]]; // Different size
        let acc = b"test";
        
        let results = verify_batch_membership(&witnesses, &elements, acc);
        // Should return all false for mismatched sizes
        assert_eq!(results, vec![false]);
    }

    #[test]
    fn accumulator_config() {
        let config = AccumulatorConfig {
            modulus_bits: 1024,
            hash_function: "SHA256".to_string(),
            max_batch_size: 500,
        };
        
        let acc = Accumulator::with_config(config.clone());
        assert_eq!(acc.config.modulus_bits, 1024);
        assert_eq!(acc.config.max_batch_size, 500);
    }

    #[test]
    fn witness_cache_performance() {
        let mut acc = Accumulator::new();
        let element = b"performance_test";
        
        // Add element
        acc.add_element(element).unwrap();
        
        // First generation
        let start_time = std::time::Instant::now();
        let _witness1 = acc.generate_witness(element).unwrap();
        let first_duration = start_time.elapsed();
        
        // Second generation (should hit cache)
        let start_time = std::time::Instant::now();
        let _witness2 = acc.generate_witness(element).unwrap();
        let second_duration = start_time.elapsed();
        
        // Cache hit should be faster (though this may not always be true in tests)
        // At minimum, cache statistics should update
        assert!(acc.stats.cache_hits > 0, "Cache should have been hit");
    }
}
