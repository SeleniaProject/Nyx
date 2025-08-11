#![forbid(unsafe_code)]

//! Basic metrics module for nyx-telemetry
//!
//! Provides essential telemetry metrics without complex external dependencies

use std::time::SystemTime;

/// Basic metrics type
#[derive(Debug, Clone)]
pub struct BasicMetrics {
    pub counter: u64,
    pub last_updated: SystemTime,
}

impl Default for BasicMetrics {
    fn default() -> Self {
        Self {
            counter: 0,
            last_updated: SystemTime::now(),
        }
    }
}

impl BasicMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn increment(&mut self) {
        self.counter += 1;
        self.last_updated = SystemTime::now();
    }
    
    pub fn get_counter(&self) -> u64 {
        self.counter
    }
}
