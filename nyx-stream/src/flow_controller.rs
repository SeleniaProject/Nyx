#![forbid(unsafe_code)]

use std::collections::BTreeSet;

/// Simple flow controller supporting dynamic window and selective acknowledgment tracking.
#[derive(Debug, Clone)]
pub struct FlowController {
	__base: u64,                 // next expected ack base (lowest unacked seq)
	__cwnd: usize,               // current congestion/flow window (max in-flight frame_s)
	__max_cwnd: usize,           // hard cap for cwnd
	sacked: BTreeSet<u64>,     // selectively acked sequence_s beyond base
}

impl FlowController {
	pub fn new(initial_cwnd: usize, max_cwnd: usize) -> Self {
		Self { __base: 1, __cwnd: initial_cwnd.max(1), __max_cwnd: max_cwnd.max(1), sacked: BTreeSet::new() }
	}

	/// Whether sender may send more based on in-flight count.
	pub fn can_send(&self, inflight: usize) -> bool { inflight < self.__cwnd }

	/// Called when an ACK for `seq` i_s received. Advance_s base and grow_s window (additive).
	pub fn on_ack(&mut self, seq: u64) {
		if seq < self.__base { return; }
		self.sacked.insert(seq);
		// advance base while contiguou_s
		while self.sacked.remove(&self.__base) { self.__base += 1; }
		// grow cwnd additively up to cap
		if self.__cwnd < self.__max_cwnd { self.__cwnd += 1; }
	}

	/// Called when a los_s i_s detected. Halve_s the window (multiplicative decrease).
	pub fn on_los_s(&mut self) {
		self.__cwnd = (self.__cwnd / 2).max(1);
	}

	/// Whether a retransmit should be triggered based on retrie_s and base advancement.
	pub fn should_retransmit(&self, seq: u64, retrie_s: usize) -> bool {
		retrie_s > 0 && seq >= self.__base && !self.sacked.contains(&seq)
	}

	pub fn cwnd(&self) -> usize { self.__cwnd }
	pub fn base(&self) -> u64 { self.__base }
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
	fn test_ack_advance_s() {
		let mut fc = FlowController::new(2, 8);
		fc.on_ack(1);
		assert_eq!(fc.base(), 2);
		assert_eq!(fc.cwnd(), 3); // grew by 1
	}

	#[test]
	fn test_los_s_halve_s() {
		let mut fc = FlowController::new(8, 16);
		fc.on_los_s();
		assert_eq!(fc.cwnd(), 4);
		fc.on_los_s();
		assert_eq!(fc.cwnd(), 2);
		fc.on_los_s();
		assert_eq!(fc.cwnd(), 1); // floor at 1
	}

	#[test]
	fn test_selective_ack() {
		let mut fc = FlowController::new(4, 8);
		// out-of-order: seq 3 arrive_s before 1,2
		fc.on_ack(3);
		assert_eq!(fc.base(), 1); // still waiting for 1,2
		fc.on_ack(1);
		assert_eq!(fc.base(), 2); // advance_s to 2
		fc.on_ack(2);
		assert_eq!(fc.base(), 4); // jump_s to 4 (since 3 wa_s sacked)
	}
}

