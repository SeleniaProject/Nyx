#![forbid(unsafe_code)]

use nyx_stream::multipath::{BufferedPacket, PathStats, ReorderingBuffer, MAX_HOPS, MIN_HOPS};
use std::time::{Duration, Instant};

fn mk_packet(path_id: u8, seq: u64, age_ms: u64) -> BufferedPacket {
    BufferedPacket {
        sequence: seq,
        path_id,
        data: vec![seq as u8],
        received_at: Instant::now() - Duration::from_millis(age_ms),
    }
}

#[test]
fn reordering_inorder_and_out_of_order() {
    let mut buf = ReorderingBuffer::new(1);

    // Insert out-of-order first (seq 1) -> nothing ready yet
    let ready = buf.insert_packet(mk_packet(1, 1, 0));
    assert!(ready.is_empty());

    // Insert expected (seq 0) -> should flush 0 and then 1
    let ready = buf.insert_packet(mk_packet(1, 0, 0));
    let payloads: Vec<Vec<u8>> = ready.into_iter().map(|p| p.data).collect();
    assert_eq!(payloads, vec![vec![0], vec![1]]);

    // Next expected is 2 now
    let (len, next) = buf.stats();
    assert_eq!(len, 0);
    assert_eq!(next, 2);
}

#[test]
fn reordering_expire_packets_by_timeout() {
    let mut buf = ReorderingBuffer::new(2);
    buf.next_expected = 10;

    // Two future packets buffered, one old enough to expire
    buf.insert_packet(mk_packet(2, 12, 250)); // older
    buf.insert_packet(mk_packet(2, 11, 50)); // newer

    // Expire with 200ms timeout -> only the first should expire
    let expired = buf.expire_packets(Duration::from_millis(200));
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0].sequence, 12);

    // Remaining buffered count should be 1
    assert_eq!(buf.buffer.len(), 1);
}

#[test]
fn reordering_max_size_enforced() {
    let mut buf = ReorderingBuffer::new(3);
    buf.max_size = 3;
    buf.next_expected = 0;

    // Insert 4 future packets -> last one should be dropped due to capacity
    for s in 1..=4 {
        let _ = buf.insert_packet(mk_packet(3, s, 0));
    }
    assert_eq!(buf.buffer.len(), 3);
}

#[test]
fn dynamic_hop_adjustment_bounds() {
    let mut stats = PathStats::new(1);
    // Start from default (5), increase due to bad conditions
    stats.loss_rate = 0.2; // high loss
    stats.rtt = Duration::from_millis(800);
    stats.adjust_hop_count();
    assert!(stats.hop_count <= MAX_HOPS);

    // Improve conditions repeatedly -> should not go below MIN_HOPS
    stats.loss_rate = 0.0;
    stats.rtt = Duration::from_millis(40);
    for _ in 0..10 {
        stats.adjust_hop_count();
    }
    assert!(stats.hop_count >= MIN_HOPS);
}

#[test]
fn optimal_hops_correlates_with_rtt_and_loss() {
    let mut stats = PathStats::new(2);

    stats.loss_rate = 0.0;
    stats.rtt = Duration::from_millis(30);
    let fast_low_loss = stats.calculate_optimal_hops();

    stats.loss_rate = 0.06; // higher loss
    stats.rtt = Duration::from_millis(300);
    let slow_high_loss = stats.calculate_optimal_hops();

    assert!(slow_high_loss >= fast_low_loss);
    assert!(slow_high_loss <= MAX_HOPS);
    assert!(fast_low_loss >= MIN_HOPS);
}
