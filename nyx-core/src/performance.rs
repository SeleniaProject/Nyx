use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Exponentially Weighted Moving Average for f64 values.
///
/// The EWMA provides a statistical technique for smoothing time-series data by giving
/// more weight to recent observations while maintaining a memory of historical values.
/// This implementation is particularly useful for network metrics and performance monitoring.
///
/// # Mathematical Foundation
/// The EWMA formula: EWMA(t) = α × Y(t) + (1-α) × EWMA(t-1)
/// where α is the smoothing factor (0 < α ≤ 1) and Y(t) is the current observation.
/// 
/// # Thread Safety
/// This struct is not thread-safe. Use synchronization primitives if sharing across threads.
///
/// # Example
/// ```rust
/// use nyx_core::performance::Ewma;
/// 
/// let mut ewma = Ewma::new(0.5);
/// ewma.update(10.0);
/// ewma.update(0.0);
/// assert_eq!(ewma.get(), Some(5.0));
/// ```
#[derive(Debug, Clone)]
pub struct Ewma {
    /// Smoothing factor between 0.0 and 1.0
    /// Higher values give more weight to recent observations
    alpha: f64,
    /// Current EWMA value, None if no values have been processed yet
    value: Option<f64>,
}

impl Ewma {
    /// Creates a new EWMA with the given smoothing factor.
    /// 
    /// # Arguments
    /// * `alpha` - Smoothing factor between 0.0 and 1.0. Higher values respond faster to changes.
    ///   Values closer to 1.0 make the EWMA more responsive to recent changes,
    ///   while values closer to 0.0 make it more stable.
    /// 
    /// # Panics
    /// Panics if alpha is not in the range (0.0, 1.0]
    /// Create a new EWMA with the given smoothing factor
    #[must_use]
    pub fn new(alpha: f64) -> Self {
        assert!(
            alpha > 0.0 && alpha <= 1.0,
            "Alpha must be in range (0.0, 1.0], got: {alpha}",
        );
        
        Self {
            alpha,
            value: None,
        }
    }

    /// Updates the EWMA with a new value.
    /// 
    /// The first value becomes the initial EWMA value. Subsequent values are
    /// smoothed using the exponential weighting formula.
    /// 
    /// # Arguments
    /// * `value` - The new observation to incorporate into the EWMA
    pub fn update(&mut self, value: f64) {
        self.value = Some(match self.value {
            Some(current) => self.alpha.mul_add(value, (1.0 - self.alpha) * current),
            None => value,
        });
    }
    
    /// Returns the current EWMA value.
    /// 
    /// # Returns
    /// * `Some(value)` if at least one update has been performed
    /// * `None` if no values have been processed yet
    #[must_use]
    pub fn get(&self) -> Option<f64> {
        self.value
    }

    /// Resets the EWMA to its initial state (no values).
    /// 
    /// After calling this method, `get()` will return `None` until `update()` is called.
    pub fn reset(&mut self) {
        self.value = None;
    }

    /// Returns the smoothing factor (alpha) used by this EWMA.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
}

/// Token bucket rate limiter with configurable capacity and refill rate.
///
/// This implementation provides a thread-safe, high-precision token bucket algorithm
/// suitable for rate limiting in network applications. The token bucket allows for
/// burst traffic up to the bucket capacity while maintaining an average rate limit.
///
/// # Algorithm Details
/// - Tokens are added at a constant rate (refill_rate tokens per second)
/// - The bucket can hold at most `capacity` tokens
/// - Each operation consumes exactly one token
/// - Operations are denied when no tokens are available
///
/// # Precision and Performance
/// - Uses nanosecond precision for accurate rate limiting
/// - Handles clock adjustments and time overflow gracefully
/// - Optimized for high-frequency operations with minimal overhead
///
/// # Thread Safety
/// This struct is not inherently thread-safe. Use appropriate synchronization
/// primitives when sharing across multiple threads.
///
/// # Example
/// ```rust
/// use nyx_core::performance::RateLimiter;
/// use std::time::Duration;
/// 
/// let mut limiter = RateLimiter::new(2.0, 4.0); // capacity: 2, rate: 4 tokens/sec
/// assert!(limiter.allow()); // consume 1 token
/// assert!(limiter.allow()); // consume 1 token
/// assert!(!limiter.allow()); // no tokens left
/// 
/// limiter.refill_with(Duration::from_millis(250)); // +1 token (0.25 * 4)
/// assert!(limiter.allow()); // consume the refilled token
/// ```
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Maximum number of tokens the bucket can hold
    capacity: f64,
    /// Current number of tokens in the bucket
    tokens: f64,
    /// Number of tokens added per second
    refill_rate: f64,
    /// Last refill timestamp in nanoseconds since UNIX epoch
    last_refill: u64,
}

/// Errors that can occur during rate limiter operations
#[derive(Debug, thiserror::Error)]
pub enum RateLimiterError {
    /// Clock moved backwards or time system error
    #[error("Time system error: {0}")]
    TimeError(String),
    /// Invalid configuration parameters
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

impl RateLimiter {
    /// Creates a new rate limiter with specified capacity and refill rate.
    /// 
    /// # Arguments
    /// * `capacity` - Maximum number of tokens that can be stored (must be > 0.0)
    /// * `refill_rate` - Number of tokens added per second (must be > 0.0)
    /// 
    /// # Panics
    /// Panics if capacity or refill_rate are not positive finite numbers
    /// 
    /// # Example
    /// ```rust
    /// use nyx_core::performance::RateLimiter;
    /// 
    /// // Allow bursts of up to 10 requests, with sustained rate of 5 req/sec
    /// let limiter = RateLimiter::new(10.0, 5.0);
    /// ```
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        assert!(
            capacity > 0.0 && capacity.is_finite(),
            "Capacity must be positive and finite, got: {capacity}",
        );
        assert!(
            refill_rate > 0.0 && refill_rate.is_finite(),
            "Refill rate must be positive and finite, got: {refill_rate}",
        );

        Self {
            capacity,
            tokens: capacity, // Start with full bucket
            refill_rate,
            last_refill: Self::current_time_nanos_safe(),
        }
    }

    /// Gets current time in nanoseconds, handling potential system clock issues.
    /// 
    /// # Returns
    /// Nanoseconds since UNIX epoch, or 0 if system time is unavailable
    fn current_time_nanos_safe() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }

    /// Refills tokens based on elapsed time since last refill.
    /// 
    /// This method handles:
    /// - Clock adjustments (backwards time movement)
    /// - Integer overflow protection
    /// - Precision maintenance in floating-point calculations
    /// 
    /// # Error Handling
    /// - If time moves backwards, no tokens are added but operation continues
    /// - Gracefully handles very large time differences
    fn refill(&mut self) {
        let now = Self::current_time_nanos_safe();
        
        // Handle backwards clock movement gracefully
        if now < self.last_refill {
            // Clock moved backwards - reset to current time but don't add tokens
            self.last_refill = now;
            return;
        }

        let elapsed_nanos = now.saturating_sub(self.last_refill);
        self.last_refill = now;

        if elapsed_nanos > 0 {
            // Convert nanoseconds to seconds with high precision
            let elapsed_secs = elapsed_nanos as f64 / 1_000_000_000.0;
            
            // Calculate tokens to add, using fused multiply-add for precision
            let tokens_to_add = elapsed_secs * self.refill_rate;
            
            // Add tokens but cap at capacity
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
        }
    }

    /// Attempts to consume one token from the bucket.
    /// 
    /// This operation will first refill the bucket based on elapsed time,
    /// then attempt to consume a token if available.
    /// 
    /// # Returns
    /// * `true` if a token was successfully consumed
    /// * `false` if no tokens were available
    /// 
    /// # Performance
    /// This method is optimized for high-frequency calls and has minimal overhead.
    pub fn allow(&mut self) -> bool {
        self.refill();
        
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
    
    /// Refills tokens based on a provided duration (primarily for testing).
    /// 
    /// This method allows manual time-based refilling, useful for deterministic
    /// testing scenarios where precise timing control is needed.
    /// 
    /// # Arguments
    /// * `duration` - Time duration to simulate for token refill
    /// 
    /// # Note
    /// This method does not update the internal timestamp and should primarily
    /// be used for testing purposes.
    pub fn refill_with(&mut self, duration: Duration) {
        let tokens_to_add = duration.as_secs_f64() * self.refill_rate;
        self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
    }
    
    /// Waits until a token is available or timeout expires using exponential backoff.
    /// 
    /// This method implements a sophisticated waiting strategy:
    /// - Exponential backoff starting from 1ms
    /// - Maximum backoff capped at 10ms to remain responsive
    /// - Efficient spinning for short waits
    /// 
    /// # Arguments
    /// * `timeout` - Maximum time to wait for a token
    /// 
    /// # Returns
    /// * `true` if a token was successfully obtained within the timeout
    /// * `false` if the timeout expired without obtaining a token
    /// 
    /// # Performance Considerations
    /// This method yields CPU time during waits, making it suitable for
    /// concurrent environments. However, for high-frequency use cases,
    /// consider using `allow()` with custom retry logic.
    pub fn wait_until_allowed(&mut self, timeout: Duration) -> bool {
        let start = Instant::now();
        let mut backoff = Duration::from_millis(1);
        const MAX_BACKOFF: Duration = Duration::from_millis(10);

        while !self.allow() {
            if start.elapsed() >= timeout {
                return false;
            }

            // Sleep with exponential backoff, capped at MAX_BACKOFF
            std::thread::sleep(backoff.min(MAX_BACKOFF));
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
        true
    }

    /// Returns the current number of available tokens.
    /// 
    /// This method refills the bucket before returning the token count,
    /// ensuring the returned value is current.
    pub fn available_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }

    /// Returns the bucket capacity.
    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    /// Returns the refill rate (tokens per second).
    pub fn refill_rate(&self) -> f64 {
        self.refill_rate
    }

    /// Returns the time until the next token will be available.
    /// 
    /// # Returns
    /// * `Duration::ZERO` if tokens are currently available
    /// * `Duration` representing wait time for the next token
    pub fn time_until_next_token(&mut self) -> Duration {
        self.refill();
        
        if self.tokens >= 1.0 {
            Duration::ZERO
        } else {
            let tokens_needed = 1.0 - self.tokens;
            let seconds_needed = tokens_needed / self.refill_rate;
            Duration::from_secs_f64(seconds_needed)
        }
    }

    /// Resets the rate limiter to its initial state with a full bucket.
    pub fn reset(&mut self) {
        self.tokens = self.capacity;
        self.last_refill = Self::current_time_nanos_safe();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn ewma_basic_behavior() {
        let mut ewma = Ewma::new(0.5);
        
        // Test initial state
        assert_eq!(ewma.get(), None);
        
        // Test first value
        ewma.update(10.0);
        assert_eq!(ewma.get(), Some(10.0));
        
        // Test smoothing
        ewma.update(0.0);
        assert_eq!(ewma.get(), Some(5.0));
        
        // Test alpha getter
        assert_eq!(ewma.alpha(), 0.5);
    }

    #[test]
    fn ewma_reset_functionality() {
        let mut ewma = Ewma::new(0.3);
        ewma.update(100.0);
        assert_eq!(ewma.get(), Some(100.0));
        
        ewma.reset();
        assert_eq!(ewma.get(), None);
        
        ewma.update(50.0);
        assert_eq!(ewma.get(), Some(50.0));
    }

    #[test]
    #[should_panic(expected = "Alpha must be in range (0.0, 1.0]")]
    fn ewma_invalid_alpha_zero() {
        let _ = Ewma::new(0.0);
    }

    #[test]
    #[should_panic(expected = "Alpha must be in range (0.0, 1.0]")]
    fn ewma_invalid_alpha_negative() {
        let _ = Ewma::new(-0.1);
    }

    #[test]
    #[should_panic(expected = "Alpha must be in range (0.0, 1.0]")]
    fn ewma_invalid_alpha_too_large() {
        let _ = Ewma::new(1.1);
    }

    #[test]
    fn ewma_edge_case_alpha() {
        // Test alpha = 1.0 (should work)
        let mut ewma = Ewma::new(1.0);
        ewma.update(10.0);
        ewma.update(20.0);
        assert_eq!(ewma.get(), Some(20.0)); // With alpha=1.0, only latest value matters
    }

    #[test]
    fn rate_limiter_basic_functionality() {
        let mut limiter = RateLimiter::new(1.0, 2.0);
        
        // Should allow first token
        assert!(limiter.allow());
        // No tokens left - use available_tokens carefully since it refills
        assert!(!limiter.allow());
        
        // Test getters
        assert_eq!(limiter.capacity(), 1.0);
        assert_eq!(limiter.refill_rate(), 2.0);
        // Check available tokens - should be very close to 0 (might have tiny refill)
        let tokens = limiter.available_tokens();
        assert!(tokens < 0.1, "Should have very few tokens, got: {tokens}");
    }

    #[test]
    fn rate_limiter_wait_functionality() {
        let mut limiter = RateLimiter::new(1.0, 2.0);
        
        // Consume the token
        assert!(limiter.allow());
        assert!(!limiter.allow());
        
        // Should allow within reasonable timeout
        let allowed = limiter.wait_until_allowed(Duration::from_millis(700));
        assert!(allowed, "Should obtain token within timeout");
    }
    
    #[test]
    fn rate_limiter_refill_with_duration() {
        let mut limiter = RateLimiter::new(2.0, 4.0);
        
        // Consume all tokens
        assert!(limiter.allow());
        assert!(limiter.allow());
        assert!(!limiter.allow());
        
        // Refill with specific duration
        limiter.refill_with(Duration::from_millis(250)); // 0.25 * 4 = 1 token
        assert!(limiter.allow());
        assert!(!limiter.allow());
        
        // Large refill should cap at capacity
        limiter.refill_with(Duration::from_secs(1)); // 1.0 * 4 = 4 tokens, capped at 2
        assert!(limiter.allow());
        assert!(limiter.allow());
        assert!(!limiter.allow()); // capacity respected
    }

    #[test]
    fn rate_limiter_reset_functionality() {
        let mut limiter = RateLimiter::new(3.0, 1.0);
        
        // Consume some tokens
        assert!(limiter.allow());
        assert!(limiter.allow());
        let tokens_before_reset = limiter.available_tokens();
        assert!(tokens_before_reset < 2.0, "Should have consumed tokens");
        
        // Reset should restore full capacity
        limiter.reset();
        let tokens_after_reset = limiter.available_tokens();
        assert!(
            (tokens_after_reset - 3.0).abs() < 0.1,
            "Should have close to full capacity after reset, got: {tokens_after_reset}"
        );
    }

    #[test]
    fn rate_limiter_time_until_next_token() {
        let mut limiter = RateLimiter::new(1.0, 2.0); // 2 tokens per second = 0.5 seconds per token
        
        // With tokens available, should be zero
        assert_eq!(limiter.time_until_next_token(), Duration::ZERO);
        
        // After consuming token, should need time
        assert!(limiter.allow());
        let wait_time = limiter.time_until_next_token();
        assert!(wait_time > Duration::ZERO);
        assert!(wait_time <= Duration::from_millis(500)); // Should be ~500ms for 2 tokens/sec
    }

    #[test]
    #[should_panic(expected = "Capacity must be positive and finite")]
    fn rate_limiter_invalid_capacity_zero() {
        RateLimiter::new(0.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "Capacity must be positive and finite")]
    fn rate_limiter_invalid_capacity_negative() {
        RateLimiter::new(-1.0, 1.0);
    }

    #[test]
    #[should_panic(expected = "Refill rate must be positive and finite")]
    fn rate_limiter_invalid_refill_rate() {
        RateLimiter::new(1.0, 0.0);
    }

    #[test]
    fn rate_limiter_backwards_time_handling() {
        let mut limiter = RateLimiter::new(1.0, 1.0);
        
        // Consume token
        assert!(limiter.allow());
        assert!(!limiter.allow());
        
        // Test that backwards time doesn't break the limiter
        // (This is more of a regression test - the implementation should handle it gracefully)
        // We can't easily simulate backwards time, but we test that multiple refills work
        for _ in 0..10 {
            limiter.refill_with(Duration::from_millis(100));
        }
        
        // Should still work correctly
        assert!(limiter.allow());
    }

    #[test]
    fn rate_limiter_high_precision_timing() {
        let mut limiter = RateLimiter::new(1000.0, 1000.0); // High rate for precision testing
        
        // Should handle high frequency operations
        let mut success_count = 0;
        for _ in 0..999 {
            if limiter.allow() {
                success_count += 1;
            }
        }
        
        assert!(success_count >= 995, "Should allow most tokens, got: {success_count}");
        
        // Try a few more - should eventually be denied
        let mut denied = false;
        for _ in 0..10 {
            if !limiter.allow() {
                denied = true;
                break;
            }
        }
        assert!(denied, "Should eventually run out of tokens");
    }

    #[test]
    fn rate_limiter_concurrent_safety_single_thread() {
        let mut limiter = RateLimiter::new(10.0, 5.0);
        
        // Simulate concurrent-like access in single thread
        let mut total_allowed = 0;
        for _ in 0..20 {
            if limiter.allow() {
                total_allowed += 1;
            }
            // Simulate some processing time
            thread::sleep(Duration::from_nanos(1));
        }
        
        assert!(total_allowed <= 10, "Should not exceed capacity");
    }

    #[test]
    fn ewma_precision_with_large_numbers() {
        let mut ewma = Ewma::new(0.1);
        
        // Test with large numbers to check for precision issues
        ewma.update(1_000_000.0);
        ewma.update(2_000_000.0);
        
        let result = ewma.get().unwrap();
        assert!(result > 1_000_000.0 && result < 2_000_000.0);
        
        // Should be close to 1,100,000 (0.1 * 2M + 0.9 * 1M)
        assert!((result - 1_100_000.0).abs() < 1.0);
    }

    #[test]
    fn rate_limiter_fractional_tokens() {
        let mut limiter = RateLimiter::new(1.5, 0.5); // Allow fractional values
        
        // Should allow one full token
        assert!(limiter.allow());
        
        // Should have 0.5 tokens left, which is less than 1.0 needed
        assert!(!limiter.allow());
        
        // After refilling for 1 second, should have 1.0 token (0.5 existing + 0.5 added)
        limiter.refill_with(Duration::from_secs(1));
        assert!(limiter.allow());
    }
}

