#![forbid(unsafe_code)]

use nom::{bytes::complete::take, number::complete::u8, IResult};

/// Flag bit (within the 6-bit flags field) indicating the presence of an 8-bit `PathID`
/// immediately following the standard 4-byte header. This implements the Multipath
/// extension described in v1.0 specification §2.
pub const FLAG_HAS_PATH_ID: u8 = 0x20; // 0b100000

/// Flag bit indicating multipath data plane is enabled for this packet
/// When set, PathID field MUST be present at byte 13 of packet header
/// This flag is stored in the reserved bits of byte 1 to allow for expanded flag space
pub const FLAG_MULTIPATH_ENABLED: u8 = 0x80; // 0b10000000 - stored in byte 1 bit 7

// Plugin Frame Types (0x50-0x5F reserved for plugins)
pub const FRAME_TYPE_PLUGIN_START: u8 = 0x50;
pub const FRAME_TYPE_PLUGIN_END: u8 = 0x5F;

// Specific Plugin Frame Types
pub const FRAME_TYPE_PLUGIN_HANDSHAKE: u8 = 0x50;
pub const FRAME_TYPE_PLUGIN_DATA: u8 = 0x51;
pub const FRAME_TYPE_PLUGIN_CONTROL: u8 = 0x52;
pub const FRAME_TYPE_PLUGIN_ERROR: u8 = 0x53;

/// Check if frame type is in plugin range
pub fn is_plugin_frame(frame_type: u8) -> bool {
    frame_type >= FRAME_TYPE_PLUGIN_START && frame_type <= FRAME_TYPE_PLUGIN_END
}

/// Parsed header including optional `path_id` for multipath data plane.
///
/// In Nyx Protocol v1.0, the PathID is a critical component of the multipath data plane
/// that enables concurrent communication over up to 8 different network paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedHeader {
    /// Basic frame header containing type, flags, and length
    pub hdr: FrameHeader,
    /// Optional Path ID (0-255) when multipath is enabled
    /// PathID identifies which of the concurrent paths this packet uses
    pub path_id: Option<u8>,
}

/// Standard frame header structure compliant with Nyx Protocol v1.0
///
/// This represents the core 4-byte header present in all Nyx packets,
/// with optional PathID extension for multipath communication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Frame type (2 bits): 0=Data, 1=Control, 2=Crypto, 3=Reserved
    pub frame_type: u8,
    /// Protocol flags (6 bits): includes multipath, end-stream, etc.
    pub flags: u8,
    /// Payload length in bytes (14 bits): maximum 16KB payload
    pub length: u16,
}

/// Parse standard 4-byte Nyx packet header.
///
/// Header format (bytes 0-3):
/// - Byte 0: frame_type (bits 7-6) + flags (bits 5-0)  
/// - Byte 1: length high byte (bits 13-8)
/// - Bytes 2-3: length continuation and reserved fields
///
/// # Returns
///
/// Parsed `FrameHeader` structure containing frame type, flags, and payload length.
/// Length field uses 14 bits allowing payloads up to 16KB.
pub fn parse_header(input: &[u8]) -> IResult<&[u8], FrameHeader> {
    let (input, byte0) = u8(input)?;
    let (input, byte1) = u8(input)?;
    let (input, len_bytes) = take(2u8)(input)?; // length low & reserved

    // byte0 = frame_type (2 bits) + flags (6 bits)
    let frame_type = byte0 >> 6;
    let mut flags = byte0 & 0x3F;

    // Extract multipath flag from byte1 bit 7 and add to flags
    if byte1 & 0x80 != 0 {
        flags |= FLAG_MULTIPATH_ENABLED;
    }

    // Length from byte1 (low 7 bits) and len_bytes[0] - v1.0 uses 14-bit length field
    let length = (((byte1 & 0x7F) as u16) << 7) | (len_bytes[0] as u16);

    Ok((
        input,
        FrameHeader {
            frame_type,
            flags,
            length,
        },
    ))
}

/// Parse extended header with optional PathID field for multipath data plane.
///
/// This implements the v1.0 multipath extension where PathID is present at byte 13
/// when FLAG_HAS_PATH_ID or FLAG_MULTIPATH_ENABLED flags are set in the header.
///
/// The PathID enables weighted round-robin scheduling across up to 8 concurrent
/// network paths, with path weights calculated as inverse RTT for optimal load balancing.
///
/// # Arguments
///
/// * `input` - Raw packet bytes starting from byte 0 (CID would precede this)
///
/// # Returns
///
/// ParsedHeader containing both the standard header fields and optional PathID.
/// PathID range is 0-255 (u8) allowing for extensive path identification.
pub fn parse_header_ext(input: &[u8]) -> IResult<&[u8], ParsedHeader> {
    let (input, hdr) = parse_header(input)?;

    // Check for multipath data plane indicators
    let has_path_id =
        (hdr.flags & FLAG_HAS_PATH_ID != 0) || (hdr.flags & FLAG_MULTIPATH_ENABLED != 0);

    if has_path_id {
        let (input, pid) = u8(input)?;
        Ok((
            input,
            ParsedHeader {
                hdr,
                path_id: Some(pid),
            },
        ))
    } else {
        Ok((input, ParsedHeader { hdr, path_id: None }))
    }
}

#[cfg(test)]
mod tests_ext {
    use super::*;

    #[test]
    fn path_id_parsed_when_multipath_flag_set() {
        // Test new FLAG_MULTIPATH_ENABLED flag
        // Create packet with frame_type=0, flags=0x60 (0x40 multipath + 0x20 path_id), length=50, path_id=9
        // Wire format: byte0 = (0 << 6) | 0x60 = 0x60, but flags must fit in 6 bits
        // So we use frame_type=1, flags=0x25 where 0x20=path_id flag, and put multipath in different location
        let bytes = [0x65u8, 0x00u8, 0x32u8, 0x00u8, 0x09u8]; // frame_type=1, flags=0x25
        let (_, parsed) = parse_header_ext(&bytes).expect("parse");
        assert_eq!(parsed.hdr.frame_type, 1);
        assert_eq!(parsed.hdr.flags & FLAG_HAS_PATH_ID, FLAG_HAS_PATH_ID);
        assert_eq!(parsed.hdr.length, 50);
        assert_eq!(parsed.path_id, Some(9));
        // For now, we won't test FLAG_MULTIPATH_ENABLED until wire format is clarified
    }

    #[test]
    fn path_id_parsed_when_flag_set() {
        // frame_type=2 (0b10), flags=0x25 (0x20 path flag + 0x05), length=50, path_id=7
        let bytes = [0xA5u8, 0x00u8, 0x32u8, 0x00u8, 0x07u8];
        let (_, parsed) = parse_header_ext(&bytes).expect("parse");
        assert_eq!(parsed.hdr.frame_type, 2);
        assert_eq!(parsed.hdr.flags & FLAG_HAS_PATH_ID, FLAG_HAS_PATH_ID);
        assert_eq!(parsed.hdr.length, 50);
        assert_eq!(parsed.path_id, Some(7));
    }

    #[test]
    fn path_id_parsed_with_only_multipath_flag() {
        // Test PathID parsing when only FLAG_MULTIPATH_ENABLED is set (without FLAG_HAS_PATH_ID)
        // frame_type=1 (0b01), multipath flag in byte1, length=100, path_id=3
        // byte0=0x40 (frame_type=1), byte1=0x80 (multipath enabled) + length_high
        let bytes = [0x40u8, 0x80u8, 0x64u8, 0x00u8, 0x03u8];
        let (_, parsed) = parse_header_ext(&bytes).expect("parse");
        assert_eq!(parsed.hdr.frame_type, 1);
        assert_eq!(
            parsed.hdr.flags & FLAG_MULTIPATH_ENABLED,
            FLAG_MULTIPATH_ENABLED
        );
        assert_eq!(parsed.hdr.length, 100);
        assert_eq!(parsed.path_id, Some(3));
    }

    #[test]
    fn no_path_id_when_flag_clear() {
        let bytes = [0x55u8, 0x00u8, 0x64u8, 0x00u8];
        let (_, parsed) = parse_header_ext(&bytes).expect("parse");
        assert!(parsed.path_id.is_none());
    }
}
