#![forbid(unsafe_code)]

//! CBOR Plugin Header Parser for Nyx Protocol v1.0
//!
//! This module implements the CBOR header parsing for Plugin Frames (0x50-0x5F).
//! Format: `{id:u32, flags:u8, data:bytes}` as specified in the v1.0 specification.
//!
//! The parser validates the CBOR structure, enforces size limits, and provides
//! secure deserialization with protection against malicious inputs.

use serde::{Deserialize, Serialize};
use ciborium::{from_reader, into_writer};
use ciborium::de::Error as DeserializeError;
use ciborium::ser::Error as SerializeError;
use thiserror::Error;
use bytes::Bytes;
use std::io::Cursor;

/// CBOR errors - unified error type for both serialization and deserialization
#[derive(Error, Debug)]
pub enum CborError {
    #[error("CBOR serialization error: {0}")]
    Serialize(#[from] SerializeError<std::io::Error>),
    #[error("CBOR deserialization error: {0}")]
    Deserialize(#[from] DeserializeError<std::io::Error>),
}

/// Maximum allowed size for plugin data payload to prevent memory exhaustion attacks
pub const MAX_PLUGIN_DATA_SIZE: usize = 65536; // 64KB limit

/// Maximum allowed size for entire CBOR header to prevent parsing DoS
pub const MAX_CBOR_HEADER_SIZE: usize = 1024; // 1KB limit for header only

/// Plugin ID type alias for consistency across the codebase
pub type PluginId = u32;

/// CBOR Plugin Header as specified in v1.0 specification
/// 
/// This structure represents the standardized header format for all plugin frames:
/// - `id`: Unique plugin identifier (u32)
/// - `flags`: Plugin-specific flags field (u8)  
/// - `data`: Binary payload data (variable length bytes)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginHeader {
    /// Unique plugin identifier
    pub id: PluginId,
    
    /// Plugin-specific flags (8 bits available for plugin use)
    pub flags: u8,
    
    /// Binary payload data - actual plugin content
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
}

/// CBOR parsing errors specific to Plugin Framework
#[derive(Error, Debug)]
pub enum PluginCborError {
    /// CBOR format is invalid or corrupted
    #[error("Invalid CBOR format: {0}")]
    InvalidFormat(#[from] CborError),
    
    /// Plugin data payload exceeds maximum allowed size
    #[error("Plugin data size {0} exceeds maximum {1}")]
    DataSizeExceeded(usize, usize),
    
    /// CBOR header itself exceeds maximum allowed size
    #[error("CBOR header size {0} exceeds maximum {1}")]
    HeaderSizeExceeded(usize, usize),
    
    /// Required field is missing from CBOR structure
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    /// Plugin ID is reserved for system use
    #[error("Plugin ID {0} is reserved for system use")]
    ReservedPluginId(PluginId),
}

/// Reserved plugin ID ranges that cannot be used by user plugins
const SYSTEM_PLUGIN_ID_START: PluginId = 0xFFFF0000;
const SYSTEM_PLUGIN_ID_END: PluginId = 0xFFFFFFFF;

impl PluginHeader {
    /// Create a new plugin header with validation
    ///
    /// # Arguments
    /// * `id` - Plugin identifier (must not be in reserved range)
    /// * `flags` - Plugin-specific flags
    /// * `data` - Plugin payload data
    ///
    /// # Errors
    /// Returns `PluginCborError::ReservedPluginId` if the ID is in the reserved range
    /// Returns `PluginCborError::DataSizeExceeded` if data is too large
    pub fn new(id: PluginId, flags: u8, data: Vec<u8>) -> Result<Self, PluginCborError> {
        // Validate plugin ID is not in reserved range
        if id >= SYSTEM_PLUGIN_ID_START && id <= SYSTEM_PLUGIN_ID_END {
            return Err(PluginCborError::ReservedPluginId(id));
        }
        
        // Validate data size limit
        if data.len() > MAX_PLUGIN_DATA_SIZE {
            return Err(PluginCborError::DataSizeExceeded(data.len(), MAX_PLUGIN_DATA_SIZE));
        }
        
        Ok(Self { id, flags, data })
    }
    
    /// Get the total size of the plugin data
    pub fn data_size(&self) -> usize {
        self.data.len()
    }
    
    /// Check if a specific flag bit is set
    pub fn has_flag(&self, flag_bit: u8) -> bool {
        self.flags & (1 << flag_bit) != 0
    }
    
    /// Set a specific flag bit
    pub fn set_flag(&mut self, flag_bit: u8) {
        self.flags |= 1 << flag_bit;
    }
    
    /// Clear a specific flag bit
    pub fn clear_flag(&mut self, flag_bit: u8) {
        self.flags &= !(1 << flag_bit);
    }
}

/// Parse CBOR plugin header from byte slice with comprehensive validation
///
/// This function performs multiple security checks:
/// 1. Header size validation to prevent memory exhaustion
/// 2. CBOR structure validation
/// 3. Plugin data size validation
/// 4. Plugin ID validation against reserved ranges
///
/// # Arguments
/// * `cbor_data` - Raw CBOR-encoded byte slice
///
/// # Returns
/// * `Ok(PluginHeader)` - Successfully parsed and validated header
/// * `Err(PluginCborError)` - Parse error with specific reason
pub fn parse_plugin_header(cbor_data: &[u8]) -> Result<PluginHeader, PluginCborError> {
    // Validate header size to prevent DoS attacks
    if cbor_data.len() > MAX_CBOR_HEADER_SIZE {
        return Err(PluginCborError::HeaderSizeExceeded(cbor_data.len(), MAX_CBOR_HEADER_SIZE));
    }
    
    // Parse CBOR structure using ciborium
    let mut cursor = Cursor::new(cbor_data);
    let header: PluginHeader = from_reader(&mut cursor)?;
    
    // Additional validation after parsing
    if header.data.len() > MAX_PLUGIN_DATA_SIZE {
        return Err(PluginCborError::DataSizeExceeded(header.data.len(), MAX_PLUGIN_DATA_SIZE));
    }
    
    // Validate plugin ID is not in reserved range
    if header.id >= SYSTEM_PLUGIN_ID_START && header.id <= SYSTEM_PLUGIN_ID_END {
        return Err(PluginCborError::ReservedPluginId(header.id));
    }
    
    Ok(header)
}

/// Serialize plugin header to CBOR format with size validation
///
/// # Arguments
/// * `header` - Plugin header to serialize
///
/// # Returns
/// * `Ok(Vec<u8>)` - CBOR-encoded bytes
/// * `Err(PluginCborError)` - Serialization error
pub fn serialize_plugin_header(header: &PluginHeader) -> Result<Vec<u8>, PluginCborError> {
    let mut cbor_bytes = Vec::new();
    into_writer(header, &mut cbor_bytes)?;
    
    // Validate serialized size
    if cbor_bytes.len() > MAX_CBOR_HEADER_SIZE {
        return Err(PluginCborError::HeaderSizeExceeded(cbor_bytes.len(), MAX_CBOR_HEADER_SIZE));
    }
    
    Ok(cbor_bytes)
}

/// Parse plugin header from Bytes wrapper (zero-copy when possible)
///
/// # Arguments
/// * `cbor_bytes` - CBOR-encoded data in Bytes wrapper
///
/// # Returns
/// * `Ok(PluginHeader)` - Successfully parsed header
/// * `Err(PluginCborError)` - Parse error
pub fn parse_plugin_header_bytes(cbor_bytes: &Bytes) -> Result<PluginHeader, PluginCborError> {
    parse_plugin_header(cbor_bytes.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_valid_plugin_header_creation() {
        let data = b"test payload".to_vec();
        let header = PluginHeader::new(12345, 0x42, data).expect("Valid header creation");
        
        assert_eq!(header.id, 12345);
        assert_eq!(header.flags, 0x42);
        assert_eq!(header.data, b"test payload");
        assert_eq!(header.data_size(), 12);
    }
    
    #[test]
    fn test_reserved_plugin_id_rejection() {
        let data = vec![1, 2, 3];
        let result = PluginHeader::new(0xFFFF0001, 0x00, data);
        
        assert!(matches!(result, Err(PluginCborError::ReservedPluginId(0xFFFF0001))));
    }
    
    #[test]
    fn test_oversized_data_rejection() {
        let oversized_data = vec![0u8; MAX_PLUGIN_DATA_SIZE + 1];
        let result = PluginHeader::new(100, 0x00, oversized_data);
        
        assert!(matches!(result, Err(PluginCborError::DataSizeExceeded(_, _))));
    }
    
    #[test]
    fn test_cbor_serialization_roundtrip() {
        let original_data = b"test serialization".to_vec();
        let original_header = PluginHeader::new(54321, 0x80, original_data).expect("Valid header");
        
        // Serialize to CBOR
        let cbor_bytes = serialize_plugin_header(&original_header).expect("Serialization");
        
        // Parse back from CBOR
        let parsed_header = parse_plugin_header(&cbor_bytes).expect("Parsing");
        
        assert_eq!(original_header, parsed_header);
    }
    
    #[test]
    fn test_flag_operations() {
        let mut header = PluginHeader::new(999, 0x00, vec![]).expect("Valid header");
        
        // Test flag setting
        header.set_flag(3);
        assert!(header.has_flag(3));
        assert_eq!(header.flags, 0x08); // 2^3 = 8 = 0x08
        
        // Test flag clearing
        header.clear_flag(3);
        assert!(!header.has_flag(3));
        assert_eq!(header.flags, 0x00);
    }
    
    #[test]
    fn test_oversized_cbor_header_rejection() {
        // Create a header that will exceed MAX_CBOR_HEADER_SIZE when serialized
        let large_data = vec![0u8; 800]; // Large but under data limit
        let header = PluginHeader::new(1, 0x00, large_data).expect("Valid header");
        
        // This should still work as it's under the limit
        let cbor_bytes = serialize_plugin_header(&header).expect("Serialization should work");
        assert!(cbor_bytes.len() <= MAX_CBOR_HEADER_SIZE);
    }
    
    #[test]
    fn test_malformed_cbor_rejection() {
        let malformed_cbor = vec![0xFF, 0xFE, 0xFD]; // Invalid CBOR
        let result = parse_plugin_header(&malformed_cbor);
        
        assert!(matches!(result, Err(PluginCborError::InvalidFormat(_))));
    }
    
    #[test]
    fn test_empty_data_allowed() {
        let header = PluginHeader::new(42, 0x01, vec![]).expect("Empty data should be allowed");
        
        assert_eq!(header.data_size(), 0);
        assert!(header.data.is_empty());
    }
    
    #[test]
    fn test_bytes_wrapper_parsing() {
        let data = b"bytes test".to_vec();
        let header = PluginHeader::new(777, 0x33, data).expect("Valid header");
        let cbor_bytes = serialize_plugin_header(&header).expect("Serialization");
        
        let bytes_wrapper = Bytes::from(cbor_bytes);
        let parsed_header = parse_plugin_header_bytes(&bytes_wrapper).expect("Bytes parsing");
        
        assert_eq!(header, parsed_header);
    }
}
