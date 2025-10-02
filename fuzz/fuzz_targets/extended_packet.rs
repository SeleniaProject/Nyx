#![no_main]

use libfuzzer_sys::fuzz_target;
use nyx_core::extended_packet::ExtendedPacketHeader;

fuzz_target!(|data: &[u8]| {
    // Fuzz extended packet header decoding
    // Reference: spec/Nyx_Protocol_v1.0_Spec_EN.md §7
    
    // Attempt to decode packet header
    let _ = ExtendedPacketHeader::decode(data);
    
    // Fuzz should not panic on any input
    // Invalid packets should return Err, not crash
});
