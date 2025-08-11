#![forbid(unsafe_code)]

//! Integration tests for PathID header functionality in multipath data plane
//! 
//! These tests validate the complete PathID implementation according to the
//! Nyx Protocol v1.0 specification, including header parsing, building,
//! validation, and multipath manager integration.

use nyx_stream::frame::{FrameHeader, ParsedHeader, FLAG_HAS_PATH_ID, FLAG_MULTIPATH_ENABLED};
use nyx_stream::builder::build_header_ext;
use nyx_stream::{parse_header_ext};
use nyx_stream::multipath::{MultipathManager, MultipathConfig};
use nyx_core::types::{PathId, is_valid_user_path_id, CONTROL_PATH_ID};
use std::time::Duration;

#[test]
fn test_pathid_header_round_trip() {
    // Test complete PathID header round-trip: build -> parse -> validate
    let path_id: PathId = 5;
    let original_header = FrameHeader {
        frame_type: 0, // Data frame
        flags: 0x05,   // Base flags
        length: 1200,  // Typical payload size
    };

    // Build header with PathID
    let header_bytes = build_header_ext(original_header, Some(path_id));

    // Parse header back
    let (remaining, parsed) = parse_header_ext(&header_bytes).expect("Failed to parse header");
    assert!(remaining.is_empty(), "Unexpected remaining bytes");

    // Validate PathID extraction
    assert_eq!(parsed.path_id, Some(path_id));
    assert_eq!(parsed.hdr.frame_type, original_header.frame_type);
    assert_eq!(parsed.hdr.length, original_header.length);

    // Verify multipath flags are set correctly
    assert!(parsed.hdr.flags & FLAG_HAS_PATH_ID != 0);
    assert!(parsed.hdr.flags & FLAG_MULTIPATH_ENABLED != 0);
}

#[test]
fn test_pathid_validation_ranges() {
    // Test PathID validation according to v1.0 specification
    
    // Control path (PathID 0) should be valid
    assert_eq!(CONTROL_PATH_ID, 0);

    // User range (1-239) should be valid
    assert!(is_valid_user_path_id(1));
    assert!(is_valid_user_path_id(128));
    assert!(is_valid_user_path_id(239));

    // System range (240-255) should be invalid for user paths
    assert!(!is_valid_user_path_id(240));
    assert!(!is_valid_user_path_id(255));

    // Control path should not be valid for user paths
    assert!(!is_valid_user_path_id(CONTROL_PATH_ID));
}

#[test]
fn test_multipath_flag_combinations() {
    // Test various combinations of multipath flags
    let test_cases = vec![
        (FLAG_HAS_PATH_ID, Some(7u8), true),
        (FLAG_MULTIPATH_ENABLED, Some(3u8), true),
        (FLAG_HAS_PATH_ID | FLAG_MULTIPATH_ENABLED, Some(15u8), true),
        (0u8, None, false), // No flags set
    ];

    for (flags, expected_path_id, should_have_path_id) in test_cases {
        let header = FrameHeader {
            frame_type: 1, // Control frame
            flags,
            length: 64,
        };

        let header_bytes = build_header_ext(header, expected_path_id);
        let (_, parsed) = parse_header_ext(&header_bytes).expect("Parse failed");

        if should_have_path_id {
            assert_eq!(parsed.path_id, expected_path_id);
            assert!(parsed.hdr.flags & (FLAG_HAS_PATH_ID | FLAG_MULTIPATH_ENABLED) != 0);
        } else {
            assert_eq!(parsed.path_id, None);
        }
    }
}

#[test]
fn test_multipath_manager_pathid_integration_minimal() {
    // nyx-stream 内の簡易 MultipathManager を用いて送信経路選択の成立を確認
    let mut mgr = MultipathManager::new(MultipathConfig::default());
    // パス追加と RTT 設定
    mgr.add_path(1, 0).unwrap();
    mgr.add_path(2, 0).unwrap();
    mgr.update_path_rtt(1, Duration::from_millis(50));
    mgr.update_path_rtt(2, Duration::from_millis(120));

    // 最適パスで送信
    let pkt = mgr.send_data(b"hello".to_vec()).expect("no path selected");
    assert!(is_valid_user_path_id(pkt.path_id));
    assert!(pkt.hop_count >= 3 && pkt.hop_count <= 7);
    assert_eq!(pkt.data, b"hello".to_vec());
}

#[test]
fn test_pathid_edge_cases() {
    // Test edge cases for PathID handling
    
    // Test maximum valid user PathID
    let max_user_path_id = 239u8;
    let header = FrameHeader {
        frame_type: 2, // Crypto frame
        flags: 0x00,
        length: 1500,
    };

    let header_bytes = build_header_ext(header, Some(max_user_path_id));
    let (_, parsed) = parse_header_ext(&header_bytes).expect("Parse failed");
    assert_eq!(parsed.path_id, Some(max_user_path_id));

    // Test minimum system PathID (should be invalid for user paths)
    let min_system_path_id = 240u8;
    assert!(!is_valid_user_path_id(min_system_path_id));

    // Test PathID boundary conditions
    assert!(is_valid_user_path_id(1));   // Minimum user PathID
    assert!(is_valid_user_path_id(239)); // Maximum user PathID
    assert!(!is_valid_user_path_id(0));  // Control path (not user)
    assert!(!is_valid_user_path_id(240)); // System range
}

#[test]
fn test_pathid_header_size_optimization() {
    // Test that PathID headers are size-optimized
    let header = FrameHeader {
        frame_type: 0,
        flags: 0,
        length: 100,
    };

    // Header without PathID should be 4 bytes
    let without_pathid = build_header_ext(header, None);
    assert_eq!(without_pathid.len(), 4);

    // Header with PathID should be 5 bytes
    let with_pathid = build_header_ext(header, Some(42));
    assert_eq!(with_pathid.len(), 5);

    // Verify size difference is exactly 1 byte (PathID)
    assert_eq!(with_pathid.len() - without_pathid.len(), 1);
}
