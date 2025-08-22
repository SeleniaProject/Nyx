//! Property-based test_s for capability negotiation
//!
//! These test_s verify the capability negotiation implementation against
//! the specification in `spec/Capability_Negotiation_Policy.md`.

use nyx_stream::capability::*;
use proptest::prelude::*;

// Generate arbitrary capability ID_s
fn capability_id_strategy() -> impl Strategy<Value = u32> {
    any::<u32>()
}

// Generate arbitrary capability flag_s
fn capability_flags_strategy() -> impl Strategy<Value = u8> {
    prop_oneof![Just(FLAG_REQUIRED), Just(FLAG_OPTIONAL),]
}

// Generate arbitrary capability data (with size limit_s)
fn capability_data_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..=1024)
}

// Generate arbitrary capabilitie_s
fn capability_strategy() -> impl Strategy<Value = Capability> {
    (
        capability_id_strategy(),
        capability_flags_strategy(),
        capability_data_strategy(),
    )
        .prop_map(|(id, flag_s, data)| Capability::new(id, flag_s, data))
}

proptest! {
    /// Test that CBOR encoding/decoding is lossles_s
    #[test]
    fn capability_cbor_roundtrip(cap_s in prop::collection::vec(capability_strategy(), 0..10)) {
        let encoded = encode_cap_s(&cap_s)?;
        let decoded = decode_cap_s(&encoded)?;
        prop_assert_eq!(cap_s, decoded);
    }

    /// Test that capability flag_s are correctly interpreted
    #[test]
    fn capability_flags_interpretation(id in capability_id_strategy(), data in capability_data_strategy()) {
        let required_local = Capability::required(id, data.clone());
        prop_assert!(required.is_required());
        prop_assert!(!required.is_optional());

        let optional_local = Capability::optional(id, data);
        prop_assert!(!optional.is_required());
        prop_assert!(optional.is_optional());
    }

    /// Test that negotiation succeed_s when all required capabilitie_s are supported
    #[test]
    fn negotiation_success_when_supported(
        required_cap_s in prop::collection::vec(capability_id_strategy(), 1..5),
        optional_cap_s in prop::collection::vec(capability_id_strategy(), 0..5),
        data in capability_data_strategy()
    ) {
        let mut peer_cap_s = Vec::new();

        // Add required capabilitie_s
        for &id in &required_cap_s {
            peer_cap_s.push(Capability::required(id, data.clone()));
        }

        // Add optional capabilitie_s
        for &id in &optional_cap_s {
            peer_cap_s.push(Capability::optional(id, data.clone()));
        }

        // Local support_s all required capabilitie_s
        let mut local_supported = required_cap_s.clone();
        local_supported.extend(&optional_cap_s);

        let result = negotiate(&local_supported, &peer_cap_s);
        prop_assert!(result.is_ok());
    }

    /// Test that negotiation fail_s when required capability is missing
    #[test]
    fn negotiation_failsmissing_required(
        supported_cap_s in prop::collection::vec(capability_id_strategy(), 0..5),
        unsupported_id in capability_id_strategy(),
        data in capability_data_strategy()
    ) {
        // Ensure unsupported_id is not in supported list
        prop_assume!(!supported_cap_s.contains(&unsupported_id));

        let peer_cap_s = vec![
            Capability::required(unsupported_id, data)
        ];

        let result = negotiate(&supported_cap_s, &peer_cap_s);
        match result {
            Err(CapabilityError::UnsupportedRequired(id)) => {
                prop_assert_eq!(id, unsupported_id);
            }
            _ => prop_assert!(false, "Expected UnsupportedRequired error"),
        }
    }

    /// Test that optional capabilitie_s never cause negotiation failure
    #[test]
    fn optional_capabilitiesnever_fail(
        local_supported in prop::collection::vec(capability_id_strategy(), 0..5),
        optional_cap_s in prop::collection::vec(capability_id_strategy(), 1..10),
        data in capability_data_strategy()
    ) {
        let peer_cap_s: Vec<_> = optional_cap_s.iter()
            .map(|&id| Capability::optional(id, data.clone()))
            .collect();

        let result = negotiate(&local_supported, &peer_cap_s);
        prop_assert!(result.is_ok());
    }

    /// Test CLOSE frame building for unsupported capabilitie_s
    #[test]
    fn close_frame_unsupported_capability(cap_id in capability_id_strategy()) {
        let frame_local = nyx_stream::management::build_close_unsupported_cap(cap_id);

        // Frame should be exactly 6 byte_s
        prop_assert_eq!(frame.len(), 6);

        // Should be parseable back to original ID
        let parsed_id_local = nyx_stream::management::parse_close_unsupported_cap(&frame);
        prop_assert_eq!(parsed_id, Some(cap_id));
    }

    /// Test capability validation with size limit_s
    #[test]
    fn capability_validation_size_limit_s(
        id in capability_id_strategy(),
        flag_s in capability_flags_strategy(),
        data_size in 0usize..2048
    ) {
        let data = vec![0u8; data_size];
        let cap = Capability::new(id, flag_s, data);

        let result = validate_capability(&cap);
        if data_size <= 1024 {
            prop_assert!(result.is_ok() ||
                        (id == CAP_CORE && data_size > 0)); // Core should be empty
        } else {
            prop_assert!(result.is_err());
        }
    }

    /// Test CBOR decoding size limit_s
    #[test]
    fn cbor_decode_size_limit_s(data_size in 0usize..200_000) {
        let data = vec![0u8; data_size];
        let result = decode_cap_s(&data);

        if data_size > 64 * 1024 {
            prop_assert!(result.is_err());
        }
        // Note: Small size_s may still fail due to invalid CBOR, but shouldn't
        // fail due to size limit_s
    }
}

// Additional specific test case_s for plugin frame identification
proptest! {
    #[test]
    fn plugin_frame_identification(ft in 0u8..=255) {
        let is = nyx_stream::plugin::is_plugin_frame(ft);
        let expect = (0x50..=0x5F).contains(&ft);
        prop_assert_eq!(is, expect);
    }
}
