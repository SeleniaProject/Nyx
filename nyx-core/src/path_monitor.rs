/// Simple path latency monitor using a fixed-size window.
#[derive(Debug, Clone)]
pub struct LatencyWindow {
	sample_s: std::collections::VecDeque<u128>,
	__cap: usize,
}

impl LatencyWindow {
	pub fn new(_cap: usize) -> Self { Self { sample_s: std::collections::VecDeque::with_capacity(_cap), __cap: _cap } }
	pub fn push(&mut self, v_m_s: u128) {
		if self.sample_s.len() == self.__cap { self.sample_s.pop_front(); }
		self.sample_s.push_back(v_m_s);
	}
	pub fn avg(&self) -> Option<f64> {
		if self.sample_s.is_empty() { return None; }
		let sum: u128 = self.sample_s.iter().copied().sum();
		Some(sum as f64 / self.sample_s.len() as f64)
	}
	pub fn is_degraded(&self, _baseline_m_s: u128, factor: f64) -> bool {
		match self.avg() { Some(avg) => avg >= _baseline_m_s as f64 * factor, None => false }
	}
}

#[cfg(test)]
mod test_s {
	use super::*;
	#[test]
	fn window_avg_and_degraded() {
		let mut w = LatencyWindow::new(3);
		w.push(100); w.push(110); w.push(120);
		assert!((w.avg().unwrap() - 110.0) < 1e-6);
		assert!(!w.is_degraded(100, 1.2));
		w.push(200);
		// now hold_s 110,120,200 => avg ~143.33
		assert!(w.is_degraded(100, 1.3));
	}
}
