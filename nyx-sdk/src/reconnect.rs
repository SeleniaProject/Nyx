#![forbid(unsafe_code)]

#[cfg(feature = "reconnect")]
pub mod retry_policy {
    use std::time::Duration;
    use tokio_retry::strategy::{jitter, ExponentialBackoff};

    /// Create an exponential backoff strategy with jitter for connection retries.
    /// This provides better distribution of retry attempts to avoid thundering herd problems.
    pub fn exponential_with_jitter(base_ms: u64, max_ms: u64) -> impl Iterator<Item = Duration> {
        ExponentialBackoff::from_millis(base_ms)
            .max_delay(Duration::from_millis(max_ms))
            .map(jitter) // Add jitter to prevent thundering herd
    }

    /// Simple exponential backoff calculation for single attempt delays.
    /// Used for compatibility with existing code patterns.
    #[must_use]
    pub fn calculate_delay(attempt: u32, base_ms: u64, max_ms: u64) -> Duration {
        // Compute 2^attempt with a safe upper bound to avoid shifting by >= 64.
        // Use saturating multiplication so large attempts cap at max_ms below.
        let shift = attempt.min(63);
        let pow = 1u64 << shift;
        let raw = base_ms.saturating_mul(pow);
        let capped = raw.min(max_ms);
        if capped == 0 {
            return Duration::from_millis(0);
        }
        let half = (capped / 2).max(1);
        let jitter = fastrand::u64(0..half);
        Duration::from_millis(half + jitter)
    }
}

#[cfg(not(feature = "reconnect"))]
pub mod retry_policy {
    use std::time::Duration;

    /// No-op implementation when reconnect feature is disabled.
    pub fn exponential_with_jitter(_base_ms: u64, _max_ms: u64) -> impl Iterator<Item = Duration> {
        std::iter::empty()
    }

    /// No-op delay calculation when reconnect feature is disabled.
    #[must_use]
    pub fn calculate_delay(_attempt: u32, _base_ms: u64, _max_ms: u64) -> Duration {
        Duration::from_millis(0)
    }
}

// Backward compatibility alias for existing code
#[cfg(feature = "reconnect")]
pub mod backoff_policy {
    pub use super::retry_policy::*;
    // Legacy function name for compatibility
    pub use super::retry_policy::calculate_delay as exponential_with_jitter;
}

#[cfg(not(feature = "reconnect"))]
pub mod backoff_policy {
    pub use super::retry_policy::*;
    // Legacy function name for compatibility
    pub use super::retry_policy::calculate_delay as exponential_with_jitter;
}
