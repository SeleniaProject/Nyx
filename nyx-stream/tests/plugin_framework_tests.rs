//! Plugin framework test_s including capability validation
//!
//! Test_s plugin frame type_s, ID range_s, and capability negotiation
//! as specified in `spec/Capability_Negotiation_Policy.md`.

#![allow(clippy::needless_collect)]

use nyx_stream::capability::{get_local_capabilities, negotiate, Capability, CAP_PLUGIN_FRAMEWORK};
use nyx_stream::plugin::{
    is_plugin_frame, PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL, FRAME_TYPE_PLUGIN_DATA,
    FRAME_TYPE_PLUGIN_ERROR, FRAME_TYPE_PLUGIN_HANDSHAKE,
};
use nyx_stream::plugin_frame::PluginFrame;

// 1) frame type validation
#[test]
fn test_plugin_frame_type_validation() {
    for t in 0x50u8..=0x5F {
        assert!(
            is_plugin_frame(t),
            "0x{t:02X} should be recognized as a plugin frame"
        );
    }
    for t in [0x00u8, 0x10, 0x4F, 0x60, 0xFF] {
        assert!(
            !is_plugin_frame(t),
            "0x{t:02X} should NOT be a plugin frame"
        );
    }
}

// 2) header CBOR encode/decode roundtrip
#[test]
fn test_plugin_header_cbor_encoding() -> Result<(), Box<dyn std::error::Error>> {
    let hdr = PluginHeader {
        id: PluginId(42),
        flags: 0b1010_0001,
        data: vec![1, 2, 3, 4, 5],
    };

    let mut buf = Vec::new();
    ciborium::ser::into_writer(&hdr, &mut buf)?;
    let got: PluginHeader = ciborium::de::from_reader(std::io::Cursor::new(&buf))?;

    assert_eq!(hdr, got);
    Ok(())
}

// 3) frame build & parse roundtrip
#[test]
fn test_plugin_frame_building_and_parsing() -> Result<(), Box<dyn std::error::Error>> {
    let hdr = PluginHeader {
        id: PluginId(7),
        flags: 0,
        data: b"hello".to_vec(),
    };
    let payload = b"payload-byte_s-for-plugin";

    for ft in [
        FRAME_TYPE_PLUGIN_HANDSHAKE,
        FRAME_TYPE_PLUGIN_DATA,
        FRAME_TYPE_PLUGIN_CONTROL,
        FRAME_TYPE_PLUGIN_ERROR,
    ] {
        let f = PluginFrame::new(ft, hdr.clone(), payload);
        let serialized = f.to_cbor()?;
        let parsed = PluginFrame::from_cbor(&serialized)?;
        assert_eq!(f, parsed, "roundtrip should keep frame identical");
        assert!(is_plugin_frame(parsed.frame_type));
    }
    Ok(())
}

// 4) basic size limits (sanity): keep encoded size under a reasonable ceiling
//    NOTE: Library does not enforce a hard limit; this test asserts we can
//    encode/decode moderately large frames used by implementations.
#[test]
fn test_plugin_frame_size_limits() -> Result<(), Box<dyn std::error::Error>> {
    let hdr = PluginHeader {
        id: PluginId(9),
        flags: 0,
        data: vec![0u8; 256],
    };
    let payload = vec![0xABu8; 64 * 1024]; // 64 KiB payload typical upper-bound for control/data

    let f = PluginFrame::new(FRAME_TYPE_PLUGIN_DATA, hdr, payload);
    let encoded = f.to_cbor()?;
    // keep within 100 KiB as a sanity envelope for CI boxes
    assert!(
        encoded.len() < 100 * 1024,
        "encoded size too large: {} byte_s",
        encoded.len()
    );
    let decoded = PluginFrame::from_cbor(&encoded)?;
    assert_eq!(f, decoded);
    Ok(())
}

// 5) Test capability negotiation for plugin framework
#[test]
fn test_plugin_framework_capabilitynegotiation() -> Result<(), Box<dyn std::error::Error>> {
    let local_caps = get_local_capabilities();

    // Should contain plugin framework capability
    let plugin_cap = local_caps
        .iter()
        .find(|cap| cap.id == CAP_PLUGIN_FRAMEWORK)
        .ok_or("Plugin framework capability not found")?;

    assert!(
        plugin_cap.is_optional(),
        "Plugin framework should be optional"
    );
    Ok(())
}

// 6) Test negotiation succeeds when peer supports plugin framework
#[test]
fn test_plugin_frameworknegotiation_success() {
    let local_supported = &[nyx_stream::capability::CAP_CORE, CAP_PLUGIN_FRAMEWORK];
    let peer_caps = vec![
        Capability::required(nyx_stream::capability::CAP_CORE, vec![]),
        Capability::optional(CAP_PLUGIN_FRAMEWORK, vec![]),
    ];

    assert!(negotiate(local_supported, &peer_caps).is_ok());
}

// 7) Test negotiation succeeds when peer doesn't request plugin framework
#[test]
fn test_plugin_frameworknegotiation_without_plugins() {
    let local_supported = &[nyx_stream::capability::CAP_CORE]; // No plugin framework
    let peer_caps = vec![
        Capability::required(nyx_stream::capability::CAP_CORE, vec![]),
        // No plugin framework requested
    ];

    assert!(negotiate(local_supported, &peer_caps).is_ok());
}

// 8) Test plugin ID range validation
#[test]
fn test_plugin_id_ranges() {
    // Test valid plugin IDs
    for id in [0u32, 1, 100, 65535, u32::MAX] {
        let plugin_id = PluginId(id);
        assert_eq!(plugin_id.0, id);
    }
}

// 9) Test plugin frame type range correspond_s to capability
#[test]
fn test_plugin_frame_types_and_capability() {
    // Plugin frame type_s should be in reserved range
    for frame_type in 0x50u8..=0x5F {
        assert!(is_plugin_frame(frame_type));
    }

    // Verify specific plugin frame type_s are in range
    assert!(is_plugin_frame(FRAME_TYPE_PLUGIN_HANDSHAKE));
    assert!(is_plugin_frame(FRAME_TYPE_PLUGIN_DATA));
    assert!(is_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL));
    assert!(is_plugin_frame(FRAME_TYPE_PLUGIN_ERROR));
}
