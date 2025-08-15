#![forbid(unsafe_code)]

use super::frame::FrameHeader;

pub use super::frame::{FLAG_HAS_PATH_ID, FLAG_MULTIPATH_ENABLED};

/// Build standard 4-byte Nyx packet header compliant with v1.0 specification.
///
/// This creates the base header that precedes all Nyx packet payloads.
/// For multipath data plane packets, use `build_header_ext` to include PathID.
///
/// # Arguments
///
/// * `hdr` - FrameHeader containing type, flags, and length information
///
/// # Returns
///
/// Fixed 4-byte array representing the packet header in wire format
pub fn build_header(hdr: FrameHeader) -> [u8; 4] {
    let mut out = [0u8; 4];

    // byte0 = frame_type (2 bits) + flags (6 bits, excluding multipath)
    out[0] = (hdr.frame_type << 6) | (hdr.flags & 0x3F);

    // byte1 = multipath_flag (1 bit) + length high (7 bits)
    let multipath_bit = if hdr.flags & FLAG_MULTIPATH_ENABLED != 0 {
        0x80
    } else {
        0x00
    };
    out[1] = multipath_bit | (((hdr.length >> 7) & 0x7F) as u8);

    // byte2 = length low (7 bits) + reserved (1 bit)
    out[2] = (hdr.length & 0x7F) as u8;

    // byte3 = reserved
    out[3] = 0;

    out
}

/// Build extended header with optional PathID for multipath data plane (v1.0).
///
/// When PathID is provided, automatically sets the appropriate multipath flags
/// and appends the PathID byte at position 13 according to the v1.0 specification.
/// This enables weighted round-robin scheduling across multiple network paths.
///
/// # Arguments
///
/// * `hdr` - Base frame header structure
/// * `path_id` - Optional path identifier (0-255) for multipath routing
///
/// # Returns
///
/// Variable-length Vec<u8> containing header bytes (4 or 5 bytes depending on PathID)
pub fn build_header_ext(hdr: FrameHeader, path_id: Option<u8>) -> Vec<u8> {
    let mut modified_hdr = hdr;

    // If PathID is provided, ensure multipath flags are set correctly
    if path_id.is_some() {
        modified_hdr.flags |= FLAG_HAS_PATH_ID;
        modified_hdr.flags |= FLAG_MULTIPATH_ENABLED;
    }

    let header = build_header(modified_hdr);
    let mut out = Vec::with_capacity(5);
    out.extend_from_slice(&header);

    if let Some(pid) = path_id {
        out.push(pid);
    }

    out
}
