use std::time::{Duration, Instant};

/// Exponentially Weighted Moving Average for f64 values.
#[derive(Debug, Clone)]
pub struct Ewma {
	alpha: f64,
	value: Option<f64>,
}

impl Ewma {
	pub fn new(alpha: f64) -> Self { Self { alpha, value: None } }
	pub fn update(&mut self, x: f64) {
		self.value = Some(match self.value { Some(v) => self.alpha * x + (1.0 - self.alpha) * v, None => x });
	}
	pub fn get(&self) -> Option<f64> { self.value }
}

/// Token-bucket rate limiter (single-thread use; wrap in Mutex for shared use).
#[derive(Debug, Clone)]
pub struct RateLimiter {
	capacity: f64,
	tokens: f64,
	refill_per_sec: f64,
	last: Instant,
}

impl RateLimiter {
	pub fn new(capacity: f64, refill_per_sec: f64) -> Self {
		Self { capacity, tokens: capacity, refill_per_sec, last: Instant::now() }
	}
	fn refill(&mut self) {
		let now = Instant::now();
		let dt = now.duration_since(self.last).as_secs_f64();
		self.last = now;
		self.tokens = (self.tokens + dt * self.refill_per_sec).min(self.capacity);
	}
	/// Try to consume one token. Returns whether allowed now.
	pub fn allow(&mut self) -> bool {
		self.refill();
		if self.tokens >= 1.0 { self.tokens -= 1.0; true } else { false }
	}
	/// Wait until allowed or timeout; returns true if allowed.
	pub fn wait_until_allowed(&mut self, timeout: Duration) -> bool {
		let start = Instant::now();
		while !self.allow() {
			if start.elapsed() >= timeout { return false; }
			std::thread::sleep(Duration::from_millis(1));
		}
		true
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn ewma_behaves() {
		let mut e = Ewma::new(0.5);
		e.update(10.0);
		assert_eq!(e.get(), Some(10.0));
		e.update(0.0);
		assert_eq!(e.get().unwrap(), 5.0);
	}
	#[test]
	fn rate_limiter_allows_and_blocks() {
		let mut rl = RateLimiter::new(1.0, 2.0); // 2 tokens per sec
		assert!(rl.allow());
		assert!(!rl.allow());
		// Should allow within ~500ms
		let ok = rl.wait_until_allowed(Duration::from_millis(700));
		assert!(ok);
	}
}
