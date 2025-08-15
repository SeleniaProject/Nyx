/// @spec 7. Extended Packet Format
// Updated: simple_frame module provides SimpleFrame structures; previous MultipathFrame placeholder removed.
// (No direct dependency needed for header flag tests.)
use nyx_stream::{
    build_header_ext, parse_header, parse_header_ext, FrameHeader, FLAG_HAS_PATH_ID,
    FLAG_MULTIPATH_ENABLED,
};

// Ensure extended header builder sets both PathID and Multipath flags
// and appends the PathID byte correctly.
#[test]
fn build_ext_sets_flags_and_appends_path_id() {
    let hdr = FrameHeader {
        frame_type: 0,
        flags: 0x00,
        length: 300,
    };
    let bytes = build_header_ext(hdr, Some(42));

    // Extended header must be 5 bytes when PathID is present
    assert_eq!(bytes.len(), 5);

    // Parse base header and verify flags reconstructed correctly
    let (_, parsed_base) = parse_header(&bytes[..4]).expect("parse base header");
    assert_eq!(parsed_base.frame_type, 0);
    assert_eq!(parsed_base.length, 300);
    // Both flags should be visible after parse: HAS_PATH_ID (in byte0 flags) and MULTIPATH (from byte1 bit7)
    assert_ne!(
        parsed_base.flags & FLAG_HAS_PATH_ID,
        0,
        "FLAG_HAS_PATH_ID must be set"
    );
    assert_ne!(
        parsed_base.flags & FLAG_MULTIPATH_ENABLED,
        0,
        "FLAG_MULTIPATH_ENABLED must be set"
    );

    // Parse extended header and verify PathID value
    let (_, parsed_ext) = parse_header_ext(&bytes).expect("parse extended header");
    assert_eq!(parsed_ext.path_id, Some(42));
}

// Ensure builder without PathID does not set multipath-related flags
// and does not append the PathID byte.
#[test]
fn build_ext_without_path_id_has_no_flags_and_no_extra_byte() {
    let hdr = FrameHeader {
        frame_type: 2,
        flags: 0x05,
        length: 127,
    };
    let bytes = build_header_ext(hdr, None);

    // Header must remain 4 bytes without PathID
    assert_eq!(bytes.len(), 4);

    let (_, parsed_base) = parse_header(&bytes).expect("parse base header");
    assert_eq!(parsed_base.frame_type, 2);
    assert_eq!(parsed_base.length, 127);
    // Multipath specific flags must not be set
    assert_eq!(parsed_base.flags & FLAG_HAS_PATH_ID, 0);
    assert_eq!(parsed_base.flags & FLAG_MULTIPATH_ENABLED, 0);

    // Extended parse should not find a PathID
    let (_, parsed_ext) = parse_header_ext(&bytes).expect("parse extended header");
    assert!(parsed_ext.path_id.is_none());
}

// Ensure that PathID is parsed when only the Multipath flag (byte1 bit 7) is set
// even if FLAG_HAS_PATH_ID in the 6-bit flags field is not set.
#[test]
fn parse_with_only_multipath_flag_reads_path_id() {
    // frame_type = 1 -> byte0 upper bits = 0b01, flags lower 6 bits = 0x00
    // length = 100 => high7=0, low7=100
    // byte0 = (1<<6) | 0x00 = 0x40
    // byte1 = 0x80 (multipath enabled) | high7(0) = 0x80
    // byte2 = low7(100) = 100, byte3 = reserved = 0x00
    // path_id = 0x2A (42)
    let bytes = [0x40u8, 0x80u8, 100u8, 0x00u8, 0x2Au8];

    // Base header parse should reconstruct multipath flag into flags field
    let (_, parsed_base) = parse_header(&bytes[..4]).expect("parse base header");
    assert_eq!(parsed_base.frame_type, 1);
    assert_ne!(parsed_base.flags & FLAG_MULTIPATH_ENABLED, 0);
    assert_eq!(parsed_base.length, 100);

    // Extended header parse should read PathID
    let (_, parsed_ext) = parse_header_ext(&bytes).expect("parse extended header");
    assert_eq!(parsed_ext.path_id, Some(42));
}
