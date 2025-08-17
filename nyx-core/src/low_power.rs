use crate::types::TimestampMs;
use crate::performance::RateLimiter;
use std::time::Duration;

/// Represents a screen/power state at a given time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenState { On, Off }

/// Compute the ratio of time the screen was Off within the observed interval.
/// The input must be a non-empty, time-ordered slice of (timestamp, state).
pub fn screen_off_ratio(events: &[(TimestampMs, ScreenState)]) -> f64 {
	// Empty input => no observation window
	if events.is_empty() { return 0.0; }
	// Avoid unwrap; if first/last are missing (shouldn't happen since not empty), return 0
	let Some(&(TimestampMs(start), _)) = events.first() else { return 0.0 };
	let Some(&(TimestampMs(end), _)) = events.last() else { return 0.0 };
	if end <= start { return 0.0; }
	let mut off_ms: u64 = 0;
	for w in events.windows(2) {
		let (t0, s0) = (w[0].0 .0, w[0].1);
		let (t1, _s1) = (w[1].0 .0, w[1].1);
		if s0 == ScreenState::Off && t1 > t0 { off_ms += t1 - t0; }
	}
	off_ms as f64 / (end - start) as f64
}

/// Triggers an action when inactivity exceeds a threshold, rate-limited.
#[derive(Debug)]
pub struct InactivityTrigger {
	threshold: Duration,
	limiter: RateLimiter,
	last_activity: TimestampMs,
}

impl InactivityTrigger {
	pub fn new(threshold: Duration, rate_per_sec: f64, now: TimestampMs) -> Self {
		Self { threshold, limiter: RateLimiter::new(1.0, rate_per_sec), last_activity: now }
	}
	pub fn record_activity(&mut self, ts: TimestampMs) { self.last_activity = ts; }
	/// Returns true if inactivity exceeded threshold and rate limiter allows firing.
	pub fn should_trigger(&mut self, now: TimestampMs) -> bool {
		let idle = now.0.saturating_sub(self.last_activity.0);
	if idle as u128 >= self.threshold.as_millis() {
			// simulate logical time refill based on idle duration
			self.limiter.refill_with(Duration::from_millis(idle));
			if self.limiter.allow() { self.last_activity = now; true } else { false }
		} else { false }
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn ratio_basic() {
		use ScreenState::*;
		let e = [
			(TimestampMs(0), On),
			(TimestampMs(50), Off),
			(TimestampMs(150), On),
			(TimestampMs(200), Off),
			(TimestampMs(300), On),
		];
		let r = screen_off_ratio(&e);
		// Off segments: [50,150)=100ms and [200,300)=100ms over total [0,300)=300ms => 200/300 ~ 0.6666
		assert!((r - 0.6666).abs() < 0.01);
	}
	#[test]
	fn inactivity_trigger() {
		let mut t = InactivityTrigger::new(Duration::from_millis(100), 1000.0, TimestampMs(0));
		assert!(!t.should_trigger(TimestampMs(50)));
		assert!(t.should_trigger(TimestampMs(150)));
		// Rate limited: immediate next call likely false
		assert!(!t.should_trigger(TimestampMs(151)));
		t.record_activity(TimestampMs(200));
		assert!(!t.should_trigger(TimestampMs(250)));
		assert!(t.should_trigger(TimestampMs(350)));
	}
}
