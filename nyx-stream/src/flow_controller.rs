#![forbid(unsafe_code)]

use std::collections::HashSet;

/// Simple flow controller supporting dynamic window and selective acknowledgment tracking.
#[derive(Debug, Clone)]
pub struct FlowController {
    base: u64,            // next expected ack base (lowest unacked seq)
    cwnd: usize,          // current congestion/flow window (max in-flight frames)
    max_cwnd: usize,      // hard cap for cwnd
    sacked: HashSet<u64>, // Ultra-high performance: HashSet for O(1) lookups instead of O(log n)
    // Pre-allocated buffer for sequence ranges to reduce allocations
    seq_buffer: Vec<u64>,
}

impl FlowController {
    pub fn new(initial_cwnd: usize, max_cwnd: usize) -> Self {
        Self {
            base: 1,
            cwnd: initial_cwnd.max(1),
            max_cwnd: max_cwnd.max(1),
            sacked: HashSet::with_capacity(64), // Pre-allocate for better performance
            seq_buffer: Vec::with_capacity(32), // Pre-allocated buffer
        }
    }

    /// Whether sender may send more based on in-flight count.
    #[inline(always)]
    pub fn can_send(&self, inflight: usize) -> bool {
        inflight < self.cwnd
    }

    /// Called when an ACK for `seq` is received. Advances base and grows window (additive).
    #[inline(always)]
    pub fn on_ack(&mut self, seq: u64) {
        if seq < self.base {
            return;
        }

        // Ultra-high performance: direct HashSet operations without repeated remove/insert
        self.sacked.insert(seq);

        // Optimized base advancement: collect contiguous sequences first
        // Reuse vector capacity to minimize allocations
        self.seq_buffer.clear();
        let mut current = self.base;

        // Collect all contiguous acknowledged sequences
        while self.sacked.contains(&current) {
            self.seq_buffer.push(current);
            current += 1;
        }

        // Remove all contiguous sequences at once for better cache performance
        if !self.seq_buffer.is_empty() {
            // Use retain to avoid multiple hash lookups
            self.sacked.retain(|&s| s < current);
            self.base = current;
        }

        // Grow cwnd additively up to cap
        if self.cwnd < self.max_cwnd {
            self.cwnd += 1;
        }
    }

    /// Called when a loss is detected. Halves the window (multiplicative decrease).
    #[inline(always)]
    pub fn on_loss(&mut self) {
        self.cwnd = (self.cwnd / 2).max(1);
    }

    /// Whether a retransmit should be triggered based on retries and base advancement.
    #[inline(always)]
    pub fn should_retransmit(&self, seq: u64, retries: usize) -> bool {
        retries > 0 && seq >= self.base && !self.sacked.contains(&seq)
    }

    #[inline(always)]
    pub fn cwnd(&self) -> usize {
        self.cwnd
    }

    #[inline(always)]
    pub fn base(&self) -> u64 {
        self.base
    }
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn test_basic_flow() {
        let mut fc = FlowController::new(4, 8);
        assert_eq!(fc.cwnd(), 4);
        assert_eq!(fc.base(), 1);
        assert!(fc.can_send(3));
        assert!(!fc.can_send(4));
    }

    #[test]
    fn test_ack_advances() {
        let mut fc = FlowController::new(2, 8);
        fc.on_ack(1);
        assert_eq!(fc.base(), 2);
        assert_eq!(fc.cwnd(), 3); // grew by 1
    }

    #[test]
    fn test_loss_halves() {
        let mut fc = FlowController::new(8, 16);
        fc.on_loss();
        assert_eq!(fc.cwnd(), 4);
        fc.on_loss();
        assert_eq!(fc.cwnd(), 2);
        fc.on_loss();
        assert_eq!(fc.cwnd(), 1); // floor at 1
    }

    #[test]
    fn test_selective_ack() {
        let mut fc = FlowController::new(4, 8);
        // out-of-order: seq 3 arrives before 1,2
        fc.on_ack(3);
        assert_eq!(fc.base(), 1); // still waiting for 1,2
        fc.on_ack(1);
        assert_eq!(fc.base(), 2); // advances to 2
        fc.on_ack(2);
        assert_eq!(fc.base(), 4); // jumps to 4 (since 3 was sacked)
    }
}
