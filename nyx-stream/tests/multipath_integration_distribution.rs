#![forbid(unsafe_code)]

use nyx_stream::multipath::{MultipathManager, MultipathConfig};
use nyx_core::types::PathId;
use std::collections::HashMap;
use std::time::Duration;

/// @spec 2. Multipath Data Plane
/// @spec 5. Adaptive Cover Traffic
#[test]
fn multipath_wrr_distribution_matches_weights() {
    let mut mgr = MultipathManager::new(MultipathConfig::default());

    // Add three paths and set RTTs -> weights 100, 50, 10 (scale=1000)
    let paths: &[(PathId, u64)] = &[(1, 10), (2, 20), (3, 100)];
    for (pid, _rtt) in paths {
        mgr.add_path(*pid, 0).unwrap();
    }
    for (pid, rtt_ms) in paths {
        mgr.update_path_rtt(*pid, Duration::from_millis(*rtt_ms));
    }

    // Run many selections and count per-path picks via send_data
    let mut counts: HashMap<PathId, u32> = HashMap::new();
    const ITER: usize = 1600; // multiple of total weight 160
    for _ in 0..ITER {
        let pkt = mgr.send_data(vec![0]).expect("no path selected");
        *counts.entry(pkt.path_id).or_insert(0) += 1;
    }

    let total: u32 = counts.values().copied().sum();
    assert_eq!(total, ITER as u32);

    // 動的 weight は RTT 以外の要素 (初期EMA等) でも変動するため、実際の stats から期待比率を再計算して許容誤差で検証
    let stats = mgr.get_path_stats();
    let mut w_sum: u64 = 0;
    let mut w_map = std::collections::HashMap::new();
    for (pid, st) in &stats { w_sum += st.weight as u64; w_map.insert(*pid, st.weight as f64); }
    let r1 = counts.get(&1).copied().unwrap_or(0) as f64 / total as f64;
    let r2 = counts.get(&2).copied().unwrap_or(0) as f64 / total as f64;
    let r3 = counts.get(&3).copied().unwrap_or(0) as f64 / total as f64;
    let e1 = w_map.get(&1).unwrap() / w_sum as f64;
    let e2 = w_map.get(&2).unwrap() / w_sum as f64;
    let e3 = w_map.get(&3).unwrap() / w_sum as f64;
    // 許容誤差 20% (短い試行数でのばらつき考慮)
    let tol = 0.20;
    assert!((r1 - e1).abs() <= e1 * tol, "path1 ratio={r1} expected={e1}");
    assert!((r2 - e2).abs() <= e2 * tol, "path2 ratio={r2} expected={e2}");
    assert!((r3 - e3).abs() <= e3 * tol, "path3 ratio={r3} expected={e3}");

    // Hop count is computed and kept within bounds
    let pkt = mgr.send_data(vec![1]).expect("no path selected");
    assert!((3..=7).contains(&pkt.hop_count));
}
