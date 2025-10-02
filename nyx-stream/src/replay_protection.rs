//! Anti-replay protection for Nyx Protocol
//!
//! Implements sliding window anti-replay protection as specified in Nyx Protocol v1.0 §2.1:
//! - Receivers MUST maintain a sliding window of size 2^20 for per-direction nonces
//! - Frames outside the window or already seen MUST be rejected with a replay error
//! - On rekey, nonces reset to zero; the anti-replay window MUST be reset accordingly
//! - Direction Identifier: Each half-duplex direction uses a distinct 32-bit direction identifier
//!
//! This module provides a high-performance bitmap-based sliding window implementation
//! suitable for high-throughput stream processing.

use crate::errors::{Error, Result};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Window size as specified: 2^20 = 1,048,576 nonces
pub const WINDOW_SIZE: u64 = 1 << 20;

/// Maximum allowed gap between expected and received nonce before rejection
/// This prevents memory exhaustion attacks from wildly out-of-order nonces
const MAX_NONCE_GAP: u64 = WINDOW_SIZE / 2;

/// Anti-replay window for a single direction
///
/// Uses a sliding bitmap window to track seen nonces efficiently.
/// Memory usage: ~131 KB per direction (2^20 bits / 8)
#[derive(Debug)]
pub struct ReplayWindow {
    /// Highest nonce value successfully received and accepted
    highest_nonce: u64,
    
    /// Bitmap tracking seen nonces within [highest_nonce - WINDOW_SIZE + 1, highest_nonce]
    /// Uses VecDeque for efficient sliding as window advances
    /// Each u64 stores 64 bits, so we need WINDOW_SIZE / 64 = 16,384 u64s
    bitmap: VecDeque<u64>,
    
    /// Statistics: total frames accepted
    accepted_count: u64,
    
    /// Statistics: total frames rejected due to replay
    replay_rejected_count: u64,
    
    /// Statistics: total frames rejected due to being too old (outside window)
    too_old_rejected_count: u64,
}

impl ReplayWindow {
    /// Create a new replay window
    ///
    /// Initially accepts any nonce >= 0, and starts building the window from the first received nonce.
    pub fn new() -> Self {
        let bitmap_size = (WINDOW_SIZE / 64) as usize;
        Self {
            highest_nonce: 0,
            bitmap: VecDeque::with_capacity(bitmap_size),
            accepted_count: 0,
            replay_rejected_count: 0,
            too_old_rejected_count: 0,
        }
    }
    
    /// Check if a nonce has been seen and mark it if not
    ///
    /// Returns:
    /// - `Ok(())` if nonce is valid and not seen before (now marked as seen)
    /// - `Err(Error::ReplayDetected)` if nonce was already seen
    /// - `Err(Error::NonceTooOld)` if nonce is outside the valid window
    pub fn check_and_update(&mut self, nonce: u64) -> Result<()> {
        // Handle first frame specially
        if self.accepted_count == 0 {
            self.highest_nonce = nonce;
            self.mark_seen(nonce);
            self.accepted_count += 1;
            return Ok(());
        }
        
        // Check if nonce is in the future (ahead of highest)
        if nonce > self.highest_nonce {
            // Reject if gap is too large (potential attack or severe reordering)
            if nonce - self.highest_nonce > MAX_NONCE_GAP {
                return Err(Error::InvalidFrame(format!(
                    "Nonce {} too far ahead of current window (highest: {})",
                    nonce, self.highest_nonce
                )));
            }
            
            // Advance window to include this nonce
            self.advance_window(nonce);
            self.mark_seen(nonce);
            self.accepted_count += 1;
            return Ok(());
        }
        
        // Nonce is in the past - check if it's within the valid window
        let window_start = self.highest_nonce.saturating_sub(WINDOW_SIZE - 1);
        
        if nonce < window_start {
            // Too old - outside the valid window
            self.too_old_rejected_count += 1;
            return Err(Error::InvalidFrame(format!(
                "Nonce {} too old (window: [{}, {}])",
                nonce, window_start, self.highest_nonce
            )));
        }
        
        // Within window - check if already seen
        if self.is_seen(nonce) {
            self.replay_rejected_count += 1;
            return Err(Error::InvalidFrame(format!("Replay detected for nonce {}", nonce)));
        }
        
        // Valid and not seen - mark as seen
        self.mark_seen(nonce);
        self.accepted_count += 1;
        Ok(())
    }
    
    /// Advance the window to include a new highest nonce
    fn advance_window(&mut self, new_highest: u64) {
        assert!(new_highest > self.highest_nonce);
        
        let shift = new_highest - self.highest_nonce;
        self.highest_nonce = new_highest;
        
        // If shift is larger than window, clear everything
        if shift >= WINDOW_SIZE {
            self.bitmap.clear();
            return;
        }
        
        // Shift bitmap by 'shift' bits
        // For simplicity, we shift in units of u64 (64 bits)
        let full_shifts = (shift / 64) as usize;
        let partial_shift = (shift % 64) as u32;
        
        // Remove old entries
        for _ in 0..full_shifts {
            if !self.bitmap.is_empty() {
                self.bitmap.pop_front();
            }
        }
        
        // Handle partial shift
        if partial_shift > 0 && !self.bitmap.is_empty() {
            let mut carry = 0u64;
            for slot in self.bitmap.iter_mut().rev() {
                let new_carry = *slot >> (64 - partial_shift);
                *slot = (*slot << partial_shift) | carry;
                carry = new_carry;
            }
        }
        
        // Ensure bitmap has correct size
        let expected_size = (WINDOW_SIZE / 64) as usize;
        while self.bitmap.len() < expected_size {
            self.bitmap.push_back(0);
        }
    }
    
    /// Check if a nonce within the current window has been seen
    fn is_seen(&self, nonce: u64) -> bool {
        if nonce > self.highest_nonce {
            return false;
        }
        
        let window_start = self.highest_nonce.saturating_sub(WINDOW_SIZE - 1);
        if nonce < window_start {
            return false;
        }
        
        // Calculate position in bitmap
        let offset = self.highest_nonce - nonce;
        let slot_index = (offset / 64) as usize;
        let bit_index = (offset % 64) as u32;
        
        if slot_index >= self.bitmap.len() {
            return false;
        }
        
        let slot = self.bitmap[slot_index];
        (slot & (1u64 << bit_index)) != 0
    }
    
    /// Mark a nonce as seen in the bitmap
    fn mark_seen(&mut self, nonce: u64) {
        if nonce > self.highest_nonce {
            return; // Should be advanced first
        }
        
        // Ensure bitmap is initialized
        let expected_size = (WINDOW_SIZE / 64) as usize;
        while self.bitmap.len() < expected_size {
            self.bitmap.push_back(0);
        }
        
        let window_start = self.highest_nonce.saturating_sub(WINDOW_SIZE - 1);
        if nonce < window_start {
            return; // Outside window
        }
        
        let offset = self.highest_nonce - nonce;
        let slot_index = (offset / 64) as usize;
        let bit_index = (offset % 64) as u32;
        
        if slot_index < self.bitmap.len() {
            self.bitmap[slot_index] |= 1u64 << bit_index;
        }
    }
    
    /// Reset the window (used after rekey)
    ///
    /// As per spec: "On rekey, nonces reset to zero; the anti-replay window MUST be reset"
    pub fn reset(&mut self) {
        self.highest_nonce = 0;
        self.bitmap.clear();
        self.accepted_count = 0;
        // Keep rejection counters for diagnostics
    }
    
    /// Get statistics for telemetry
    pub fn stats(&self) -> ReplayWindowStats {
        ReplayWindowStats {
            accepted_count: self.accepted_count,
            replay_rejected_count: self.replay_rejected_count,
            too_old_rejected_count: self.too_old_rejected_count,
            highest_nonce: self.highest_nonce,
        }
    }
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for replay window telemetry
#[derive(Debug, Clone, Copy)]
pub struct ReplayWindowStats {
    pub accepted_count: u64,
    pub replay_rejected_count: u64,
    pub too_old_rejected_count: u64,
    pub highest_nonce: u64,
}

/// Per-direction replay protection manager
///
/// Manages separate replay windows for each traffic direction as required by spec.
#[derive(Debug, Clone)]
pub struct DirectionalReplayProtection {
    /// Replay window for initiator → responder direction
    initiator_to_responder: Arc<RwLock<ReplayWindow>>,
    
    /// Replay window for responder → initiator direction
    responder_to_initiator: Arc<RwLock<ReplayWindow>>,
}

impl DirectionalReplayProtection {
    /// Create a new directional replay protection manager
    pub fn new() -> Self {
        Self {
            initiator_to_responder: Arc::new(RwLock::new(ReplayWindow::new())),
            responder_to_initiator: Arc::new(RwLock::new(ReplayWindow::new())),
        }
    }
    
    /// Check a nonce for the initiator → responder direction
    pub async fn check_initiator_to_responder(&self, nonce: u64) -> Result<()> {
        let mut window = self.initiator_to_responder.write().await;
        window.check_and_update(nonce)
    }
    
    /// Check a nonce for the responder → initiator direction
    pub async fn check_responder_to_initiator(&self, nonce: u64) -> Result<()> {
        let mut window = self.responder_to_initiator.write().await;
        window.check_and_update(nonce)
    }
    
    /// Reset both windows after rekey
    pub async fn reset_all(&self) {
        let mut init_window = self.initiator_to_responder.write().await;
        let mut resp_window = self.responder_to_initiator.write().await;
        init_window.reset();
        resp_window.reset();
    }
    
    /// Get statistics for both directions
    pub async fn stats(&self) -> (ReplayWindowStats, ReplayWindowStats) {
        let init_window = self.initiator_to_responder.read().await;
        let resp_window = self.responder_to_initiator.read().await;
        (init_window.stats(), resp_window.stats())
    }
}

impl Default for DirectionalReplayProtection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_first_nonce_accepted() {
        let mut window = ReplayWindow::new();
        assert!(window.check_and_update(0).is_ok());
        assert_eq!(window.accepted_count, 1);
        assert_eq!(window.highest_nonce, 0);
    }
    
    #[test]
    fn test_sequential_nonces_accepted() {
        let mut window = ReplayWindow::new();
        for i in 0..100 {
            assert!(window.check_and_update(i).is_ok(), "Failed at nonce {}", i);
        }
        assert_eq!(window.accepted_count, 100);
        assert_eq!(window.highest_nonce, 99);
    }
    
    #[test]
    fn test_replay_detected() {
        let mut window = ReplayWindow::new();
        assert!(window.check_and_update(10).is_ok());
        assert!(window.check_and_update(11).is_ok());
        
        // Try to replay nonce 10
        let result = window.check_and_update(10);
        assert!(result.is_err());
        assert_eq!(window.replay_rejected_count, 1);
    }
    
    #[test]
    fn test_out_of_order_within_window() {
        let mut window = ReplayWindow::new();
        assert!(window.check_and_update(100).is_ok());
        assert!(window.check_and_update(50).is_ok()); // Within window
        assert!(window.check_and_update(75).is_ok());
        assert!(window.check_and_update(99).is_ok());
        
        assert_eq!(window.accepted_count, 4);
    }
    
    #[test]
    fn test_too_old_rejected() {
        let mut window = ReplayWindow::new();
        assert!(window.check_and_update(WINDOW_SIZE + 100).is_ok());
        
        // Try nonce 0 - should be too old (outside window)
        let result = window.check_and_update(0);
        assert!(result.is_err());
        assert_eq!(window.too_old_rejected_count, 1);
    }
    
    #[test]
    fn test_window_advancement() {
        let mut window = ReplayWindow::new();
        
        // Start with nonce 1000
        assert!(window.check_and_update(1000).is_ok());
        
        // Jump ahead by 1000
        assert!(window.check_and_update(2000).is_ok());
        
        // Nonces within [1000, 2000] should still be valid
        assert!(window.check_and_update(1500).is_ok());
        assert!(window.check_and_update(1999).is_ok());
    }
    
    #[test]
    fn test_reset_clears_window() {
        let mut window = ReplayWindow::new();
        
        for i in 0..100 {
            assert!(window.check_and_update(i).is_ok());
        }
        
        window.reset();
        
        assert_eq!(window.highest_nonce, 0);
        assert_eq!(window.accepted_count, 0);
        
        // Should be able to reuse nonces after reset
        assert!(window.check_and_update(50).is_ok());
    }
    
    #[test]
    fn test_large_gap_rejected() {
        let mut window = ReplayWindow::new();
        assert!(window.check_and_update(100).is_ok());
        
        // Try to jump too far ahead (> MAX_NONCE_GAP)
        let result = window.check_and_update(100 + MAX_NONCE_GAP + 1);
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_directional_protection() {
        let protection = DirectionalReplayProtection::new();
        
        // Test initiator → responder
        assert!(protection.check_initiator_to_responder(0).await.is_ok());
        assert!(protection.check_initiator_to_responder(1).await.is_ok());
        assert!(protection.check_initiator_to_responder(0).await.is_err()); // Replay
        
        // Test responder → initiator (independent window)
        assert!(protection.check_responder_to_initiator(0).await.is_ok());
        assert!(protection.check_responder_to_initiator(1).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_reset_all_directions() {
        let protection = DirectionalReplayProtection::new();
        
        assert!(protection.check_initiator_to_responder(100).await.is_ok());
        assert!(protection.check_responder_to_initiator(200).await.is_ok());
        
        protection.reset_all().await;
        
        // After reset, should accept nonces from beginning again
        assert!(protection.check_initiator_to_responder(0).await.is_ok());
        assert!(protection.check_responder_to_initiator(0).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_statistics() {
        let protection = DirectionalReplayProtection::new();
        
        for i in 0..10 {
            let _ = protection.check_initiator_to_responder(i).await;
        }
        
        // Try some replays
        let _ = protection.check_initiator_to_responder(5).await;
        let _ = protection.check_initiator_to_responder(7).await;
        
        let (init_stats, _resp_stats) = protection.stats().await;
        assert_eq!(init_stats.accepted_count, 10);
        assert_eq!(init_stats.replay_rejected_count, 2);
        assert_eq!(init_stats.highest_nonce, 9);
    }
}
