#![forbid(unsafe_code)]

use std::collections::BTreeSet;

/// Simple flow controller supporting dynamic window and selective acknowledgment tracking.
#[derive(Debug, Clone)]
pub struct FlowController {
	base: u64,                 // next expected ack base (lowest unacked seq)
	cwnd: usize,               // current congestion/flow window (max in-flight frames)
	max_cwnd: usize,           // hard cap for cwnd
	sacked: BTreeSet<u64>,     // selectively acked sequences beyond base
}

impl FlowController {
	pub fn new(initial_cwnd: usize, max_cwnd: usize) -> Self {
		Self { base: 1, cwnd: initial_cwnd.max(1), max_cwnd: max_cwnd.max(1), sacked: BTreeSet::new() }
	}

	/// Whether sender may send more based on in-flight count.
	pub fn can_send(&self, inflight: usize) -> bool { inflight < self.cwnd }

	/// Called when an ACK for `seq` is received. Advances base and grows window (additive).
	pub fn on_ack(&mut self, seq: u64) {
		if seq < self.base { return; }
		self.sacked.insert(seq);
		// advance base while contiguous
		while self.sacked.remove(&self.base) { self.base += 1; }
		// grow cwnd additively up to cap
		if self.cwnd < self.max_cwnd { self.cwnd += 1; }
	}

	/// Called on timeout/loss indication to shrink window (multiplicative decrease).
	pub fn on_loss(&mut self) {
		self.cwnd = (self.cwnd / 2).max(1);
	}

	/// Called when a NACK/duplicate ACK observed for `seq` to prioritize retransmit.
	pub fn should_retransmit(&self, seq: u64, retries: u32) -> bool {
		// If we haven't seen ack up to seq and retries > 0, consider retransmit.
		retries > 0 && seq >= self.base && !self.sacked.contains(&seq)
	}

	pub fn cwnd(&self) -> usize { self.cwnd }
	pub fn base(&self) -> u64 { self.base }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn ack_advances_and_grows() {
		let mut fc = FlowController::new(2, 10);
		assert!(fc.can_send(1));
		fc.on_ack(1);
		assert_eq!(fc.base(), 2);
		assert!(fc.cwnd() >= 3);
		fc.on_ack(3); // out-of-order ack beyond base
		assert_eq!(fc.base(), 2); // base unchanged until 2 is acked
		fc.on_ack(2);
		assert_eq!(fc.base(), 4); // collapses contiguous 2..3
	}

	#[test]
	fn loss_halves_window() {
		let mut fc = FlowController::new(8, 100);
		fc.on_loss();
		assert_eq!(fc.cwnd(), 4);
		fc.on_loss();
		assert_eq!(fc.cwnd(), 2);
		fc.on_loss();
		assert_eq!(fc.cwnd(), 1);
		fc.on_loss();
		assert_eq!(fc.cwnd(), 1);
	}
}

