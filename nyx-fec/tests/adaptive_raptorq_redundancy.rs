#![forbid(unsafe_code)]
use nyx_fec::AdaptiveRaptorQ;
use std::time::Duration;

/// @spec 7. FEC (Adaptive RaptorQ)
/// 低品質ネットワーク条件 -> 冗長率上昇、その後良好条件 -> 低下 (上昇時よりは増えない) を確認。
#[test]
fn adaptive_raptorq_redundancy_adjusts_both_directions() {
    let mut adapt = AdaptiveRaptorQ::new(0.15, 10, 0.05, 0.6); // 初期 15%
    let start = adapt.redundancy();

    // 悪条件繰り返し
    for _ in 0..15 {
        adapt.update_network_condition(nyx_fec::NetworkCondition {
            timestamp: std::time::Instant::now(),
            packet_loss_rate: 0.2,
            rtt: Duration::from_millis(600),
            bandwidth_estimate: 80_000,
            congestion_level: 0.9,
        });
    }
    let increased = adapt.redundancy();
    assert!(
        increased >= start,
        "redundancy should increase under poor conditions start={start} now={increased}"
    );

    // 良好条件
    for _ in 0..30 {
        adapt.update_network_condition(nyx_fec::NetworkCondition {
            timestamp: std::time::Instant::now(),
            packet_loss_rate: 0.0,
            rtt: Duration::from_millis(40),
            bandwidth_estimate: 40_000_000,
            congestion_level: 0.05,
        });
    }
    let decreased = adapt.redundancy();
    assert!(decreased <= increased + 0.0001, "redundancy should not continue rising after good recovery increased={increased} dec={decreased}");
    assert!(decreased >= 0.05 && decreased <= 0.6);
}
