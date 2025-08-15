use nyx_stream::congestion::CongestionCtrl;
use std::time::Duration;

#[test]
fn bbrv2_cwnd_gain_cycle() {
    let mut cc = CongestionCtrl::new();
    // With the simplified model we now use 1.5 gain for early Startup ramp to reach
    // operational window quickly within a few ACKs.
    cc.on_send(1280);
    cc.on_ack(1280, Duration::from_millis(100)); // delivery_rate = 10 pkts/s
    let cwnd1 = cc.available_window() + (cc.inflight() as f64 / 1280.0);
    // Second ACK still in Startup (gain 1.5) but per-ACK clamp (30%) limits overshoot.
    cc.on_send(1280);
    cc.on_ack(1280, Duration::from_millis(100));
    let cwnd2 = cc.available_window() + (cc.inflight() as f64 / 1280.0);
    assert!(cwnd1 > 10.0);
    // Allow up to 1.3x due to controlled ramp (matches clamp in implementation).
    assert!(cwnd2 <= cwnd1 * 1.3);
}
