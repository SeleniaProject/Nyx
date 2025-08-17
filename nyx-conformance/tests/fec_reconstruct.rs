#![cfg(feature = "raptorq")]
use nyx_fec::raptorq::{adaptive_raptorq_redundancy, Redundancy};

#[test]
fn fec_zero_redundancy_no_change() {
	let base = Redundancy { tx: 0.0, rx: 0.0 };
	let next = adaptive_raptorq_redundancy(10, 0.0, base);
	assert_eq!(next.tx, 0.0);
	assert_eq!(next.rx, 0.0);
}

