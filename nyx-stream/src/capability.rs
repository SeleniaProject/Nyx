//! Capability negotiation implementation for Nyx Protocol v1.0
//!
//! This module implements the capability negotiation system as defined in
//! `spec/Capability_Negotiation_Policy.md`. It provides CBOR-based capability
//! exchange, negotiation algorithms, and error handling for unsupported required capabilities.
//!
//! # Wire Format
//! Capabilities are exchanged as CBOR arrays containing maps with:
//! - `id`: u32 capability identifier
//! - `flags`: u8 flags (bit 0: 1=Required, 0=Optional)
//! - `data`: bytes for version/parameters/sub-features
//!
//! # Error Handling
//! Unsupported required capabilities trigger session termination with
//! `ERR_UNSUPPORTED_CAP = 0x07` and the unsupported capability id in CLOSE reason.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Error codes for capability negotiation failures
pub const ERR_UNSUPPORTED_CAP: u16 = 0x07;

/// Predefined capability ids as per specification
pub const CAP_CORE: u32 = 0x0001;
pub const CAP_PLUGIN_FRAMEWORK: u32 = 0x0002;

/// Local supported capability ids
pub const LOCAL_CAP_IDS: &[u32] = &[CAP_CORE, CAP_PLUGIN_FRAMEWORK];

/// Capability flags
pub const FLAG_REQUIRED: u8 = 0x01;
pub const FLAG_OPTIONAL: u8 = 0x00;

/// A single capability with id, flags, and optional data
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Capability {
    /// Capability identifier (32-bit)
    pub id: u32,
    /// Flags byte (bit 0: Required=1, Optional=0)
    pub flags: u8,
    /// Optional data for versioning/parameters
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

impl Capability {
    /// Create a new capability
    pub fn new(id: u32, flags: u8, data: Vec<u8>) -> Self {
        Self { id, flags, data }
    }

    /// Create a required capability
    pub fn required(id: u32, data: Vec<u8>) -> Self {
        Self::new(id, FLAG_REQUIRED, data)
    }

    /// Create an optional capability
    pub fn optional(id: u32, data: Vec<u8>) -> Self {
        Self::new(id, FLAG_OPTIONAL, data)
    }

    /// Check if this capability is required
    pub fn is_required(&self) -> bool {
        (self.flags & FLAG_REQUIRED) != 0
    }

    /// Check if this capability is optional
    pub fn is_optional(&self) -> bool {
        !self.is_required()
    }
}

/// Error type for capability negotiation failures
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityError {
    /// Unsupported required capability with id
    UnsupportedRequired(u32),
    /// CBOR encoding/decoding error
    CborError(String),
    /// Invalid capability data
    InvalidData(String),
}

impl std::fmt::Display for CapabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapabilityError::UnsupportedRequired(id) => {
                write!(f, "Unsupported required capability: 0x{id:08x}")
            }
            CapabilityError::CborError(msg) => write!(f, "CBOR error: {msg}"),
            CapabilityError::InvalidData(msg) => write!(f, "Invalid capability data: {msg}"),
        }
    }
}

impl std::error::Error for CapabilityError {}

/// Encode capabilities list to CBOR bytes
pub fn encode_caps(capabilities: &[Capability]) -> Result<Vec<u8>, CapabilityError> {
    let mut buffer = Vec::new();
    ciborium::ser::into_writer(capabilities, &mut buffer)
        .map_err(|e| CapabilityError::CborError(e.to_string()))?;
    Ok(buffer)
}

/// Decode capabilities list from CBOR bytes
pub fn decode_caps(data: &[u8]) -> Result<Vec<Capability>, CapabilityError> {
    // Enforce size limits to prevent DoS attacks
    if data.len() > 64 * 1024 {
        return Err(CapabilityError::InvalidData(
            "Capability data too large".to_string(),
        ));
    }

    ciborium::de::from_reader(std::io::Cursor::new(data))
        .map_err(|e| CapabilityError::CborError(e.to_string()))
}

/// Negotiate capabilities between local and peer
///
/// Returns Ok(()) if negotiation succeeds, or Err with the first
/// unsupported required capability id if negotiation fails.
///
/// # Algorithm
/// 1. For each peer capability marked as required
/// 2. Check if local implementation supports it
/// 3. Return error on first unsupported required capability
/// 4. Optional capabilities are always accepted (may be ignored)
pub fn negotiate(local_supported: &[u32], peer_caps: &[Capability]) -> Result<(), CapabilityError> {
    let local_set: HashSet<u32> = local_supported.iter().copied().collect();

    for cap in peer_caps {
        if cap.is_required() && !local_set.contains(&cap.id) {
            return Err(CapabilityError::UnsupportedRequired(cap.id));
        }
    }

    Ok(())
}

/// Get local capabilities that should be advertised to peers
pub fn get_local_capabilities() -> Vec<Capability> {
    vec![
        Capability::required(CAP_CORE, vec![]), // Core protocol is always required
        Capability::optional(CAP_PLUGIN_FRAMEWORK, vec![]), // Plugin framework is optional
    ]
}

/// Validate capability structure and data bounds with comprehensive security checks
///
/// # Security Enhancements
/// - Prevents DoS attacks through oversized capability data
/// - Validates data format and structure integrity
/// - Implements strict bounds checking for all capability types
/// - Detects malformed capabilities that could cause parsing issues
pub fn validate_capability(cap: &Capability) -> Result<(), CapabilityError> {
    // SECURITY ENHANCEMENT: Comprehensive data size validation
    if cap.data.len() > 1024 {
        return Err(CapabilityError::InvalidData(format!(
            "SECURITY: Capability data size {} exceeds maximum 1024 bytes (DoS prevention)",
            cap.data.len()
        )));
    }

    // SECURITY: Prevent zero-size data when data is expected
    if cap.data.is_empty() && matches!(cap.id, CAP_PLUGIN_FRAMEWORK) {
        return Err(CapabilityError::InvalidData(
            "SECURITY: Plugin framework capability requires non-empty data".to_string(),
        ));
    }

    // SECURITY: Validate capability ID range to prevent invalid IDs
    if cap.id > 0xFFFF {
        return Err(CapabilityError::InvalidData(format!(
            "SECURITY: Invalid capability ID {} exceeds maximum allowed value",
            cap.id
        )));
    }

    // Validate known capability ids have expected formats
    match cap.id {
        CAP_CORE => {
            // Core capability should have empty data for v1.0
            if !cap.data.is_empty() {
                return Err(CapabilityError::InvalidData(
                    "SECURITY: Core capability must have empty data for v1.0 (protocol compliance)"
                        .to_string(),
                ));
            }
        }
        CAP_PLUGIN_FRAMEWORK => {
            // SECURITY ENHANCEMENT: Validate plugin framework data format
            if cap.data.len() < 4 {
                return Err(CapabilityError::InvalidData(
                    "SECURITY: Plugin framework capability data too short (minimum 4 bytes required)".to_string(),
                ));
            }

            // Basic sanity check for version information
            let version = u32::from_le_bytes([cap.data[0], cap.data[1], cap.data[2], cap.data[3]]);
            if version > 1000 {
                // Reasonable version number limit
                return Err(CapabilityError::InvalidData(format!(
                    "SECURITY: Plugin framework version {version} exceeds reasonable limit"
                )));
            }
        }
        _ => {
            // Unknown capabilities are allowed (forward compatibility)
            // But we still enforce basic security constraints
            if cap.data.len() > 512 {
                // Stricter limit for unknown capabilities
                return Err(CapabilityError::InvalidData(format!(
                    "SECURITY: Unknown capability {} data size {} exceeds limit for unknown types",
                    cap.id,
                    cap.data.len()
                )));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_flags() {
        let required = Capability::required(CAP_CORE, vec![]);
        assert!(required.is_required());
        assert!(!required.is_optional());

        let optional = Capability::optional(CAP_PLUGIN_FRAMEWORK, vec![]);
        assert!(!optional.is_required());
        assert!(optional.is_optional());
    }

    #[test]
    fn test_cbor_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let caps = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::optional(CAP_PLUGIN_FRAMEWORK, b"v1.0".to_vec()),
        ];

        let encoded = encode_caps(&caps)?;
        let decoded = decode_caps(&encoded)?;

        assert_eq!(caps, decoded);
        Ok(())
    }

    #[test]
    fn test_negotiate_success() {
        let local = &[CAP_CORE, CAP_PLUGIN_FRAMEWORK];
        let peer = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::optional(CAP_PLUGIN_FRAMEWORK, vec![]),
        ];

        assert!(negotiate(local, &peer).is_ok());
    }

    #[test]
    fn test_negotiate_unsupported_required() {
        let local = &[CAP_CORE]; // Missing plugin framework
        let peer = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::required(CAP_PLUGIN_FRAMEWORK, vec![]), // This will fail
        ];

        match negotiate(local, &peer) {
            Err(CapabilityError::UnsupportedRequired(id)) => {
                assert_eq!(id, CAP_PLUGIN_FRAMEWORK);
            }
            other => {
                eprintln!("Expected UnsupportedRequired error, got: {other:?}");
                panic!("Expected UnsupportedRequired error for CAP_PLUGIN_FRAMEWORK");
            }
        }
    }

    #[test]
    fn test_negotiate_optional_unknown() {
        let local = &[CAP_CORE];
        let peer = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::optional(0x9999, vec![]), // Unknown but optional
        ];

        // Should succeed - optional capabilities are always accepted
        assert!(negotiate(local, &peer).is_ok());
    }

    #[test]
    fn test_validate_capability_size_limits() {
        let oversized = Capability::new(CAP_CORE, FLAG_OPTIONAL, vec![0u8; 2048]);
        assert!(validate_capability(&oversized).is_err());

        // Test normal size with non-core capability (core has special validation)
        let normal = Capability::new(CAP_PLUGIN_FRAMEWORK, FLAG_OPTIONAL, vec![0u8; 100]);
        assert!(validate_capability(&normal).is_ok());
    }

    #[test]
    fn test_decode_size_limits() {
        let oversized_data = vec![0u8; 128 * 1024]; // 128KB
        assert!(decode_caps(&oversized_data).is_err());
    }

    #[test]
    fn test_core_capability_validation() {
        // Core capability should have empty data
        let valid_core = Capability::required(CAP_CORE, vec![]);
        assert!(validate_capability(&valid_core).is_ok());

        let invalid_core = Capability::required(CAP_CORE, b"unexpected".to_vec());
        assert!(validate_capability(&invalid_core).is_err());
    }
}
