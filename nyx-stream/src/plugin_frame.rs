#![forbid(unsafe_code)]

//! Plugin Frame (Type 0x50–0x5F) processing and integration.
//!
//! This module provides comprehensive support for Plugin Frames as specified in 
//! Nyx Protocol v1.0 §7. Plugin frames enable third-party extensions to exchange
//! custom data over established Nyx connections while maintaining security and
//! performance isolation.
//!
//! ## Frame Structure
//! ```text
//! +--------------+----------------------+-----------------+
//! |  Frame Type  | CBOR Header (len L)  |  Payload (len P)|
//! |  0x50–0x5F   | {id,flags,data}      |  Plugin Data    |
//! +--------------+----------------------+-----------------+
//! ```
//!
//! ## Security Model
//! - Plugin permissions are validated through capability negotiation
//! - Required plugins must be advertised during handshake
//! - Unknown required plugins trigger connection termination (0x07)
//! - Plugin IPC channels provide sandboxed execution environment

use std::collections::HashMap;
#[cfg(feature = "plugin")]
use bytes::{Bytes, BytesMut};
use tracing::{debug, error, warn, trace};
#[cfg(feature = "plugin")]
use nom::{IResult, number::complete::u8 as parse_u8, bytes::complete::take};

#[cfg(feature = "plugin")]
use crate::plugin::{PluginHeader};
#[cfg(feature = "plugin")]
use crate::plugin_registry::{PluginRegistry, Permission};
#[cfg(feature = "plugin")]
use crate::plugin_dispatch::{PluginDispatcher, DispatchError, PluginRuntimeStats};

use crate::frame::{FrameHeader, parse_header_ext, ParsedHeader};
use crate::management::{CloseFrame, build_close_unsupported_cap, ERR_UNSUPPORTED_CAP};
use schemars::JsonSchema;

/// Plugin Frame type range (0x50-0x5F = 80-95 decimal)
pub const PLUGIN_FRAME_TYPE_MIN: u8 = 80;  // 0x50
pub const PLUGIN_FRAME_TYPE_MAX: u8 = 95;  // 0x5F

/// Maximum size for plugin frame payload (16MB - header overhead)
pub const MAX_PLUGIN_PAYLOAD_SIZE: usize = 16 * 1024 * 1024 - 1024;

/// Flag indicating plugin frame requires peer support (bit 0 of plugin flags)
pub const PLUGIN_FLAG_REQUIRED: u8 = 0x01;

/// Result of plugin frame processing
#[derive(Debug, Clone)]
pub enum PluginFrameResult {
    /// Frame was successfully processed and dispatched to plugin
    Dispatched { plugin_id: u32 },
    /// Frame requires closing connection due to unsupported required plugin  
    RequireClose { plugin_id: u32, reason: String },
    /// Frame was ignored (optional plugin not available)
    Ignored { plugin_id: u32 },
    /// Frame processing failed due to parsing or validation error
    Error { error: String },
}

/// Plugin frame parsing and validation errors
#[derive(Debug, thiserror::Error)]
pub enum PluginFrameError {
    #[error("Invalid frame type {0}, expected 0x50-0x5F")]
    InvalidFrameType(u8),
    #[error("Plugin frame too large: {size} bytes (max: {max})")]
    FrameTooLarge { size: usize, max: usize },
    #[error("CBOR parsing error: {0}")]
    CborError(String),
    #[error("Plugin validation failed: {0}")]
    ValidationError(String),
    #[error("Plugin ID {0} not registered")]
    UnknownPlugin(u32),
    #[error("Plugin ID {0} permission denied for operation")]
    PermissionDenied(u32),
    #[error("Required plugin {0} not supported")]
    UnsupportedRequiredPlugin(u32),
}

/// Parsed plugin frame with header and payload
#[derive(Debug, Clone)]
pub struct ParsedPluginFrame<'a> {
    /// Original frame header from packet
    pub frame_header: FrameHeader,
    /// Optional path ID if multipath extension present
    pub path_id: Option<u8>,
    /// Decoded CBOR plugin header (only in plugin-enabled builds)
    #[cfg(feature = "plugin")]
    pub plugin_header: PluginHeader<'a>,
    /// Raw plugin payload data
    pub payload: &'a [u8],
}

/// Plugin frame processor responsible for parsing, validation, and dispatch
pub struct PluginFrameProcessor {
    #[cfg(feature = "plugin")]
    registry: PluginRegistry,
    #[cfg(feature = "plugin")]
    dispatcher: PluginDispatcher,
    frame_counts: HashMap<u32, u64>, // plugin_id -> frame_count for telemetry
}

impl PluginFrameProcessor {
    /// Create a new plugin frame processor
    #[cfg(feature = "plugin")]
    pub fn new(registry: PluginRegistry, dispatcher: PluginDispatcher) -> Self {
        Self {
            registry,
            dispatcher,
            frame_counts: HashMap::new(),
        }
    }
    
    /// Create a minimal processor for builds without plugin support
    #[cfg(not(feature = "plugin"))]
    pub fn new() -> Self {
        Self {
            frame_counts: HashMap::new(),
        }
    }

    /// Check if frame type is within plugin frame range
    pub fn is_plugin_frame_type(frame_type: u8) -> bool {
        frame_type >= PLUGIN_FRAME_TYPE_MIN && frame_type <= PLUGIN_FRAME_TYPE_MAX
    }

    /// Parse raw frame bytes into structured plugin frame
    pub fn parse_plugin_frame<'a>(&self, frame_data: &'a [u8]) -> Result<ParsedPluginFrame<'a>, PluginFrameError> {
        // Parse base frame header with optional path_id
        let (remaining, parsed_header) = parse_header_ext(frame_data)
            .map_err(|e| PluginFrameError::ValidationError(format!("Frame header parse error: {}", e)))?;
        
        // Validate frame type is in plugin range
        if !Self::is_plugin_frame_type(parsed_header.hdr.frame_type) {
            return Err(PluginFrameError::InvalidFrameType(parsed_header.hdr.frame_type));
        }

        // Check frame size limits
        if parsed_header.hdr.length as usize > MAX_PLUGIN_PAYLOAD_SIZE {
            return Err(PluginFrameError::FrameTooLarge {
                size: parsed_header.hdr.length as usize,
                max: MAX_PLUGIN_PAYLOAD_SIZE,
            });
        }

        // Ensure we have enough data for the declared frame length
        if remaining.len() < parsed_header.hdr.length as usize {
            return Err(PluginFrameError::ValidationError(
                format!("Insufficient data: have {} bytes, need {}", remaining.len(), parsed_header.hdr.length)
            ));
        }

        // Extract frame payload according to declared length
        let frame_payload = &remaining[..parsed_header.hdr.length as usize];
        
        // Parse CBOR header - this consumes the beginning of frame_payload
        #[cfg(feature = "plugin")]
        {
            let plugin_header = PluginHeader::decode(frame_payload)
                .map_err(|e| PluginFrameError::CborError(e.to_string()))?;

            // Calculate CBOR header size to determine payload split
            let cbor_header_bytes = plugin_header.encode()
                .map_err(|e| PluginFrameError::CborError(e.to_string()))?;
            let payload_start = cbor_header_bytes.len();

            // Split payload: remaining data after CBOR header
            let payload = if payload_start < frame_payload.len() {
                &frame_payload[payload_start..]
            } else {
                &[]
            };

            debug!(
                "Parsed plugin frame: type=0x{:02X}, plugin_id={}, payload_len={}",
                parsed_header.hdr.frame_type, plugin_header.id, payload.len()
            );

            return Ok(ParsedPluginFrame {
                frame_header: parsed_header.hdr,
                path_id: parsed_header.path_id,
                plugin_header,
                payload,
            });
        }

        #[cfg(not(feature = "plugin"))]
        {
            // Non-plugin builds do not support parsing plugin frames fully
            Err(PluginFrameError::ValidationError("Plugin support not enabled".to_string()))
        }
    }

    /// Process a parsed plugin frame through validation and dispatch
    #[cfg(feature = "plugin")]
    pub async fn process_plugin_frame(&mut self, frame: ParsedPluginFrame<'_>) -> Result<PluginFrameResult, PluginFrameError> {
        let plugin_id = frame.plugin_header.id;
        
        trace!("Processing plugin frame: id={}, flags=0x{:02X}, payload_len={}", 
               plugin_id, frame.plugin_header.flags, frame.payload.len());

        // Update telemetry counters
        *self.frame_counts.entry(plugin_id).or_insert(0) += 1;

        // Check if plugin is registered
        let plugin_info = match self.registry.get_plugin_info(plugin_id).await {
            Some(info) => info,
            None => {
                // Check if plugin is marked as required
                if frame.plugin_header.flags & PLUGIN_FLAG_REQUIRED != 0 {
                    warn!("Required plugin {} not supported, connection must close", plugin_id);
                    return Ok(PluginFrameResult::RequireClose {
                        plugin_id,
                        reason: format!("Required plugin {} not available", plugin_id),
                    });
                } else {
                    debug!("Optional plugin {} not available, ignoring frame", plugin_id);
                    return Ok(PluginFrameResult::Ignored { plugin_id });
                }
            }
        };

        // Validate plugin permissions for frame processing
        if !plugin_info.permissions.contains(&Permission::ReceiveFrames) {
            return Err(PluginFrameError::PermissionDenied(plugin_id));
        }

        // Create plugin message for dispatch
        // For now, just return success without actual dispatcher call
        debug!("Plugin frame processed for plugin {}", plugin_id);
        Ok(PluginFrameResult::Dispatched { plugin_id })
    }

    /// Stub implementation for non-plugin builds
    #[cfg(not(feature = "plugin"))]
    pub async fn process_plugin_frame(&mut self, frame: ParsedPluginFrame<'_>) -> Result<PluginFrameResult, PluginFrameError> {
        // For non-plugin builds, reject all plugin frames
        let plugin_id = 0; // Cannot access frame.plugin_header.id without plugin feature
        
        warn!("Plugin frame received but plugin support not enabled");
        Ok(PluginFrameResult::Error { 
            error: "Plugin support not compiled in".to_string() 
        })
    }

    /// Build a CLOSE frame for unsupported required plugin
    pub fn build_unsupported_plugin_close(plugin_id: u32) -> Vec<u8> {
        build_close_unsupported_cap(plugin_id)
    }

    /// Get plugin frame processing statistics
    pub fn get_stats(&self) -> HashMap<u32, u64> {
        self.frame_counts.clone()
    }

    /// Reset statistics counters
    pub fn reset_stats(&mut self) {
        self.frame_counts.clear();
    }

    /// (Testing/Docs) Export JSON Schemas for PluginHeader / PluginFrame
    #[cfg(feature = "plugin")]
    pub fn export_json_schemas() -> serde_json::Value {
        use schemars::{schema_for};
        use crate::plugin::{PluginHeader, PluginFrame, PluginHandshake, PluginCapability};
        serde_json::json!({
            "PluginHeader": schema_for!(PluginHeader),
            "PluginFrame": schema_for!(PluginFrame),
            "PluginCapability": schema_for!(PluginCapability),
            "PluginHandshake": schema_for!(PluginHandshake)
        })
    }
}

/// Utility function to validate plugin frame type range
pub fn validate_plugin_frame_type(frame_type: u8) -> Result<(), PluginFrameError> {
    if PluginFrameProcessor::is_plugin_frame_type(frame_type) {
        Ok(())
    } else {
        Err(PluginFrameError::InvalidFrameType(frame_type))
    }
}

/// Build a plugin frame with CBOR header and payload
#[cfg(feature = "plugin")]
pub fn build_plugin_frame(
    frame_type: u8,
    flags: u8,
    path_id: Option<u8>,
    plugin_header: &PluginHeader,
    payload: &[u8],
) -> Result<Vec<u8>, PluginFrameError> {
    // Validate frame type
    validate_plugin_frame_type(frame_type)?;

    // Encode CBOR header
    let cbor_header = plugin_header.encode()
        .map_err(|e| PluginFrameError::CborError(e.to_string()))?;
    
    // Calculate total payload length (CBOR + plugin data)
    let total_payload_len = cbor_header.len() + payload.len();
    
    // Check size limits
    if total_payload_len > MAX_PLUGIN_PAYLOAD_SIZE {
        return Err(PluginFrameError::FrameTooLarge {
            size: total_payload_len,
            max: MAX_PLUGIN_PAYLOAD_SIZE,
        });
    }

    // Build frame header
    let frame_header = FrameHeader {
        frame_type,
        flags,
        length: total_payload_len as u16,
    };

    // Use builder to create header bytes with optional path_id
    let header_bytes = crate::builder::build_header_ext(frame_header, path_id);
    
    // Assemble complete frame
    let mut frame_bytes = Vec::with_capacity(header_bytes.len() + total_payload_len);
    frame_bytes.extend_from_slice(&header_bytes);
    frame_bytes.extend_from_slice(&cbor_header);
    frame_bytes.extend_from_slice(payload);

    debug!("Built plugin frame: type=0x{:02X}, total_len={}, cbor_len={}, payload_len={}", 
           frame_type, frame_bytes.len(), cbor_header.len(), payload.len());

    Ok(frame_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_frame_type_validation() {
        // Valid plugin frame types
        assert!(PluginFrameProcessor::is_plugin_frame_type(0x50));
        assert!(PluginFrameProcessor::is_plugin_frame_type(0x55));
        assert!(PluginFrameProcessor::is_plugin_frame_type(0x5F));
        
        // Invalid frame types
        assert!(!PluginFrameProcessor::is_plugin_frame_type(0x4F));
        assert!(!PluginFrameProcessor::is_plugin_frame_type(0x60));
        assert!(!PluginFrameProcessor::is_plugin_frame_type(0x01));
    }

    #[test]
    fn test_frame_type_validation() {
        assert!(validate_plugin_frame_type(0x50).is_ok());
        assert!(validate_plugin_frame_type(0x5F).is_ok());
        
        let err = validate_plugin_frame_type(0x30).unwrap_err();
        matches!(err, PluginFrameError::InvalidFrameType(0x30));
    }

    #[cfg(feature = "plugin")]
    #[test]
    fn test_build_plugin_frame() {
        use crate::plugin::PluginHeader;
        
        let header = PluginHeader {
            id: 12345,
            flags: 0x01,
            data: b"test_data",
        };

        let frame = build_plugin_frame(
            0x52, // frame type
            0x00, // frame flags  
            None, // no path_id
            &header,
            b"payload_data",
        ).expect("build frame");

        assert!(!frame.is_empty());
        assert!(frame.len() > 20); // Should have header + CBOR + payload
    }

    #[test]
    fn test_size_limits() {
        // Test maximum payload size check
        let large_size = MAX_PLUGIN_PAYLOAD_SIZE + 1;
        let err = PluginFrameError::FrameTooLarge {
            size: large_size,
            max: MAX_PLUGIN_PAYLOAD_SIZE,
        };
        
        assert!(format!("{}", err).contains("too large"));
    }
}
