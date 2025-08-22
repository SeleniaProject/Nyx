use std::time::{Duration, Instant};

/// Exponentially Weighted Moving Average for f64 values.
///
/// Example:
/// -------
/// ```rust
/// use nyx_core::performance::Ewma;
/// let mut e = Ewma::new(0.5);
/// e.update(10.0);
/// e.update(0.0);
/// assert_eq!(e.get(), Some(5.0));
/// ```
#[derive(Debug, Clone)]
pub struct Ewma {
    alpha: f64,
    value: Option<f64>,
}

impl Ewma {
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha,
            value: None,
        }
    }

    #[inline(always)]
    pub fn update(&mut self, x: f64) {
        self.value = Some(match self.value {
            Some(v) => self.alpha * x + (1.0 - self.alpha) * v,
            None => x,
        });
    }
    pub fn get(&self) -> Option<f64> {
        self.value
    }
}

/// Token-bucket rate limiter (single-thread use; wrap in `Mutex` for shared use).
/// Ultra-high performance version with CPU cache optimization and minimal branching.
///
/// Example (deterministic with logical time)
/// ---------------------------------------
/// ```rust
/// use nyx_core::performance::RateLimiter;
/// use std::time::Duration;
/// let mut rl = RateLimiter::new(2.0, 4.0); // capacity 2, 4 tokens/sec
/// assert!(rl.allow()); // consume 1 (1 left)
/// assert!(rl.allow()); // consume 1 (0 left)
/// assert!(!rl.allow());
/// rl.refill_with(Duration::from_millis(250)); // +1 token (0.25*4)
/// assert!(rl.allow()); // consume 1 (back to 0)
/// rl.refill_with(Duration::from_secs(1)); // +4 tokens, capped at capacity 2
/// assert!(rl.allow());
/// assert!(rl.allow());
/// assert!(!rl.allow()); // capacity cap respected
/// ```
#[derive(Debug, Clone)]
#[repr(align(64))] // Cache line alignment for L1 cache optimization
pub struct RateLimiter {
    capacity: f64,
    tokens: f64,
    refill_per_sec: f64,
    last_refill_nanos: u64, // Use nanos for higher precision and avoid floating point conversion
}

impl RateLimiter {
    /// Creates a new ultra-high performance rate limiter.
    /// 
    /// # Arguments
    /// * `capacity` - Maximum token capacity (burst size)
    /// * `refill_per_sec` - Tokens refilled per second
    pub fn new(capacity: f64, refill_per_sec: f64) -> Self {
        Self {
            capacity,
            tokens: capacity,
            refill_per_sec,
            last_refill_nanos: Self::get_time_nanos(),
        }
    }

    /// Get current time in nanoseconds for maximum precision.
    /// Falls back to standard time measurement for compatibility.
    #[inline(always)]
    fn get_time_nanos() -> u64 {
        // Use high-precision timer for optimal performance
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }

    /// Ultra-optimized refill with minimal floating-point operations.
    #[inline(always)]
    fn refill_ultra_fast(&mut self) {
        let current_nanos = Self::get_time_nanos();
        let elapsed_nanos = current_nanos.saturating_sub(self.last_refill_nanos);
        self.last_refill_nanos = current_nanos;
        
        // Convert to seconds using efficient division
        const NANOS_PER_SEC: f64 = 1_000_000_000.0;
        let elapsed_secs = elapsed_nanos as f64 / NANOS_PER_SEC;
        
        // Branchless min operation for performance
        let new_tokens = self.tokens + elapsed_secs * self.refill_per_sec;
        self.tokens = if new_tokens > self.capacity { self.capacity } else { new_tokens };
    }

    #[inline(always)]
    fn refill(&mut self) {
        // High-performance time measurement
        let now_nanos = Self::get_time_nanos();
        let elapsed_nanos = now_nanos.saturating_sub(self.last_refill_nanos);
        self.last_refill_nanos = now_nanos;
        
        let dt = elapsed_nanos as f64 / 1_000_000_000.0; // Convert to seconds
        // Use f64::min for better performance than Ord::min
        self.tokens = (self.tokens + dt * self.refill_per_sec).min(self.capacity);
    }
    /// Refill by a provided elapsed duration (logical time). Does not change internal `last_refill`.
    pub fn refill_with(&mut self, dt: Duration) {
        self.tokens =
            (self.tokens + dt.as_secs_f64() * self.refill_per_sec).min(self.capacity);
    }
    /// Try to consume one token. Returns whether allowed now.
    #[inline(always)]
    pub fn allow(&mut self) -> bool {
        self.refill();
        // Use >= comparison to avoid potential floating point precision issues
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
    /// Wait until allowed or timeout; returns true if allowed.
    /// Uses exponential backoff instead of busy waiting for better performance
    pub fn wait_until_allowed(&mut self, timeout: Duration) -> bool {
        let start = Instant::now();
        let mut sleep_duration = Duration::from_millis(1);

        while !self.allow() {
            if start.elapsed() >= timeout {
                return false;
            }

            // Exponential backoff with maximum sleep time to prevent excessive waiting
            std::thread::sleep(sleep_duration.min(Duration::from_millis(10)));
            sleep_duration = (sleep_duration * 2).min(Duration::from_millis(10));
        }
        true
    }

    /// Ultimate performance version: minimal branching, CPU cache-friendly
    #[inline(always)]
    pub fn allow_ultra_fast(&mut self) -> bool {
        self.refill_ultra_fast();
        
        // Branchless token consumption using bit manipulation
        let has_tokens = (self.tokens >= 1.0) as u8;
        self.tokens -= has_tokens as f64;
        has_tokens != 0
    }

    /// More efficient version that avoids repeated refill calls
    #[inline(always)]
    pub fn allow_optimized(&mut self) -> bool {
        let current_nanos = Self::get_time_nanos();
        let elapsed_nanos = current_nanos.saturating_sub(self.last_refill_nanos);
        self.last_refill_nanos = current_nanos;
        
        let dt = elapsed_nanos as f64 / 1_000_000_000.0; // Convert to seconds
        self.tokens = (self.tokens + dt * self.refill_per_sec).min(self.capacity);

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ewma_behavior() {
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
    #[test]
    fn rate_limiter_refill_with_caps() {
        let mut rl = RateLimiter::new(2.0, 4.0);
        assert!(rl.allow());
        assert!(rl.allow());
        assert!(!rl.allow());
        rl.refill_with(Duration::from_millis(250));
        assert!(rl.allow());
        rl.refill_with(Duration::from_secs(1)); // should cap at capacity
        assert!(rl.allow());
        assert!(rl.allow());
        assert!(!rl.allow());
    }
}

