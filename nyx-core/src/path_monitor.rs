/// Simple path latency monitor using a fixed-size window.
#[derive(Debug, Clone)]
pub struct LatencyWindow {
    samples: std::collections::VecDeque<u128>,
    cap: usize,
}

impl LatencyWindow {
    pub fn new(cap: usize) -> Self {
        Self {
            samples: std::collections::VecDeque::with_capacity(cap),
            cap,
        }
    }
    pub fn push(&mut self, v_ms: u128) {
        if self.samples.len() == self.cap {
            self.samples.pop_front();
        }
        self.samples.push_back(v_ms);
    }
    pub fn avg(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let sum: u128 = self.samples.iter().copied().sum();
        Some(sum as f64 / self.samples.len() as f64)
    }
    pub fn is_degraded(&self, baseline_ms: u128, factor: f64) -> bool {
        match self.avg() {
            Some(avg) => avg >= baseline_ms as f64 * factor,
            None => false,
        }
    }
}

#[cfg(test)]
mod test_s {
    use super::*;
    #[test]
    fn window_avg_and_degraded() {
        let mut w = LatencyWindow::new(3);
        w.push(100);
        w.push(110);
        w.push(120);
        assert!((w.avg().unwrap() - 110.0) < 1e-6);
        assert!(!w.is_degraded(100, 1.2));
        w.push(200);
        // now hold_s 110,120,200 => avg ~143.33
        assert!(w.is_degraded(100, 1.3));
    }
}
