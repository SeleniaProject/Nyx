#![forbid(unsafe_code)]

#[cfg(feature = "reconnect")]
pub mod backoff_policy {
	use std::time::Duration;

	pub fn exponential_with_jitter(attempt: u32, base_ms: u64, max_ms: u64) -> Duration {
		let pow = 1u64.saturating_shl(attempt.min(16));
		let raw = base_ms.saturating_mul(pow);
		let capped = raw.min(max_ms);
		let jitter = fastrand::u64(0..(capped / 2).max(1));
		Duration::from_millis(capped / 2 + jitter)
	}
}

#[cfg(not(feature = "reconnect"))]
pub mod backoff_policy {
	use std::time::Duration;
	pub fn exponential_with_jitter(_attempt: u32, _base_ms: u64, _max_ms: u64) -> Duration { Duration::from_millis(0) }
}

