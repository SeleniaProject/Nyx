#![forbid(unsafe_code)]

use nyx_stream::multipath::{MultipathManager, MultipathConfig};
use nyx_core::types::PathId;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone)]
struct SimPkt {
    path: PathId,
    seq: u64,
    delay_ms: u64,
    data: Vec<u8>,
}

fn encode(path: PathId, seq: u64, payload: u8) -> Vec<u8> {
    let mut v = Vec::with_capacity(1 + 8 + 1);
    v.push(path);
    v.extend_from_slice(&seq.to_le_bytes());
    v.push(payload);
    v
}

fn decode(buf: &[u8]) -> (PathId, u64, u8) {
    let path = buf[0];
    let mut arr = [0u8; 8];
    arr.copy_from_slice(&buf[1..9]);
    let seq = u64::from_le_bytes(arr);
    let payload = buf[9];
    (path, seq, payload)
}

#[test]
fn multipath_end_to_end_reassembly_per_path_inorder() {
    let mut mgr = MultipathManager::new(MultipathConfig::default());

    // Setup 3 paths with different RTTs: 10ms, 20ms, 100ms
    for pid in [1u8, 2u8, 3u8] {
        mgr.add_path(pid, 0).unwrap();
    }
    mgr.update_path_rtt(1, Duration::from_millis(10));
    mgr.update_path_rtt(2, Duration::from_millis(20));
    mgr.update_path_rtt(3, Duration::from_millis(100));

    // Generate scheduled sends, assign per-path sequences, and synthetic delays
    const N: usize = 600;
    let mut seq_per_path: HashMap<PathId, u64> = HashMap::new();
    let base_delay: HashMap<PathId, u64> = [(1, 5u64), (2, 10u64), (3, 50u64)].into_iter().collect();
    let mut inflight: Vec<SimPkt> = Vec::with_capacity(N);

    for i in 0..N {
        let payload = (i % 256) as u8;
        let sent = mgr.send_data(vec![payload]).expect("no path selected");
        let path = sent.path_id;
        let seq = seq_per_path.entry(path).or_insert(0);
        // jitter pattern to cause out-of-order arrivals within each path
        let jitter = ((i % 7) * 3) as u64; // 0,3,6,9,12,15,18ms
        let delay_ms = base_delay[&path] + jitter;
        let data = encode(path, *seq, payload);
        inflight.push(SimPkt { path, seq: *seq, delay_ms, data });
        *seq += 1;
    }

    // Sort by delay to simulate arrival order across all paths
    inflight.sort_by_key(|p| p.delay_ms);

    // Track last delivered sequence per path to assert in-order delivery per path
    let mut last_seq_delivered: HashMap<PathId, i64> = HashMap::new();
    let mut delivered_count: HashMap<PathId, u64> = HashMap::new();

    for pkt in inflight {
        let ready = mgr.receive_packet(pkt.path, pkt.seq, pkt.data.clone());
        for buf in ready {
            let (path, seq, _payload) = decode(&buf);
            let last = last_seq_delivered.get(&path).copied().unwrap_or(-1);
            assert_eq!(seq as i64, last + 1, "path {} delivered out-of-order: got seq {} after {}", path, seq, last);
            last_seq_delivered.insert(path, seq as i64);
            *delivered_count.entry(path).or_insert(0) += 1;
        }

        // Occasionally process timeouts (no-op given our short delays vs default timeout)
        let _ = mgr.process_timeouts();
    }

    // Verify we delivered all per-path packets
    for (path, total_sent) in seq_per_path {
        let delivered = delivered_count.get(&path).copied().unwrap_or(0);
        assert_eq!(delivered, total_sent, "path {}: delivered {} vs sent {}", path, delivered, total_sent);
    }

    // Spot-check hop_count bounds via a few more sends
    for _ in 0..10 {
        let sent = mgr.send_data(vec![0]).expect("no path");
        assert!((3..=7).contains(&sent.hop_count));
    }
}
