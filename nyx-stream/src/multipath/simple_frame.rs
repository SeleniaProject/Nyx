#![forbid(unsafe_code)]

//! Simple Frame structure for multipath integration
//! This is a simplified frame implementation for multipath testing

use std::collections::HashMap;

/// Simple frame type enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SimpleFrameType {
    Data,
    Control,
}

/// Simple frame structure for multipath integration
#[derive(Debug, Clone)]
pub struct SimpleFrame {
    pub frame_type: SimpleFrameType,
    pub data: Vec<u8>,
    pub flags: u8,
    pub extended_headers: HashMap<String, String>,
}

impl SimpleFrame {
    /// Create a new frame
    pub fn new(frame_type: SimpleFrameType, data: Vec<u8>) -> Self {
        Self {
            frame_type,
            data,
            flags: 0,
            extended_headers: HashMap::new(),
        }
    }

    /// Set a flag
    pub fn set_flag(&mut self, flag: u8) {
        self.flags |= flag;
    }

    /// Add extended header
    pub fn add_extended_header(&mut self, key: &str, value: &str) -> Result<(), String> {
        self.extended_headers.insert(key.to_string(), value.to_string());
        Ok(())
    }

    /// Parse header (simplified)
    pub fn parse_header(&self) -> Result<SimpleHeader, String> {
        Ok(SimpleHeader {
            flags: self.flags,
        })
    }

    /// Parse extended header
    pub fn parse_header_ext(&self) -> Result<HashMap<String, String>, String> {
        Ok(self.extended_headers.clone())
    }
}

/// Simple header structure
#[derive(Debug, Clone)]
pub struct SimpleHeader {
    pub flags: u8,
}
