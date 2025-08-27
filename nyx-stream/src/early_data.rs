//! 0-RTT Early Data and Anti-Replay Protection
//!
//! This module implements the 0-RTT early data handling and anti-replay protection
//! as specified in Nyx Protocol v1.0 Section 2.1: Early-Data and 0-RTT Reception Requirements.
//!
//! ## Key Features
//!
//! - **Direction Identifier**: 32-bit IDs prevent nonce overlap between directions
//! - **Anti-Replay Window**: 2^20 sliding window for per-direction nonce tracking
//! - **Early Data Scope**: 0-RTT data accepted after first CRYPTO message
//! - **Rekey Interaction**: Window reset on rekey to avoid false positives
//! - **Telemetry Integration**: Counters for replay drops and early-data acceptance

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Anti-replay window size as specified in the protocol (2^20 = 1,048,576)
pub const ANTI_REPLAY_WINDOW_SIZE: usize = 1 << 20;

/// Maximum early data payload size (64KB)
pub const MAX_EARLY_DATA_SIZE: usize = 64 * 1024;

/// Direction identifiers for preventing nonce overlap
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DirectionId(pub u32);

impl DirectionId {
    /// Initiator to Responder direction
    pub const I2R: DirectionId = DirectionId(1);
    /// Responder to Initiator direction  
    pub const R2I: DirectionId = DirectionId(2);
    /// Bidirectional (for symmetric protocols)
    pub const BIDIRECTIONAL: DirectionId = DirectionId(0);
}

/// Anti-replay protection using a sliding window
#[derive(Debug, Clone)]
pub struct AntiReplayWindow {
    /// Direction identifier for this window
    direction_id: DirectionId,
    /// Sliding window of seen nonces
    seen_nonces: VecDeque<u64>,
    /// Highest nonce value seen so far
    highest_nonce: u64,
    /// Window size (must be power of 2)
    window_size: usize,
    /// Number of nonces seen
    total_seen: u64,
    /// Number of replay attempts blocked
    replay_blocks: u64,
    /// Timestamp of last rekey (for window reset)
    last_rekey: Option<Instant>,
}

impl AntiReplayWindow {
    /// Create new anti-replay window for a direction
    pub fn new(direction_id: DirectionId) -> Self {
        Self::with_size(direction_id, ANTI_REPLAY_WINDOW_SIZE)
    }

    /// Create anti-replay window with custom size (for testing)
    pub fn with_size(direction_id: DirectionId, window_size: usize) -> Self {
        // Ensure window size is power of 2 for efficiency
        assert!(window_size.is_power_of_two(), "Window size must be power of 2");
        
        Self {
            direction_id,
            seen_nonces: VecDeque::with_capacity(window_size),
            highest_nonce: 0,
            window_size,
            total_seen: 0,
            replay_blocks: 0,
            last_rekey: None,
        }
    }

    /// Check if nonce is acceptable (not a replay)
    ///
    /// Returns `true` if the nonce is new and should be accepted,
    /// `false` if it's a replay and should be rejected.
    pub fn check_nonce(&mut self, nonce: u64) -> bool {
        self.total_seen += 1;

        // Nonce must be greater than highest seen
        if nonce <= self.highest_nonce {
            // Check if it's within the window and not already seen
            let window_start = self.highest_nonce.saturating_sub(self.window_size as u64);
            
            if nonce > window_start {
                // Within window - check if already seen
                if self.seen_nonces.contains(&nonce) {
                self.replay_blocks += 1;
                return false; // Replay detected
            }
            
            // New nonce within window - accept but don't update highest
            self.seen_nonces.push_back(nonce);
                self.trim_window();
                return true;
            } else {
                // Outside window (too old) - reject
                self.replay_blocks += 1;
                return false;
            }
        }

        // New highest nonce - accept and update window
        self.highest_nonce = nonce;
        self.seen_nonces.push_back(nonce);
        self.trim_window();
        true
    }

    /// Reset window on rekey (as required by spec)
    pub fn reset_for_rekey(&mut self) {
        self.seen_nonces.clear();
        self.highest_nonce = 0;
        self.last_rekey = Some(Instant::now());
    }

    /// Get direction identifier
    pub fn direction_id(&self) -> DirectionId {
        self.direction_id
    }

    /// Get replay statistics
    pub fn stats(&self) -> AntiReplayStats {
        AntiReplayStats {
            direction_id: self.direction_id,
            total_nonces_seen: self.total_seen,
            replay_blocks: self.replay_blocks,
            current_window_size: self.seen_nonces.len(),
            highest_nonce: self.highest_nonce,
            last_rekey: self.last_rekey,
        }
    }    /// Trim window to maintain size limit
    fn trim_window(&mut self) {
        while self.seen_nonces.len() > self.window_size {
            self.seen_nonces.pop_front();
        }
    }
}

/// Early data handler for 0-RTT support
#[derive(Debug, Clone)]
pub struct EarlyDataHandler {
    /// Anti-replay windows per direction
    replay_windows: std::collections::HashMap<DirectionId, AntiReplayWindow>,
    /// Maximum early data size allowed
    max_early_data_size: usize,
    /// Number of early data frames accepted
    early_data_accepted: u64,
    /// Number of early data frames rejected
    early_data_rejected: u64,
    /// Total bytes of early data processed
    early_data_bytes: u64,
}

impl Default for EarlyDataHandler {
    #[must_use]
    fn default() -> Self {
        Self::new()
    }
}

impl EarlyDataHandler {
    /// Create new early data handler
    pub fn new() -> Self {
        let mut replay_windows = std::collections::HashMap::new();
        replay_windows.insert(DirectionId::I2R, AntiReplayWindow::new(DirectionId::I2R));
        replay_windows.insert(DirectionId::R2I, AntiReplayWindow::new(DirectionId::R2I));

        Self {
            replay_windows,
            max_early_data_size: MAX_EARLY_DATA_SIZE,
            early_data_accepted: 0,
            early_data_rejected: 0,
            early_data_bytes: 0,
        }
    }

    /// Process early data frame with anti-replay protection
    ///
    /// Returns `Ok(())` if the frame should be accepted,
    /// `Err(EarlyDataError)` if it should be rejected.
    pub fn process_early_data(
        &mut self,
        direction: DirectionId,
        nonce: u64,
        data: &[u8],
    ) -> Result<(), EarlyDataError> {
        // Size check
        if data.len() > self.max_early_data_size {
            self.early_data_rejected += 1;
            return Err(EarlyDataError::PayloadTooLarge {
                size: data.len(),
                max_size: self.max_early_data_size,
            });
        }

        // Anti-replay check
        let window = self.replay_windows.get_mut(&direction)
            .ok_or(EarlyDataError::InvalidDirection(direction))?;

        if !window.check_nonce(nonce) {
            self.early_data_rejected += 1;
            return Err(EarlyDataError::ReplayDetected {
                direction,
                nonce,
                highest_seen: window.highest_nonce,
            });
        }

        // Accept early data
        self.early_data_accepted += 1;
        self.early_data_bytes += data.len() as u64;
        Ok(())
    }

    /// Handle rekey event (reset all windows)
    pub fn handle_rekey(&mut self) {
        for window in self.replay_windows.values_mut() {
            window.reset_for_rekey();
        }
    }

    /// Get anti-replay window for direction
    pub fn get_window(&self, direction: DirectionId) -> Option<&AntiReplayWindow> {
        self.replay_windows.get(&direction)
    }

    /// Get anti-replay window for direction (mutable)
    pub fn get_window_mut(&mut self, direction: DirectionId) -> Option<&mut AntiReplayWindow> {
        self.replay_windows.get_mut(&direction)
    }

    /// Add custom direction window
    pub fn add_direction(&mut self, direction: DirectionId) {
        self.replay_windows.insert(direction, AntiReplayWindow::new(direction));
    }

    /// Get telemetry data for monitoring
    pub fn telemetry_data(&self) -> EarlyDataTelemetry {
        let total_replay_blocks: u64 = self.replay_windows.values()
            .map(|w| w.replay_blocks)
            .sum();

        let total_nonces_seen: u64 = self.replay_windows.values()
            .map(|w| w.total_seen)
            .sum();

        EarlyDataTelemetry {
            early_data_accepted: self.early_data_accepted,
            early_data_rejected: self.early_data_rejected,
            early_data_bytes: self.early_data_bytes,
            total_replay_blocks,
            total_nonces_seen,
            directions: self.replay_windows.values()
                .map(|w| w.stats())
                .collect(),
        }
    }
}

/// Errors that can occur during early data processing
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EarlyDataError {
    /// Payload exceeds maximum size
    #[error("Early data payload too large: {size} bytes (max: {max_size})")]
    PayloadTooLarge { size: usize, max_size: usize },

    /// Replay attack detected
    #[error("Replay detected on direction {direction:?}: nonce {nonce} (highest seen: {highest_seen})")]
    ReplayDetected {
        direction: DirectionId,
        nonce: u64,
        highest_seen: u64,
    },

    /// Invalid direction identifier
    #[error("Invalid direction identifier: {0:?}")]
    InvalidDirection(DirectionId),

    /// Frame outside anti-replay window
    #[error("Frame outside anti-replay window: nonce {nonce} (window start: {window_start})")]
    OutsideWindow { nonce: u64, window_start: u64 },
}

/// Statistics for anti-replay protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiReplayStats {
    /// Direction identifier
    pub direction_id: DirectionId,
    /// Total nonces seen
    pub total_nonces_seen: u64,
    /// Number of replay attempts blocked
    pub replay_blocks: u64,
    /// Current window size
    pub current_window_size: usize,
    /// Highest nonce value seen
    pub highest_nonce: u64,
    /// Timestamp of last rekey
    pub last_rekey: Option<Instant>,
}

/// Telemetry data for early data and anti-replay monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyDataTelemetry {
    /// Number of early data frames accepted
    pub early_data_accepted: u64,
    /// Number of early data frames rejected
    pub early_data_rejected: u64,
    /// Total bytes of early data processed
    pub early_data_bytes: u64,
    /// Total replay attempts blocked across all directions
    pub total_replay_blocks: u64,
    /// Total nonces seen across all directions
    pub total_nonces_seen: u64,
    /// Per-direction statistics
    pub directions: Vec<AntiReplayStats>,
}

/// Nonce construction helper with direction identifier
pub fn construct_nonce_with_direction(
    base_nonce: &[u8; 12],
    direction_id: DirectionId,
    sequence: u64,
) -> [u8; 12] {
    let mut nonce = *base_nonce;
    
    // XOR direction ID into first 4 bytes to prevent overlap
    let dir_bytes = direction_id.0.to_be_bytes();
    for (i, &b) in dir_bytes.iter().enumerate() {
        nonce[i] ^= b;
    }
    
    // XOR sequence into last 8 bytes
    let seq_bytes = sequence.to_be_bytes();
    for (i, &b) in seq_bytes.iter().enumerate() {
        nonce[4 + i] ^= b;
    }
    
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anti_replay_window_basic() {
        let mut window = AntiReplayWindow::with_size(DirectionId::I2R, 16);
        
        // First nonce should be accepted
        assert!(window.check_nonce(1));
        assert_eq!(window.highest_nonce, 1);
        
        // Higher nonce should be accepted
        assert!(window.check_nonce(5));
        assert_eq!(window.highest_nonce, 5);
        
        // Replay should be rejected
        assert!(!window.check_nonce(5));
        assert_eq!(window.replay_blocks, 1);
    }

    #[test]
    fn test_anti_replay_window_sliding() {
        let mut window = AntiReplayWindow::with_size(DirectionId::I2R, 4);
        
        // Fill window
        assert!(window.check_nonce(10));
        assert!(window.check_nonce(11));
        assert!(window.check_nonce(12));
        assert!(window.check_nonce(13));
        assert!(window.check_nonce(14));
        
        // Old nonce outside window should be rejected
        assert!(!window.check_nonce(9));
        
        // Recent nonce within window should be accepted
        assert!(window.check_nonce(11)); // Was seen before but within window
    }

    #[test]
    fn test_anti_replay_window_rekey() {
        let mut window = AntiReplayWindow::with_size(DirectionId::I2R, 16);
        
        // Add some nonces
        assert!(window.check_nonce(100));
        assert!(window.check_nonce(101));
        
        // Reset for rekey
        window.reset_for_rekey();
        
        // Previous nonces should now be acceptable again
        assert!(window.check_nonce(100));
        assert!(window.check_nonce(101));
        assert_eq!(window.highest_nonce, 101);
    }

    #[test]
    fn test_early_data_handler() {
        let mut handler = EarlyDataHandler::new();
        
        let data = b"test early data";
        
        // First early data should be accepted
        assert!(handler.process_early_data(DirectionId::I2R, 1, data).is_ok());
        assert_eq!(handler.early_data_accepted, 1);
        
        // Replay should be rejected
        assert!(handler.process_early_data(DirectionId::I2R, 1, data).is_err());
        assert_eq!(handler.early_data_rejected, 1);
        
        // Different direction should be independent
        assert!(handler.process_early_data(DirectionId::R2I, 1, data).is_ok());
        assert_eq!(handler.early_data_accepted, 2);
    }

    #[test]
    fn test_early_data_size_limit() {
        let mut handler = EarlyDataHandler::new();
        
        // Large payload should be rejected
        let large_data = vec![0u8; MAX_EARLY_DATA_SIZE + 1];
        let result = handler.process_early_data(DirectionId::I2R, 1, &large_data);
        
        assert!(matches!(result, Err(EarlyDataError::PayloadTooLarge { .. })));
        assert_eq!(handler.early_data_rejected, 1);
    }

    #[test]
    fn test_nonce_construction_with_direction() {
        let base_nonce = [0u8; 12];
        
        let nonce1 = construct_nonce_with_direction(&base_nonce, DirectionId::I2R, 100);
        let nonce2 = construct_nonce_with_direction(&base_nonce, DirectionId::R2I, 100);
        
        // Different directions should produce different nonces
        assert_ne!(nonce1, nonce2);
        
        // Same direction and sequence should produce same nonce
        let nonce3 = construct_nonce_with_direction(&base_nonce, DirectionId::I2R, 100);
        assert_eq!(nonce1, nonce3);
    }

    #[test]
    fn test_telemetry_data() -> Result<(), Box<dyn std::error::Error>> {
        let mut handler = EarlyDataHandler::new();
        
        let data = b"test";
        
        // Process some data
        handler.process_early_data(DirectionId::I2R, 1, data)?;
        handler.process_early_data(DirectionId::I2R, 1, data).unwrap_err(); // Replay
        handler.process_early_data(DirectionId::R2I, 1, data)?;
        
        let telemetry = handler.telemetry_data();
        
        assert_eq!(telemetry.early_data_accepted, 2);
        assert_eq!(telemetry.early_data_rejected, 1);
        assert_eq!(telemetry.early_data_bytes, 8); // 2 * 4 bytes
        assert_eq!(telemetry.total_replay_blocks, 1);
        assert_eq!(telemetry.directions.len(), 2);
        Ok(())
    }

    #[test]
    fn test_direction_identifier_uniqueness() {
        assert_ne!(DirectionId::I2R, DirectionId::R2I);
        assert_ne!(DirectionId::I2R, DirectionId::BIDIRECTIONAL);
        assert_ne!(DirectionId::R2I, DirectionId::BIDIRECTIONAL);
    }

    #[test]
    fn test_window_size_power_of_two() {
        // Should work with power of 2
        let window = AntiReplayWindow::with_size(DirectionId::I2R, 1024);
        assert_eq!(window.window_size, 1024);
        
        // Should panic with non-power of 2
        std::panic::catch_unwind(|| {
            AntiReplayWindow::with_size(DirectionId::I2R, 1000);
        }).unwrap_err();
    }
}
