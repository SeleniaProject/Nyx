
//! Property-based tests for capability negotiation
//!
//! These tests verify the capability negotiation implementation against
//! the specification in `spec/Capability_Negotiation_Policy.md`.

use proptest::prelude::*;
use nyx_stream::capability::*;

// Generate arbitrary capability IDs
fn capability_id_strategy() -> impl Strategy<Value = u32> {
    any::<u32>()
}

// Generate arbitrary capability flags
fn capability_flags_strategy() -> impl Strategy<Value = u8> {
    prop_oneof![
        Just(FLAG_REQUIRED),
        Just(FLAG_OPTIONAL),
    ]
}

// Generate arbitrary capability data (with size limits)
fn capability_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..=1024)
}

// Generate arbitrary capabilities
fn capability_strategy() -> impl Strategy<Value = Capability> {
    (capability_id_strategy(), capability_flags_strategy(), capability_data_strategy())
        .prop_map(|(id, flags, data)| Capability::new(id, flags, data))
}

proptest! {
    /// Test that CBOR encoding/decoding is lossless
    #[test]
    fn capability_cbor_roundtrip(caps in prop::collection::vec(capability_strategy(), 0..10)) {
        let encoded = encode_caps(&caps).unwrap();
        let decoded = decode_caps(&encoded).unwrap();
        prop_assert_eq!(caps, decoded);
    }

    /// Test that capability flags are correctly interpreted
    #[test]
    fn capability_flags_interpretation(id in capability_id_strategy(), data in capability_data_strategy()) {
        let required = Capability::required(id, data.clone());
        prop_assert!(required.is_required());
        prop_assert!(!required.is_optional());

        let optional = Capability::optional(id, data);
        prop_assert!(!optional.is_required());
        prop_assert!(optional.is_optional());
    }

    /// Test that negotiation succeeds when all required capabilities are supported
    #[test]
    fn negotiation_success_when_supported(
        required_caps in prop::collection::vec(capability_id_strategy(), 1..5),
        optional_caps in prop::collection::vec(capability_id_strategy(), 0..5),
        data in capability_data_strategy()
    ) {
        let mut peer_caps = Vec::new();
        
        // Add required capabilities
        for &id in &required_caps {
            peer_caps.push(Capability::required(id, data.clone()));
        }
        
        // Add optional capabilities
        for &id in &optional_caps {
            peer_caps.push(Capability::optional(id, data.clone()));
        }
        
        // Local supports all required capabilities
        let mut local_supported = required_caps.clone();
        local_supported.extend(&optional_caps);
        
        let result = negotiate(&local_supported, &peer_caps);
        prop_assert!(result.is_ok());
    }

    /// Test that negotiation fails when required capability is missing
    #[test]
    fn negotiation_fails_missing_required(
        supported_caps in prop::collection::vec(capability_id_strategy(), 0..5),
        unsupported_id in capability_id_strategy(),
        data in capability_data_strategy()
    ) {
        // Ensure unsupported_id is not in supported list
        prop_assume!(!supported_caps.contains(&unsupported_id));
        
        let peer_caps = vec![
            Capability::required(unsupported_id, data)
        ];
        
        let result = negotiate(&supported_caps, &peer_caps);
        match result {
            Err(CapabilityError::UnsupportedRequired(id)) => {
                prop_assert_eq!(id, unsupported_id);
            }
            _ => prop_assert!(false, "Expected UnsupportedRequired error"),
        }
    }

    /// Test that optional capabilities never cause negotiation failure
    #[test]
    fn optional_capabilities_never_fail(
        local_supported in prop::collection::vec(capability_id_strategy(), 0..5),
        optional_caps in prop::collection::vec(capability_id_strategy(), 1..10),
        data in capability_data_strategy()
    ) {
        let peer_caps: Vec<_> = optional_caps.iter()
            .map(|&id| Capability::optional(id, data.clone()))
            .collect();
        
        let result = negotiate(&local_supported, &peer_caps);
        prop_assert!(result.is_ok());
    }

    /// Test CLOSE frame building for unsupported capabilities
    #[test]
    fn close_frame_unsupported_capability(cap_id in capability_id_strategy()) {
        let frame = nyx_stream::management::build_close_unsupported_cap(cap_id);
        
        // Frame should be exactly 6 bytes
        prop_assert_eq!(frame.len(), 6);
        
        // Should be parseable back to original ID
        let parsed_id = nyx_stream::management::parse_close_unsupported_cap(&frame);
        prop_assert_eq!(parsed_id, Some(cap_id));
    }

    /// Test capability validation with size limits
    #[test]
    fn capability_validation_size_limits(
        id in capability_id_strategy(),
        flags in capability_flags_strategy(),
        data_size in 0usize..2048
    ) {
        let data = vec![0u8; data_size];
        let cap = Capability::new(id, flags, data);
        
        let result = validate_capability(&cap);
        if data_size <= 1024 {
            prop_assert!(result.is_ok() || 
                        (id == CAP_CORE && data_size > 0)); // Core should be empty
        } else {
            prop_assert!(result.is_err());
        }
    }

    /// Test CBOR decoding size limits
    #[test]
    fn cbor_decode_size_limits(data_size in 0usize..200_000) {
        let data = vec![0u8; data_size];
        let result = decode_caps(&data);
        
        if data_size > 64 * 1024 {
            prop_assert!(result.is_err());
        }
        // Note: Small sizes may still fail due to invalid CBOR, but shouldn't
        // fail due to size limits
    }
}

// Additional specific test cases for plugin frame identification
proptest! {
    #[test]
    fn plugin_frame_identification(ft in 0u8..=255) {
        let is = nyx_stream::plugin::is_plugin_frame(ft);
        let expect = (0x50..=0x5F).contains(&ft);
        prop_assert_eq!(is, expect);
    }
}

