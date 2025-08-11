#![forbid(unsafe_code)]

use nyx_stream::multipath::{MultipathManager, MultipathConfig};
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn unhealthy_path_gets_minimal_share() {
    let mut mgr = MultipathManager::new(MultipathConfig::default());

    // Two paths
    mgr.add_path(1, 0).unwrap();
    mgr.add_path(2, 0).unwrap();

    // Equal RTT initially -> roughly equal share
    mgr.update_path_rtt(1, Duration::from_millis(50));
    mgr.update_path_rtt(2, Duration::from_millis(50));

    let mut counts: HashMap<u8, u32> = HashMap::new();
    for _ in 0..1000 {
        let pkt = mgr.send_data(vec![0]).expect("no path selected");
        *counts.entry(pkt.path_id).or_insert(0) += 1;
    }
    let total = 1000f64;
    let r1 = *counts.get(&1).unwrap_or(&0) as f64 / total;
    let r2 = *counts.get(&2).unwrap_or(&0) as f64 / total;
    assert!(r1 > 0.4 && r1 < 0.6, "baseline r1={r1}");
    assert!(r2 > 0.4 && r2 < 0.6, "baseline r2={r2}");

    // Make path 2 unhealthy (high loss and RTT)
    mgr.update_path_rtt(2, Duration::from_millis(500));
    mgr.update_path_loss(2, 0.6);

    // Recount selections
    counts.clear();
    for _ in 0..2100 {
        let pkt = mgr.send_data(vec![0]).expect("no path selected");
        *counts.entry(pkt.path_id).or_insert(0) += 1;
    }

    let total = 2100f64;
    let r1 = *counts.get(&1).unwrap_or(&0) as f64 / total;
    let r2 = *counts.get(&2).unwrap_or(&0) as f64 / total;

    // Unhealthy path should get only minimal share (<= ~1/(20:1) ~= 4.7%). Allow 10% max.
    assert!(r2 < 0.10, "unhealthy path ratio too high: r2={r2}, r1={r1}");
}
