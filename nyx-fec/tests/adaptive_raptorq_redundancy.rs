#![cfg(feature = "raptorq")]

use nyx_fec::raptorq::{adaptive_raptorq_redundancy, Redundancy};

#[test]
fn adaptive_raptorq_redundancy_adjusts_both_directions() {
    let prev = Redundancy { tx: 0.1, rx: 0.1 };
    let next = adaptive_raptorq_redundancy(150, 0.2, prev);
    assert!(next.tx > prev.tx);
    assert!(next.rx > prev.rx);
}
