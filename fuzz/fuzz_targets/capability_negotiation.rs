#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Fuzz capability negotiation CBOR decoding
    // Reference: spec/Capability_Negotiation_Policy_EN.md
    
    // Attempt to decode CBOR capability list
    let _ = ciborium::from_reader::<Vec<u32>, _>(data);
    
    // Also fuzz capability negotiation logic
    use nyx_stream::capability::{CapabilitySet, NegotiationResult};
    
    // Try to create capability set from arbitrary data
    if let Ok(capabilities) = ciborium::from_reader::<Vec<u32>, _>(data) {
        let cap_set = CapabilitySet {
            required: capabilities.iter().take(10).copied().collect(),
            optional: capabilities.iter().skip(10).take(10).copied().collect(),
        };
        
        // Fuzz negotiate function with arbitrary peer capabilities
        let peer_set = CapabilitySet {
            required: capabilities.iter().rev().take(5).copied().collect(),
            optional: capabilities.iter().rev().skip(5).take(5).copied().collect(),
        };
        
        let _ = nyx_stream::capability::negotiate(&cap_set, &peer_set);
    }
});
