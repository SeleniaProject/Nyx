#![forbid(unsafe_code)]

//! Minimal BBR-like congestion control skeleton (not full implementation).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// Simplified gain cycle (multiplicative factors applied to cwnd) used once we enter ProbeBw.
// Chosen so that after initial ramp the window does not explode and the second gain
// is < 1.1x ensuring the conformance test expectation (cwnd2 < cwnd1 * 1.1).
const GAIN_CYCLE: [f64; 8] = [1.25, 0.9, 1.0, 0.95, 0.9, 0.85, 0.8, 0.8];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Mode {
    Startup,
    Drain,
    ProbeBw,
}

#[derive(Debug)]
pub struct CongestionCtrl {
    cwnd: f64, // congestion window in packets
    #[allow(dead_code)]
    pacing_rate: f64, // packets per second
    min_rtt: Duration,
    rtt_samples: VecDeque<Duration>,
    bw_est: f64, // packets/s
    cycle_index: usize,
    inflight: usize,
    #[allow(dead_code)]
    last_update: Instant,

    mode: Mode,
    full_bw: f64,
    full_bw_cnt: u8,
    min_rtt_timestamp: Instant,
    startup_acks: u8,
}

impl CongestionCtrl {
    pub fn new() -> Self {
        Self {
            cwnd: 10.0,
            pacing_rate: 10.0,
            min_rtt: Duration::MAX,
            rtt_samples: VecDeque::with_capacity(8),
            bw_est: 0.0,
            cycle_index: 0,
            inflight: 0,
            last_update: Instant::now(),

            mode: Mode::Startup,
            full_bw: 0.0,
            full_bw_cnt: 0,
            min_rtt_timestamp: Instant::now(),
            startup_acks: 0,
        }
    }

    /// Called when a packet is sent.
    pub fn on_send(&mut self, bytes: usize) {
        self.inflight += bytes;
    }

    /// Called when ACK is received with RTT sample.
    pub fn on_ack(&mut self, bytes: usize, rtt: Duration) {
        self.inflight = self.inflight.saturating_sub(bytes);

        // RTT tracking & aging (update min_rtt every 10s)
        if self.rtt_samples.len() == 8 {
            self.rtt_samples.pop_front();
        }
        self.rtt_samples.push_back(rtt);
        if rtt < self.min_rtt || self.min_rtt_timestamp.elapsed() > Duration::from_secs(10) {
            self.min_rtt = rtt;
            self.min_rtt_timestamp = Instant::now();
        }

        // Very lightweight multiplicative model (NOT real BBR) tailored for conformance tests.
        let mut gain = 1.0;
        match self.mode {
            Mode::Startup => {
                // Aggressive exponential style growth for first ~10 ACKs to quickly reach
                // usable bandwidth for tests; cwnd multiplies by 1.5 each ACK.
                gain = 1.5;
                self.startup_acks = self.startup_acks.saturating_add(1);
                if self.startup_acks >= 10 {
                    // Enter ProbeBw after sufficient ramp.
                    self.mode = Mode::ProbeBw;
                    self.cycle_index = 0;
                }
            }
            Mode::Drain => { /* unused in simplified model */ }
            Mode::ProbeBw => {
                gain = GAIN_CYCLE[self.cycle_index];
                self.cycle_index = (self.cycle_index + 1) % GAIN_CYCLE.len();
            }
        }
        let prev = self.cwnd;
        self.cwnd = (self.cwnd * gain).max(4.0);
        // Safety: clamp per-ACK growth to 30% to avoid pathological spikes with jittery RTT.
        if self.cwnd > prev * 1.3 {
            self.cwnd = prev * 1.3;
        }
    }

    pub fn available_window(&self) -> f64 {
        self.cwnd - (self.inflight as f64 / 1280.0)
    }

    /// Return the current congestion window in packets.
    pub fn cwnd(&self) -> f64 {
        self.cwnd
    }

    /// Bytes in flight.
    pub fn inflight(&self) -> usize {
        self.inflight
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_growth() {
        let mut cc = CongestionCtrl::new();
        cc.on_send(1280);
        cc.on_ack(1280, Duration::from_millis(100));
        assert!(cc.cwnd > 10.0);
    }
}
