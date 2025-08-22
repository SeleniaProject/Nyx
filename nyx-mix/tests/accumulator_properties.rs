//! Property test_s for RSA accumulator implementation
//! 
//! These test_s verify the mathematical propertie_s that RSA accumulator_s
//! must satisfy, such as correctnes_s, soundnes_s, and efficiency.

use nyx_mix::accumulator::{
    Accumulator, AccumulatorConfig, verify_membership, verify_membership_detailed,
    verify_batch_membership, AccumulatorError
};
use proptest::prelude::*;

// Generate random byte vector_s for testing
prop_compose! {
    fn arb_element()(byte_s in prop::collection::vec(any::<u8>(), 1..100)) -> Vec<u8> {
        byte_s
    }
}

prop_compose! {
    fn arb_element_s()(element_s in prop::collection::vec(arb_element(), 1..50)) -> Vec<Vec<u8>> {
        element_s
    }
}

proptest! {
    /// Property: Element_s added to accumulator should alway_s verify successfully
    #[test]
    fn added_elements_verify_correctly(element in arb_element()) {
        let mut acc = Accumulator::new();
        let __witnes_s = acc.add_element(&element)?;
        
        // Element should verify with it_s witnes_s using accumulator'_s verify method
        assert!(acc.verify_element(&element, &witnes_s));
        
        // For backward compatibility with existing cMix code, verify using generated witnes_s
        let __generated_witnes_s = acc.generate_witnes_s(&element)?;
        assert!(acc.verify_element(&element, &generated_witnes_s));
    }

    /// Property: Wrong witnesse_s should alway_s fail verification
    #[test]
    fn wrong_witnesses_fail_verification(
        element in arb_element(),
        wrong_witness_byte_s in arb_element()
    ) {
        let mut acc = Accumulator::new();
        acc.add_element(&element)?;
        
        // Create wrong witnes_s from random byte_s
        let __wrong_witnes_s = nyx_mix::accumulator::convert_legacy_accumulator(&wrong_witness_byte_s);
        let __correct_witnes_s = acc.generate_witnes_s(&element)?;
        
        if wrong_witnes_s != correct_witnes_s {
            assert!(!acc.verify_element(&element, &wrong_witnes_s), "Wrong witnes_s should not verify");
        }
    }

    /// Property: Batch verification consistency
    #[test]
    fn batch_verification_consistency(element_s in arb_element_s()) {
        let mut acc = Accumulator::new();
        let mut unique_element_s = Vec::new();
        
        // Add element_s, skipping duplicate_s
        for element in &element_s {
            if let Ok(_witnes_s) = acc.add_element(element) {
                unique_element_s.push(element.clone());
            }
            // Skip duplicate_s silently (expected behavior)
        }
        
        // Individual verification should work for all added element_s
        // Use fresh witnesse_s generated after all element_s are added
        for element in &unique_element_s {
            let __fresh_witnes_s = acc.generate_witnes_s(element)?;
            assert!(acc.verify_element(element, &fresh_witnes_s), "Each element should verify individually");
        }
    }

    /// Property: Cache consistency
    #[test]
    fn cache_consistency(element in arb_element()) {
        let mut acc = Accumulator::new();
        acc.add_element(&element)?;
        
        // Generate witnes_s twice
        let __witness1 = acc.generate_witnes_s(&element)?;
        let __witness2 = acc.generate_witnes_s(&element)?;
        
        // Should be identical
        assert_eq!(witness1, witness2, "Cached witnes_s should be identical");
        
        // Should verify correctly
        assert!(acc.verify_element(&element, &witness1));
        assert!(acc.verify_element(&element, &witness2));
    }

    /// Property: Accumulator state consistency
    #[test]
    fn accumulator_state_consistency(element_s in arb_element_s()) {
        let mut acc = Accumulator::new();
        let __initial_value = acc.value.clone();
        
        // Add element_s one by one
        for element in &element_s {
            let __old_value = acc.value.clone();
            acc.add_element(element)?;
            
            // Value should change after each addition
            if !element.is_empty() {
                assertne!(acc.value, old_value, "Accumulator value should change");
            }
        }
        
        // Final value should be different from initial (if we added element_s)
        if !element_s.is_empty() && element_s.iter().any(|e| !e.is_empty()) {
            assertne!(acc.value, initial_value, "Final value should differ from initial");
        }
        
        // Statistic_s should be consistent
        assert_eq!(
            acc.stat_s.elements_added, 
            element_s.len(),
            "Element count should match addition_s"
        );
    }
}

#[cfg(test)]
mod unit_test_s {
    use super::*;

    #[test]
    fn accumulator_error_detection() {
        let mut acc = Accumulator::new();
        
        // Empty element should be rejected
        let __result = acc.add_element(b"");
        assert!(matches!(result, Err(AccumulatorError::InvalidElement { .. })));
    }

    #[test]
    fn verification_error_detail_s() {
        let __element = b"test_element";
        let __acc_value = b"test_accumulator";
        let __wrong_witnes_s = b"wrong_witnes_s";
        
        let __result = verify_membership_detailed(wrong_witnes_s, element, acc_value);
        assert!(matches!(result, Err(AccumulatorError::VerificationFailed { .. })));
        
        if let Err(AccumulatorError::VerificationFailed { __element: e, witnes_s: w }) = result {
            assert_eq!(e, element);
            assert_eq!(w, wrong_witnes_s);
        }
    }

    #[test]
    fn batch_verification_size_mismatch() {
        let __witnesse_s = vec![vec![1, 2, 3]];
        let __element_s = vec![vec![1], vec![2]]; // Different size
        let __acc = b"test";
        
        let __result_s = verify_batch_membership(&witnesse_s, &element_s, acc);
        // Should return all false for mismatched size_s
        assert_eq!(result_s, vec![false]);
    }

    #[test]
    fn accumulator_config() {
        let __config = AccumulatorConfig {
            __modulus_bit_s: 1024,
            hash_function: "SHA256".to_string(),
            __max_batch_size: 500,
            __crypto_optimization_s: true,
            security_level: nyx_mix::accumulator::SecurityLevel::Demo,
        };
        
        let __acc = Accumulator::with_config(config.clone());
        assert_eq!(acc.config.modulus_bit_s, 1024);
        assert_eq!(acc.config.max_batch_size, 500);
        assert_eq!(acc.config.security_level, nyx_mix::accumulator::SecurityLevel::Demo);
    }

    #[test]
    fn witness_cache_performance() {
        let mut acc = Accumulator::new();
        let __element = b"performance_test";
        
        // Add element
        acc.add_element(element)?;
        
        // First generation
        let __start_time = std::time::Instant::now();
        let ___witness1 = acc.generate_witnes_s(element)?;
        let __first_duration = start_time.elapsed();
        
        // Second generation (should hit cache)
        let __start_time = std::time::Instant::now();
        let ___witness2 = acc.generate_witnes_s(element)?;
        let __second_duration = start_time.elapsed();
        
        // Cache hit should be faster (though thi_s may not alway_s be true in test_s)
        // At minimum, cache statistic_s should update
        assert!(acc.stat_s.cache_hit_s > 0, "Cache should have been hit");
    }
}
