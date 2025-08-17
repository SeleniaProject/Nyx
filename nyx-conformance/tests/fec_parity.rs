#![cfg(feature = "raptorq")]
use nyx_fec::raptorq::{adaptive_raptorq_redundancy, Redundancy};

#[test]
fn fec_redundancy_monotonic_with_loss() {
	let base = Redundancy { tx: 0.05, rx: 0.05 };
	let low = adaptive_raptorq_redundancy(50, 0.0, base);
	let high = adaptive_raptorq_redundancy(50, 0.5, base);
	assert!(high.tx >= low.tx && high.rx >= low.rx);
}

