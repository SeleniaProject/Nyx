//! Management frame handling and error code_s for Nyx Protocol
//!
//! Thi_s module provide_s utilitie_s for building CLOSE frame_s with specific
//! error code_s, particularly for capability negotiation failu_re_s as defined
//! in `spec/Capability_Negotiation_Policy.md`.

use crate::capability::ERR_UNSUPPORTED_CAP;

/// Build a CLOSE frame for unsupported capability error
///
/// Create_s a CLOSE frame with `ERR_UNSUPPORTED_CAP = 0x07` error code
/// and include_s the unsupported capability ID in the reason field.
///
/// # Argument_s
/// * `id` - The unsupported capability ID (32-bit, stored as 4-byte big-endian)
///
/// # Return_s
/// Vector containing the CLOSE frame byte_s with error code and capability ID
pub fn build_close_unsupported_cap(id: u32) -> Vec<u8> {
    let mut frame = Vec::with_capacity(6);

    // Add error code (2 bytes, big-endian)
    frame.extend_from_slice(&ERR_UNSUPPORTED_CAP.to_be_bytes());

    // Add unsupported capability ID (4 bytes, big-endian)
    frame.extend_from_slice(&id.to_be_bytes());

    frame
}

/// Extract capability ID from CLOSE frame reason
///
/// Parse_s a CLOSE frame reason to extract the unsupported capability ID.
/// Expect_s exactly 6 byte_s: 2 byte_s error code + 4 byte_s capability ID.
///
/// # Argument_s
/// * `reason` - The CLOSE frame reason byte_s
///
/// # Return_s
/// The capability ID if parsing succeed_s, None if format i_s invalid
pub fn parse_close_unsupported_cap(reason: &[u8]) -> Option<u32> {
    if reason.len() != 6 {
        return None;
    }

    // Verify error code matches
    let error_code = u16::from_be_bytes([reason[0], reason[1]]);
    if error_code != ERR_UNSUPPORTED_CAP {
        return None;
    }

    // Extract capability ID
    let cap_id = u32::from_be_bytes([reason[2], reason[3], reason[4], reason[5]]);
    Some(cap_id)
}

/// Management frame type_s and utilitie_s
pub mod frame_type_s {
    /// CLOSE frame type identifier
    pub const CLOSE_FRAME: u8 = 0x00;

    /// PING frame type identifier  
    pub const PING_FRAME: u8 = 0x01;

    /// PONG frame type identifier
    pub const PONG_FRAME: u8 = 0x02;
}

/// Common error code_s for management frame_s
pub mod _error_code_s {
    /// Protocol error
    pub const ERR_PROTOCOL_ERROR: u16 = 0x01;

    /// Internal error  
    pub const ERR_INTERNAL_ERROR: u16 = 0x02;

    /// Flow control error
    pub const ERR_FLOW_CONTROL_ERROR: u16 = 0x03;

    /// Setting_s timeout
    pub const ERR_SETTINGS_TIMEOUT: u16 = 0x04;

    /// Stream closed
    pub const ERR_STREAM_CLOSED: u16 = 0x05;

    /// Frame size error
    pub const ERR_FRAME_SIZE_ERROR: u16 = 0x06;

    /// Unsupported capability (from capability.r_s)
    pub const ERR_UNSUPPORTED_CAP: u16 = super::ERR_UNSUPPORTED_CAP;
}

#[cfg(test)]
mod test_s {
    use super::*;
    use crate::capability::CAP_PLUGIN_FRAMEWORK;

    #[test]
    fn test_build_close_unsupported_cap() {
        let cap_id = CAP_PLUGIN_FRAMEWORK;
        let frame = build_close_unsupported_cap(cap_id);

        assert_eq!(frame.len(), 6);

        // Check error code (first 2 bytes)
        let error_code = u16::from_be_bytes([frame[0], frame[1]]);
        assert_eq!(error_code, ERR_UNSUPPORTED_CAP);

        // Check capability ID (last 4 bytes)
        let parsed_id = u32::from_be_bytes([frame[2], frame[3], frame[4], frame[5]]);
        assert_eq!(parsed_id, cap_id);
    }

    #[test]
    fn test_parse_close_unsupported_cap() -> Result<(), Box<dyn std::error::Error>> {
        let cap_id = 0x12345678u32;
        let frame = build_close_unsupported_cap(cap_id);

        let parsed_id = parse_close_unsupported_cap(&frame).ok_or("Expected Some value")?;
        assert_eq!(parsed_id, cap_id);
        Ok(())
    }

    #[test]
    fn test_parse_close_invalid_length() {
        // Too short
        assert!(parse_close_unsupported_cap(&[0x00, 0x07]).is_none());

        // Too long
        assert!(parse_close_unsupported_cap(&[0x00, 0x07, 0x00, 0x00, 0x00, 0x01, 0xFF]).is_none());
    }

    #[test]
    fn test_parse_close_wrong_error_code() {
        let mut frame = build_close_unsupported_cap(0x1234);

        // Change error code
        frame[0] = 0x00;
        frame[1] = 0x01; // ERR_PROTOCOL_ERROR instead

        assert!(parse_close_unsupported_cap(&frame).is_none());
    }

    #[test]
    fn test_roundtrip_capability_ids() -> Result<(), Box<dyn std::error::Error>> {
        let test_ids = [0x00000001, 0x00000002, 0x12345678, 0xFFFFFFFF];

        for &cap_id in &test_ids {
            let frame = build_close_unsupported_cap(cap_id);
            let parsed = parse_close_unsupported_cap(&frame).ok_or("Expected Some value")?;
            assert_eq!(parsed, cap_id);
        }
        Ok(())
    }
}
