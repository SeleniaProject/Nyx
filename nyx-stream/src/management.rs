#![forbid(unsafe_code)]

//! Management frame definitions (PING/PONG, SETTINGS, etc.) according to Nyx Protocol §16.
//! Currently implements PING (0x31) and PONG (0x32) frames.

use nom::{
    bytes::complete::take,
    number::complete::{be_u16, be_u32, u8 as parse_u8},
};
use nom::{number::complete::be_u64, IResult};
use std::vec::Vec;

/// PING frame (Type=0x31).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PingFrame {
    /// Random nonce echoed back by the peer.
    pub nonce: u64,
}

/// PONG frame (Type=0x32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PongFrame {
    /// Nonce copied from corresponding PING frame.
    pub nonce: u64,
}

/// Parse PING payload (8-byte big-endian nonce).
pub fn parse_ping_frame(input: &[u8]) -> IResult<&[u8], PingFrame> {
    let (input, nonce) = be_u64(input)?;
    Ok((input, PingFrame { nonce }))
}

/// Build PING payload (8-byte big-endian nonce).
pub fn build_ping_frame(frame: &PingFrame) -> [u8; 8] {
    frame.nonce.to_be_bytes()
}

/// Parse PONG payload (8-byte big-endian nonce).
pub fn parse_pong_frame(input: &[u8]) -> IResult<&[u8], PongFrame> {
    let (input, nonce) = be_u64(input)?;
    Ok((input, PongFrame { nonce }))
}

/// Build PONG payload (8-byte big-endian nonce).
pub fn build_pong_frame(frame: &PongFrame) -> [u8; 8] {
    frame.nonce.to_be_bytes()
}

/// CLOSE frame (Type=0x3F).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseFrame<'a> {
    /// Application-defined error code.
    pub code: u16,
    /// Optional human-readable reason string (UTF-8).
    pub reason: &'a [u8],
}

/// Build CLOSE frame payload.
pub fn build_close_frame(code: u16, reason: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + reason.len());
    out.extend_from_slice(&code.to_be_bytes());
    out.push(reason.len() as u8);
    out.extend_from_slice(reason);
    out
}

/// Parse CLOSE frame payload.
pub fn parse_close_frame<'a>(input: &'a [u8]) -> IResult<&'a [u8], CloseFrame<'a>> {
    let (input, code) = be_u16(input)?;
    let (input, len) = parse_u8(input)?;
    let (input, reason_bytes) = take(len)(input)?;
    Ok((
        input,
        CloseFrame {
            code,
            reason: reason_bytes,
        },
    ))
}

/// PATH_CHALLENGE / PATH_RESPONSE token size (128-bit).
const TOKEN_LEN: usize = 16;

/// PATH_CHALLENGE frame (Type=0x33).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathChallengeFrame {
    pub token: [u8; TOKEN_LEN],
}

/// PATH_RESPONSE frame (Type=0x34).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathResponseFrame {
    pub token: [u8; TOKEN_LEN],
}

/// Build PATH_CHALLENGE payload.
pub fn build_path_challenge_frame(token: &[u8; TOKEN_LEN]) -> [u8; TOKEN_LEN] {
    *token
}

/// Build PATH_RESPONSE payload.
pub fn build_path_response_frame(token: &[u8; TOKEN_LEN]) -> [u8; TOKEN_LEN] {
    *token
}

/// Parse PATH_CHALLENGE payload.
pub fn parse_path_challenge_frame(input: &[u8]) -> IResult<&[u8], PathChallengeFrame> {
    let (input, bytes) = take(TOKEN_LEN)(input)?;
    let mut token = [0u8; TOKEN_LEN];
    token.copy_from_slice(bytes);
    Ok((input, PathChallengeFrame { token }))
}

/// Parse PATH_RESPONSE payload.
pub fn parse_path_response_frame(input: &[u8]) -> IResult<&[u8], PathResponseFrame> {
    let (input, bytes) = take(TOKEN_LEN)(input)?;
    let mut token = [0u8; TOKEN_LEN];
    token.copy_from_slice(bytes);
    Ok((input, PathResponseFrame { token }))
}

/// SETTINGS frame (Type=0x30).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setting {
    pub id: u16,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsFrame {
    pub settings: Vec<Setting>,
}

/// Standard SETTINGS parameter IDs as defined in Nyx Protocol v1.0
///
/// These settings are exchanged during connection establishment to negotiate
/// protocol capabilities and requirements between peers.
pub mod setting_ids {
    /// Maximum frame size that can be processed (bytes)
    pub const MAX_FRAME_SIZE: u16 = 0x0001;

    /// Flow control window size (bytes)  
    pub const FLOW_WINDOW_SIZE: u16 = 0x0002;

    /// Maximum number of concurrent streams
    pub const MAX_CONCURRENT_STREAMS: u16 = 0x0003;

    /// Multipath support enabled (0=disabled, 1=enabled)
    pub const MULTIPATH_ENABLED: u16 = 0x0004;

    /// Post-quantum cryptography mode (0=hybrid, 1=pq-only)
    pub const PQ_MODE: u16 = 0x0005;

    /// Plugin support capabilities
    /// - Value is a bitmask indicating supported plugin framework features
    /// - Bit 0: Basic plugin frame processing  
    /// - Bit 1: Dynamic plugin loading
    /// - Bit 2: Sandboxed plugin execution
    /// - Bit 3-31: Reserved for future use
    pub const PLUGIN_SUPPORT: u16 = 0x0010;

    /// Required plugin list (encoded as CBOR array of plugin IDs)
    /// - This setting carries a CBOR-encoded array of u32 plugin IDs
    /// - All listed plugins MUST be supported by the peer
    /// - Connection MUST be closed with ERR_UNSUPPORTED_CAP if any required plugin is unavailable
    pub const PLUGIN_REQUIRED: u16 = 0x0011;

    /// Optional plugin list (encoded as CBOR array of plugin IDs)  
    /// - This setting carries a CBOR-encoded array of u32 plugin IDs
    /// - These plugins are preferred but not mandatory
    /// - Peer should enable these plugins if available
    pub const PLUGIN_OPTIONAL: u16 = 0x0012;

    /// Plugin security policy (bitmask)
    /// - Bit 0: Require signature verification for loaded plugins
    /// - Bit 1: Enable plugin network access
    /// - Bit 2: Enable plugin filesystem access
    /// - Bit 3: Enable plugin IPC with other plugins
    /// - Bit 4-31: Reserved
    pub const PLUGIN_SECURITY_POLICY: u16 = 0x0013;

    /// Low Power preference advertisement
    /// - Value semantics: 0 = normal power, 1 = low power preferred
    /// - This setting allows endpoints to signal power-saving preference so peers
    ///   can adjust keepalive/idle timers and traffic shaping accordingly.
    pub const LOW_POWER_PREFERENCE: u16 = 0x0014;
    /// CBOR-encoded required plugin list (extended SETTINGS payload)
    pub const PLUGIN_REQUIRED_CBOR: u16 = 0x0015;
    /// CBOR-encoded optional plugin list (extended SETTINGS payload)
    pub const PLUGIN_OPTIONAL_CBOR: u16 = 0x0016;
}

/// Plugin support capability flags for PLUGIN_SUPPORT setting
pub mod plugin_support_flags {
    /// Basic plugin frame processing (Type 0x50-0x5F)
    pub const BASIC_FRAMES: u32 = 0x0001;

    /// Dynamic plugin loading from external modules
    pub const DYNAMIC_LOADING: u32 = 0x0002;

    /// Sandboxed plugin execution environment  
    pub const SANDBOXED_EXECUTION: u32 = 0x0004;

    /// Plugin-to-plugin IPC communication
    pub const INTER_PLUGIN_IPC: u32 = 0x0008;

    /// Plugin persistence and state management
    pub const PLUGIN_PERSISTENCE: u32 = 0x0010;
}

/// Plugin security policy flags for PLUGIN_SECURITY_POLICY setting
pub mod plugin_security_flags {
    /// Require cryptographic signature verification for all plugins
    pub const REQUIRE_SIGNATURES: u32 = 0x0001;

    /// Allow plugins to access network resources
    pub const ALLOW_NETWORK: u32 = 0x0002;

    /// Allow plugins to access filesystem
    pub const ALLOW_FILESYSTEM: u32 = 0x0004;

    /// Allow plugins to communicate with each other via IPC
    pub const ALLOW_INTER_PLUGIN_IPC: u32 = 0x0008;

    /// Allow plugins to spawn external processes
    pub const ALLOW_PROCESS_SPAWN: u32 = 0x0010;
}

/// Build SETTINGS payload (concatenated TLVs).
pub fn build_settings_frame(settings: &[Setting]) -> Vec<u8> {
    let mut v: Vec<u8> = Vec::with_capacity(settings.len() * 6);
    for s in settings {
        v.extend_from_slice(&s.id.to_be_bytes());
        v.extend_from_slice(&s.value.to_be_bytes());
    }
    v
}

/// Marker indicating start of extended SETTINGS payload section (id=0xFFFF)
const SETTINGS_EXT_MARKER: u16 = 0xFFFF;

/// Build SETTINGS payload with an optional extension section carrying variable-length values.
/// The extension section layout:
///   [0xFFFF][count:u16] { [id:u16][len:u16][bytes:len] } * count
pub fn build_settings_frame_ext(settings: &[Setting], cbor_ext: &[(u16, &[u8])]) -> Vec<u8> {
    let mut v = build_settings_frame(settings);
    if !cbor_ext.is_empty() {
        v.extend_from_slice(&SETTINGS_EXT_MARKER.to_be_bytes());
        v.extend_from_slice(&(cbor_ext.len() as u16).to_be_bytes());
        for (id, bytes) in cbor_ext {
            v.extend_from_slice(&id.to_be_bytes());
            let len: u16 = (*bytes).len().min(u16::MAX as usize) as u16;
            v.extend_from_slice(&len.to_be_bytes());
            v.extend_from_slice(&bytes[..len as usize]);
        }
    }
    v
}

/// Parse SETTINGS payload into vector.
pub fn parse_settings_frame(input: &[u8]) -> IResult<&[u8], SettingsFrame> {
    let mut rest = input;
    let mut list: Vec<Setting> = Vec::new();
    while !rest.is_empty() {
        let (i, id) = be_u16(rest)?;
        // Stop TLV parsing if extension marker encountered
        if id == SETTINGS_EXT_MARKER {
            rest = rest; // leave marker for extended parser
            break;
        }
        let (i, value) = be_u32(i)?;
        list.push(Setting { id, value });
        rest = i;
    }
    Ok((rest, SettingsFrame { settings: list }))
}

/// Parse SETTINGS payload with optional extension section.
/// Returns (remaining, (frame, ext_pairs)) where ext_pairs are (id, bytes).
pub fn parse_settings_frame_ext(
    input: &[u8],
) -> IResult<&[u8], (SettingsFrame, Vec<(u16, Vec<u8>)>)> {
    let (rest, frame) = parse_settings_frame(input)?;
    let mut ext: Vec<(u16, Vec<u8>)> = Vec::new();
    if rest.is_empty() {
        return Ok((rest, (frame, ext)));
    }
    // Expect marker
    let (i, marker) = be_u16(rest)?;
    if marker != SETTINGS_EXT_MARKER {
        return Ok((rest, (frame, ext))); // unknown trailing data; ignore
    }
    let (mut i, count) = be_u16(i)?;
    for _ in 0..count {
        let (ni, id) = be_u16(i)?;
        i = ni;
        let (ni, len) = be_u16(i)?;
        i = ni;
        let (ni, bytes) = take(len)(i)?;
        i = ni;
        ext.push((id, bytes.to_vec()));
    }
    Ok((i, (frame, ext)))
}

/// Error code for unsupported required capability (Nyx §8)
pub const ERR_UNSUPPORTED_CAP: u16 = 0x07;

/// Convenience: build CLOSE payload for unsupported required capability.
pub fn build_close_unsupported_cap(cap_id: u32) -> Vec<u8> {
    let mut reason = Vec::with_capacity(4);
    reason.extend_from_slice(&cap_id.to_be_bytes());
    build_close_frame(ERR_UNSUPPORTED_CAP, &reason)
}

/// Helper functions for plugin-related SETTINGS processing
#[cfg(feature = "plugin")]
pub mod plugin_settings {
    use super::*;
    use std::io::Cursor;

    /// Encode a list of plugin IDs as CBOR for PLUGIN_REQUIRED/PLUGIN_OPTIONAL settings
    pub fn encode_plugin_list(plugin_ids: &[u32]) -> Vec<u8> {
        let mut buffer = Vec::new();
        let _ = ciborium::into_writer(plugin_ids, &mut buffer).map_err(|_| ());
        buffer
    }

    /// Decode CBOR plugin list from SETTINGS value
    pub fn decode_plugin_list(cbor_data: &[u8]) -> Result<Vec<u32>, String> {
        let mut cursor = Cursor::new(cbor_data);
        ciborium::from_reader::<Vec<u32>, _>(&mut cursor)
            .map_err(|e| format!("CBOR decode error: {}", e))
    }

    /// Create SETTINGS frame advertising required plugins
    pub fn build_plugin_required_setting(plugin_ids: &[u32]) -> Setting {
        Setting {
            id: setting_ids::PLUGIN_REQUIRED,
            value: {
                // For SETTINGS, we store the CBOR length in the value field
                // The actual CBOR data is carried in an extended SETTINGS format
                // or separate negotiation mechanism. For now, we use a simplified approach
                // where the value field contains the count of required plugins.
                plugin_ids.len() as u32
            },
        }
    }

    /// Create SETTINGS frame advertising plugin support capabilities
    pub fn build_plugin_support_setting(capabilities: u32) -> Setting {
        Setting {
            id: setting_ids::PLUGIN_SUPPORT,
            value: capabilities,
        }
    }

    /// Create SETTINGS frame advertising plugin security policy
    pub fn build_plugin_security_setting(policy: u32) -> Setting {
        Setting {
            id: setting_ids::PLUGIN_SECURITY_POLICY,
            value: policy,
        }
    }

    /// Extract plugin-related settings from a SETTINGS frame
    pub fn extract_plugin_settings(frame: &SettingsFrame) -> PluginSettingsInfo {
        let mut info = PluginSettingsInfo::default();

        for setting in &frame.settings {
            match setting.id {
                setting_ids::PLUGIN_SUPPORT => {
                    info.support_flags = setting.value;
                }
                setting_ids::PLUGIN_REQUIRED => {
                    info.required_plugin_count = setting.value;
                }
                setting_ids::PLUGIN_OPTIONAL => {
                    info.optional_plugin_count = setting.value;
                }
                setting_ids::PLUGIN_SECURITY_POLICY => {
                    info.security_policy = setting.value;
                }
                _ => {} // Ignore non-plugin settings
            }
        }

        info
    }

    /// Information extracted from plugin-related SETTINGS
    #[derive(Debug, Clone, Default)]
    pub struct PluginSettingsInfo {
        pub support_flags: u32,
        pub required_plugin_count: u32,
        pub optional_plugin_count: u32,
        pub security_policy: u32,
    }

    impl PluginSettingsInfo {
        /// Check if peer supports basic plugin frame processing
        pub fn supports_plugin_frames(&self) -> bool {
            self.support_flags & super::plugin_support_flags::BASIC_FRAMES != 0
        }

        /// Check if peer supports dynamic plugin loading
        pub fn supports_dynamic_loading(&self) -> bool {
            self.support_flags & super::plugin_support_flags::DYNAMIC_LOADING != 0
        }

        /// Check if peer requires plugin signature verification
        pub fn requires_signatures(&self) -> bool {
            self.security_policy & super::plugin_security_flags::REQUIRE_SIGNATURES != 0
        }

        /// Check if peer allows plugin network access
        pub fn allows_network_access(&self) -> bool {
            self.security_policy & super::plugin_security_flags::ALLOW_NETWORK != 0
        }
    }
}
