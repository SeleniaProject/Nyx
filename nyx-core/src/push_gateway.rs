//! Push Notification Path / Gateway Reconnection Manager
//!
//! Provides a lightweight pathway to (re)establish a minimal Nyx path set
//! after a mobile push wake event. Designed for Low Power scenarios where
//! the primary multipath set has been torn down or quiesced to save power.
//!
//! Responsibilities:
//! - Track last wake timestamps and debounce spurious multiple wakes.
//! - Initiate fast path builder request (1 control + 1 data) via callback.
//! - Expose FFI functions: nyx_push_wake(), nyx_resume_low_power_session().
//! - Provide jittered exponential backoff with capped retry for reconnection failures.
//! - Collect detailed latency histograms with configurable buckets for analysis.
//!
//! This module intentionally avoids direct dependency on heavy routing
//! components; instead it relies on an injected trait object implementing
//! a minimal reconnection contract so that integration stays decoupled.
//!
//! Safety: All extern "C" functions are thin wrappers that delegate into
//! thread-safe interior (Arc + Mutex). No unsafe code required.

#[cfg(feature = "telemetry")]
use nyx_telemetry::metrics::BasicMetrics;
use once_cell::sync::OnceCell;
use rand::{thread_rng, Rng};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn}; // basic counter metrics

/// Latency histogram with configurable buckets for detailed analysis
#[derive(Debug, Clone)]
pub struct LatencyHistogram {
    /// Histogram buckets: maps upper bound (ms) to count
    buckets: HashMap<u64, u64>,
    /// Predefined bucket boundaries in milliseconds
    bucket_boundaries: Vec<u64>,
    /// Total sample count
    total_samples: u64,
    /// Sum of all samples for mean calculation
    sum_latency_ms: u128,
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        // Define buckets optimized for push gateway latency characteristics
        // Covers range from sub-second to multiple seconds with good granularity
        let bucket_boundaries = vec![
            10, 25, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 30000, 60000,
        ];

        let mut buckets = HashMap::new();
        for &boundary in &bucket_boundaries {
            buckets.insert(boundary, 0);
        }
        // Add overflow bucket for > 60s
        buckets.insert(u64::MAX, 0);

        Self {
            buckets,
            bucket_boundaries,
            total_samples: 0,
            sum_latency_ms: 0,
        }
    }
}

impl LatencyHistogram {
    /// Record a latency sample in the appropriate bucket
    pub fn record_sample(&mut self, latency_ms: u64) {
        self.total_samples += 1;
        self.sum_latency_ms += latency_ms as u128;

        // Find appropriate bucket (first boundary >= sample value)
        let bucket_key = self
            .bucket_boundaries
            .iter()
            .find(|&&boundary| latency_ms <= boundary)
            .copied()
            .unwrap_or(u64::MAX); // Overflow bucket

        *self.buckets.entry(bucket_key).or_insert(0) += 1;

        debug!(
            "Recorded latency sample: {}ms in bucket ≤{}",
            latency_ms,
            if bucket_key == u64::MAX {
                "∞".to_string()
            } else {
                bucket_key.to_string()
            }
        );
    }

    /// Get total number of recorded samples
    pub fn total_samples(&self) -> u64 {
        self.total_samples
    }

    /// Calculate percentile from histogram buckets
    pub fn calculate_percentile(&self, percentile: f64) -> Option<u64> {
        if self.total_samples == 0 {
            return None;
        }

        let target_count = (percentile * self.total_samples as f64).ceil() as u64;
        let mut cumulative_count = 0;

        // Sort buckets by boundary value for percentile calculation
        let mut sorted_buckets: Vec<(u64, u64)> = self
            .buckets
            .iter()
            .map(|(&boundary, &count)| (boundary, count))
            .collect();
        sorted_buckets.sort_by_key(|(boundary, _)| *boundary);

        for (boundary, count) in sorted_buckets {
            cumulative_count += count;
            if cumulative_count >= target_count {
                return Some(boundary);
            }
        }

        None
    }

    /// Get mean latency
    pub fn mean_latency_ms(&self) -> Option<f64> {
        if self.total_samples == 0 {
            None
        } else {
            Some(self.sum_latency_ms as f64 / self.total_samples as f64)
        }
    }

    /// Get bucket distribution for detailed analysis
    pub fn bucket_distribution(&self) -> Vec<(u64, u64)> {
        let mut distribution: Vec<(u64, u64)> = self
            .buckets
            .iter()
            .map(|(&boundary, &count)| (boundary, count))
            .collect();
        distribution.sort_by_key(|(boundary, _)| *boundary);
        distribution
    }

    /// Reset histogram (useful for periodic analysis)
    pub fn reset(&mut self) {
        for count in self.buckets.values_mut() {
            *count = 0;
        }
        self.total_samples = 0;
        self.sum_latency_ms = 0;
    }
}

/// Enhanced jittered backoff calculator
#[derive(Debug, Clone)]
pub struct JitteredBackoff {
    base_delay: Duration,
    max_delay: Duration,
    jitter_factor: f64, // 0.0 to 1.0, representing maximum jitter percentage
}

impl Default for JitteredBackoff {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(30), // Cap at 30 seconds
            jitter_factor: 0.3,                 // Up to 30% jitter
        }
    }
}

impl JitteredBackoff {
    /// Calculate next backoff delay with exponential growth and jitter
    pub fn calculate_delay(&self, attempt: u8) -> Duration {
        // Exponential backoff: base * 2^(attempt-1)
        let exponential_delay =
            self.base_delay.as_millis() as u64 * 2u64.pow((attempt.saturating_sub(1)) as u32);

        // Cap to maximum delay
        let capped_delay = exponential_delay.min(self.max_delay.as_millis() as u64);

        // Apply jitter: random factor between (1 - jitter_factor) and (1 + jitter_factor)
        let jitter_range = self.jitter_factor;
        let jitter_multiplier = {
            let mut rng = thread_rng();
            1.0 + (rng.gen::<f64>() - 0.5) * 2.0 * jitter_range
        };

        let jittered_delay = (capped_delay as f64 * jitter_multiplier) as u64;

        debug!(
            "Calculated jittered backoff for attempt {}: base={}ms, exponential={}ms, capped={}ms, jittered={}ms",
            attempt,
            self.base_delay.as_millis(),
            exponential_delay,
            capped_delay,
            jittered_delay
        );

        Duration::from_millis(jittered_delay)
    }

    /// Configure custom backoff parameters
    pub fn with_config(base_delay: Duration, max_delay: Duration, jitter_factor: f64) -> Self {
        Self {
            base_delay,
            max_delay,
            jitter_factor: jitter_factor.clamp(0.0, 1.0),
        }
    }
}

/// Error type for push gateway operations.
#[derive(thiserror::Error, Debug)]
pub enum PushGatewayError {
    #[error("Reconnection already in progress")]
    AlreadyInProgress,
    #[error("Too soon since last wake (debounced)")]
    Debounced,
    #[error("Executor unavailable")]
    ExecutorUnavailable,
    #[error("Maximum retries exhausted")]
    RetriesExhausted,
}

/// Minimal trait abstracting a reconnection path builder.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub trait MinimalReconnector: Send + Sync + 'static {
    /// Attempt to (re)establish a minimal path set. Should be idempotent.
    fn reconnect_minimal(&self) -> BoxFuture<'_, Result<(), String>>;
}

/// Internal mutable state with enhanced telemetry capabilities
#[derive(Debug)]
struct InnerState {
    last_wake: Option<Instant>,
    reconnect_in_flight: bool,

    // Basic counters
    total_wake_events: u64,
    debounced_wake_events: u64,
    total_reconnect_attempts: u64,
    total_reconnect_failures: u64,
    total_reconnect_success: u64,

    // Legacy simple metrics for backward compatibility
    cumulative_latency_ms: u128,
    latency_samples: VecDeque<u64>, // ring buffer for percentile calc

    // Enhanced histogram for detailed analysis
    latency_histogram: LatencyHistogram,

    // Backoff configuration
    backoff_calculator: JitteredBackoff,

    #[cfg(feature = "telemetry")]
    wake_metric: BasicMetrics,
    #[cfg(feature = "telemetry")]
    debounced_wake_metric: BasicMetrics,
    #[cfg(feature = "telemetry")]
    reconnect_success_metric: BasicMetrics,
    #[cfg(feature = "telemetry")]
    reconnect_fail_metric: BasicMetrics,
}

impl Default for InnerState {
    fn default() -> Self {
        Self {
            last_wake: None,
            reconnect_in_flight: false,
            total_wake_events: 0,
            debounced_wake_events: 0,
            total_reconnect_attempts: 0,
            total_reconnect_failures: 0,
            total_reconnect_success: 0,
            cumulative_latency_ms: 0,
            latency_samples: VecDeque::with_capacity(64),
            latency_histogram: LatencyHistogram::default(),
            backoff_calculator: JitteredBackoff::default(),

            #[cfg(feature = "telemetry")]
            wake_metric: BasicMetrics::new(),
            #[cfg(feature = "telemetry")]
            debounced_wake_metric: BasicMetrics::new(),
            #[cfg(feature = "telemetry")]
            reconnect_success_metric: BasicMetrics::new(),
            #[cfg(feature = "telemetry")]
            reconnect_fail_metric: BasicMetrics::new(),
        }
    }
}

/// Enhanced Push Gateway Manager with jittered backoff and detailed histograms
pub struct PushGatewayManager {
    state: Mutex<InnerState>,
    reconnector: Arc<dyn MinimalReconnector>,
    debounce: Duration,
    max_retries: u8,
}

impl PushGatewayManager {
    pub fn new(reconnector: Arc<dyn MinimalReconnector>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(InnerState::default()),
            reconnector,
            debounce: Duration::from_secs(2),
            max_retries: 5,
        })
    }

    /// Create manager with custom configuration
    pub fn with_config(
        reconnector: Arc<dyn MinimalReconnector>,
        debounce: Duration,
        max_retries: u8,
        backoff_config: JitteredBackoff,
    ) -> Arc<Self> {
        let mut inner_state = InnerState::default();
        inner_state.backoff_calculator = backoff_config;

        Arc::new(Self {
            state: Mutex::new(inner_state),
            reconnector,
            debounce,
            max_retries,
        })
    }

    /// Construct from a simple async closure returning Result<(), String>
    pub fn from_async_fn<F, Fut>(f: F) -> Arc<Self>
    where
        F: Send + Sync + 'static + Fn() -> Fut,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        struct FnReconnector<F>(F);
        impl<F, Fut> MinimalReconnector for FnReconnector<F>
        where
            F: Send + Sync + 'static + Fn() -> Fut,
            Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
        {
            fn reconnect_minimal(&self) -> BoxFuture<'_, Result<(), String>> {
                let fut = (self.0)();
                Box::pin(async move { fut.await })
            }
        }
        let reconnector: Arc<dyn MinimalReconnector> = Arc::new(FnReconnector(f));
        Self::new(reconnector)
    }

    /// Record a push wake event (may trigger reconnection later via resume call).
    pub fn push_wake(&self) -> Result<(), PushGatewayError> {
        let mut s = self.state.lock().unwrap();
        let now = Instant::now();

        // Check debouncing
        if let Some(prev) = s.last_wake {
            if now.duration_since(prev) < self.debounce {
                s.debounced_wake_events += 1;
                #[cfg(feature = "telemetry")]
                {
                    s.debounced_wake_metric.increment();
                }
                return Err(PushGatewayError::Debounced);
            }
        }

        s.last_wake = Some(now);
        s.total_wake_events += 1;
        #[cfg(feature = "telemetry")]
        {
            s.wake_metric.increment();
        }

        info!("Push wake event recorded");
        Ok(())
    }

    /// Enhanced resume with jittered backoff and detailed latency tracking
    pub async fn resume_low_power_session(self: &Arc<Self>) -> Result<(), PushGatewayError> {
        let start_all = Instant::now();

        // Set reconnection in progress flag
        {
            let mut s = self.state.lock().unwrap();
            if s.reconnect_in_flight {
                return Err(PushGatewayError::AlreadyInProgress);
            }
            s.reconnect_in_flight = true;
        }

    let mut attempt: u8 = 0;
    // Track last error (if any) only when a failure occurs to avoid unused assignment warnings
    let mut last_error: Option<String> = None;

        loop {
            attempt += 1;

            // Record attempt (short critical section)
            {
                let mut s = self.state.lock().unwrap();
                s.total_reconnect_attempts += 1;
            }

            info!(
                "Attempting reconnection (attempt {} of {})",
                attempt, self.max_retries
            );

            // Attempt reconnection
            let reconnect_start = Instant::now();
            let res = self.reconnector.reconnect_minimal().await;
            let reconnect_duration = reconnect_start.elapsed();

            match res {
                Ok(_) => {
                    let total_elapsed_ms = start_all.elapsed().as_millis() as u64;
                    let reconnect_latency_ms = reconnect_duration.as_millis() as u64;

                    // Update success metrics
                    {
                        let mut s = self.state.lock().unwrap();
                        s.reconnect_in_flight = false;
                        s.total_reconnect_success += 1;

                        #[cfg(feature = "telemetry")]
                        {
                            s.reconnect_success_metric.increment();
                        }

                        // Update legacy metrics for backward compatibility
                        s.cumulative_latency_ms += total_elapsed_ms as u128;
                        if s.latency_samples.len() == 64 {
                            s.latency_samples.pop_front();
                        }
                        s.latency_samples.push_back(total_elapsed_ms);

                        // Update detailed histogram
                        s.latency_histogram.record_sample(total_elapsed_ms);
                    }

                    info!(
                        attempt = attempt,
                        reconnect_latency_ms = reconnect_latency_ms,
                        total_latency_ms = total_elapsed_ms,
                        "Minimal path reconnection succeeded"
                    );

                    return Ok(());
                }
                Err(e) => {
                    // Record last error for final logging on retry exhaustion
                    last_error = Some(e.to_string());

                    // Update failure metrics
                    {
                        let mut s = self.state.lock().unwrap();
                        s.total_reconnect_failures += 1;

                        #[cfg(feature = "telemetry")]
                        {
                            s.reconnect_fail_metric.increment();
                        }
                    }

                    warn!(
                        attempt = attempt,
                        error = %e,
                        reconnect_duration_ms = reconnect_duration.as_millis(),
                        "Reconnection attempt failed"
                    );

                    // Check if we've exhausted retries
                    if attempt >= self.max_retries {
                        let mut s = self.state.lock().unwrap();
                        s.reconnect_in_flight = false;

                        // Use a stable string slice for structured logging
                        let last_error_str = last_error.as_deref().unwrap_or("");
                        error!(
                            total_attempts = attempt,
                            last_error = last_error_str,
                            "Reconnection retries exhausted"
                        );

                        return Err(PushGatewayError::RetriesExhausted);
                    }
                }
            }

            // Calculate jittered backoff delay for next attempt
            let backoff_delay = {
                let s = self.state.lock().unwrap();
                s.backoff_calculator.calculate_delay(attempt)
            };

            info!(
                "Waiting {}ms before next reconnection attempt",
                backoff_delay.as_millis()
            );

            tokio::time::sleep(backoff_delay).await;
        }
    }

    /// Get comprehensive statistics including histogram data
    pub fn stats(&self) -> PushGatewayStats {
        let s = self.state.lock().unwrap();

        // Legacy average calculation for backward compatibility
        let avg = if s.total_reconnect_success > 0 {
            Some((s.cumulative_latency_ms / s.total_reconnect_success as u128) as u64)
        } else {
            None
        };

        // Legacy percentile calculation from ring buffer
        let (p50, p95) = percentile_pair(&s.latency_samples);

        // Enhanced histogram-based statistics
        let histogram_p50 = s.latency_histogram.calculate_percentile(0.50);
        let histogram_p95 = s.latency_histogram.calculate_percentile(0.95);
        let histogram_p99 = s.latency_histogram.calculate_percentile(0.99);
        let histogram_mean = s.latency_histogram.mean_latency_ms();
        let bucket_distribution = s.latency_histogram.bucket_distribution();

        PushGatewayStats {
            total_wake_events: s.total_wake_events,
            debounced_wake_events: s.debounced_wake_events,
            total_reconnect_attempts: s.total_reconnect_attempts,
            total_reconnect_failures: s.total_reconnect_failures,
            total_reconnect_success: s.total_reconnect_success,

            // Legacy metrics for backward compatibility
            avg_reconnect_latency_ms: avg,
            p50_latency_ms: p50,
            p95_latency_ms: p95,

            // Enhanced histogram-based metrics
            histogram_p50_ms: histogram_p50,
            histogram_p95_ms: histogram_p95,
            histogram_p99_ms: histogram_p99,
            histogram_mean_ms: histogram_mean,
            histogram_total_samples: s.latency_histogram.total_samples,

            // Detailed bucket distribution for analysis
            latency_buckets: bucket_distribution,
        }
    }

    /// Reset histogram data (useful for periodic analysis windows)
    pub fn reset_histogram(&self) {
        let mut s = self.state.lock().unwrap();
        s.latency_histogram.reset();
        info!("Push gateway latency histogram has been reset");
    }

    /// Configure backoff parameters at runtime
    pub fn configure_backoff(&self, base_delay: Duration, max_delay: Duration, jitter_factor: f64) {
        let mut s = self.state.lock().unwrap();
        s.backoff_calculator = JitteredBackoff::with_config(base_delay, max_delay, jitter_factor);
        info!(
            "Push gateway backoff configured: base={}ms, max={}ms, jitter={:.1}%",
            base_delay.as_millis(),
            max_delay.as_millis(),
            jitter_factor * 100.0
        );
    }
}

/// Enhanced statistics snapshot with histogram data
#[derive(Debug, Clone)]
pub struct PushGatewayStats {
    // Basic counters
    pub total_wake_events: u64,
    pub debounced_wake_events: u64,
    pub total_reconnect_attempts: u64,
    pub total_reconnect_failures: u64,
    pub total_reconnect_success: u64,

    // Legacy metrics for backward compatibility
    pub avg_reconnect_latency_ms: Option<u64>,
    pub p50_latency_ms: Option<u64>,
    pub p95_latency_ms: Option<u64>,

    // Enhanced histogram-based metrics
    pub histogram_p50_ms: Option<u64>,
    pub histogram_p95_ms: Option<u64>,
    pub histogram_p99_ms: Option<u64>,
    pub histogram_mean_ms: Option<f64>,
    pub histogram_total_samples: u64,

    // Detailed bucket distribution: (upper_bound_ms, count)
    pub latency_buckets: Vec<(u64, u64)>,
}

impl PushGatewayStats {
    /// Calculate failure rate as percentage
    pub fn failure_rate_percent(&self) -> f64 {
        if self.total_reconnect_attempts == 0 {
            0.0
        } else {
            (self.total_reconnect_failures as f64 / self.total_reconnect_attempts as f64) * 100.0
        }
    }

    /// Calculate success rate as percentage
    pub fn success_rate_percent(&self) -> f64 {
        if self.total_reconnect_attempts == 0 {
            0.0
        } else {
            (self.total_reconnect_success as f64 / self.total_reconnect_attempts as f64) * 100.0
        }
    }

    /// Get debounce rate as percentage of total wakes
    pub fn debounce_rate_percent(&self) -> f64 {
        if self.total_wake_events == 0 {
            0.0
        } else {
            (self.debounced_wake_events as f64 / self.total_wake_events as f64) * 100.0
        }
    }

    /// Format histogram distribution for logging/display
    pub fn format_histogram_distribution(&self) -> String {
        if self.latency_buckets.is_empty() {
            return "No histogram data available".to_string();
        }

        let mut result = String::from("Latency distribution:\n");
        for (boundary, count) in &self.latency_buckets {
            if *count > 0 {
                let percentage = (*count as f64 / self.histogram_total_samples as f64) * 100.0;
                let boundary_str = if *boundary == u64::MAX {
                    ">60000ms".to_string()
                } else {
                    format!("≤{}ms", boundary)
                };
                result.push_str(&format!(
                    "  {}: {} samples ({:.1}%)\n",
                    boundary_str, count, percentage
                ));
            }
        }

        result
    }
}

fn percentile_pair(samples: &VecDeque<u64>) -> (Option<u64>, Option<u64>) {
    if samples.is_empty() {
        return (None, None);
    }
    let mut v: Vec<u64> = samples.iter().copied().collect();
    v.sort_unstable();
    let idx =
        |pct: f64| -> usize { ((pct * ((v.len() - 1) as f64)).round() as usize).min(v.len() - 1) };
    (Some(v[idx(0.50)]), Some(v[idx(0.95)]))
}

// Global singleton (simple for FFI calls)
static GLOBAL_MANAGER: OnceCell<Arc<PushGatewayManager>> = OnceCell::new();

/// Initialize global manager (called by daemon setup)
pub fn install_global_manager(mgr: Arc<PushGatewayManager>) -> bool {
    GLOBAL_MANAGER.set(mgr).is_ok()
}

fn with_manager<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Arc<PushGatewayManager>) -> R,
{
    GLOBAL_MANAGER.get().map(f)
}

/// FFI: record a push wake event (debounced). Returns 0 success, >0 debounced, <0 error.
#[no_mangle]
pub extern "C" fn nyx_push_wake() -> i32 {
    with_manager(|m| match m.push_wake() {
        Ok(_) => 0,
        Err(PushGatewayError::Debounced) => 1,
        Err(_) => -1,
    })
    .unwrap_or(-2)
}

/// FFI: attempt resume (async dispatch). Returns immediately (0 queued / -1 error / -2 uninit).
#[no_mangle]
pub extern "C" fn nyx_resume_low_power_session() -> i32 {
    if let Some(m) = GLOBAL_MANAGER.get() {
        let m_clone = m.clone();
        // Spawn onto a default runtime (expect caller has a Tokio runtime installed)
        tokio::spawn(async move {
            let _ = m_clone.resume_low_power_session().await;
        });
        0
    } else {
        -2
    }
}

// --- Comprehensive Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};

    struct MockReconnector {
        attempts: AtomicU8,
        fail_until: u8,
        delay_ms: u64, // Simulate reconnection delay
    }

    impl MockReconnector {
        fn new(fail_until: u8, delay_ms: u64) -> Self {
            Self {
                attempts: AtomicU8::new(0),
                fail_until,
                delay_ms,
            }
        }
    }

    impl MinimalReconnector for MockReconnector {
        fn reconnect_minimal(&self) -> BoxFuture<'_, Result<(), String>> {
            let attempts_ref = &self.attempts;
            let fail_until = self.fail_until;
            let delay_ms = self.delay_ms;

            Box::pin(async move {
                // Simulate network delay
                if delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }

                let a = attempts_ref.fetch_add(1, Ordering::SeqCst) + 1;
                if a <= fail_until {
                    Err(format!("Simulated failure on attempt {}", a))
                } else {
                    Ok(())
                }
            })
        }
    }

    #[test]
    fn test_latency_histogram_basic() {
        let mut histogram = LatencyHistogram::default();

        // Test recording samples
        histogram.record_sample(150);
        histogram.record_sample(350);
        histogram.record_sample(1500);
        histogram.record_sample(25000); // High latency

        assert_eq!(histogram.total_samples, 4);

        // Test percentile calculations
        let p50 = histogram.calculate_percentile(0.50);
        let p95 = histogram.calculate_percentile(0.95);

        assert!(p50.is_some());
        assert!(p95.is_some());
        assert!(p50.unwrap() <= p95.unwrap());

        // Test mean calculation
        let mean = histogram.mean_latency_ms();
        assert!(mean.is_some());
        assert!(mean.unwrap() > 0.0);
    }

    #[test]
    fn test_jittered_backoff_basic() {
        let backoff = JitteredBackoff::default();

        let delay1 = backoff.calculate_delay(1);
        let delay2 = backoff.calculate_delay(2);
        let delay3 = backoff.calculate_delay(3);

        // Exponential growth (with jitter, so approximate)
        assert!(delay1 < delay2);
        assert!(delay2 < delay3);

        // Should respect maximum delay
        let high_attempt_delay = backoff.calculate_delay(20);
        assert!(high_attempt_delay <= backoff.max_delay * 2); // Allow for jitter
    }

    #[test]
    fn test_jittered_backoff_configuration() {
        let custom_backoff = JitteredBackoff::with_config(
            Duration::from_millis(100),
            Duration::from_secs(5),
            0.5, // 50% jitter
        );

        let delay = custom_backoff.calculate_delay(1);

        // Should be roughly around base delay (100ms) with jitter
        assert!(delay.as_millis() >= 50); // 50% below base
        assert!(delay.as_millis() <= 200); // 100% above base
    }

    #[tokio::test]
    async fn test_enhanced_retry_with_histogram() {
        let reconn = Arc::new(MockReconnector::new(2, 50)); // Fail first 2, with 50ms delay
        let mgr = PushGatewayManager::new(reconn);

        mgr.push_wake().unwrap();
        let result = mgr.resume_low_power_session().await;
        assert!(result.is_ok());

        let stats = mgr.stats();
        assert_eq!(stats.total_reconnect_failures, 2);
        assert_eq!(stats.total_reconnect_success, 1);
        assert_eq!(stats.total_reconnect_attempts, 3);

        // Verify histogram recorded the successful attempt
        assert!(stats.histogram_total_samples > 0);
        assert!(stats.histogram_p50_ms.is_some());
        assert!(stats.histogram_mean_ms.is_some());

        // Success rate should be 33.3%
        assert!((stats.success_rate_percent() - 33.33).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_debounce_behavior() {
        let reconn = Arc::new(MockReconnector::new(0, 0));
        let mgr = PushGatewayManager::new(reconn);

        // First wake should succeed
        assert!(mgr.push_wake().is_ok());

        // Immediate second wake should be debounced
        assert!(matches!(mgr.push_wake(), Err(PushGatewayError::Debounced)));

        let stats = mgr.stats();
        assert_eq!(stats.total_wake_events, 1);
        assert_eq!(stats.debounced_wake_events, 1);

        // Debounce rate should be 100% (1 debounced out of 1 total processed wake)
        let expected_rate =
            (stats.debounced_wake_events as f64 / stats.total_wake_events as f64) * 100.0;
        assert!((stats.debounce_rate_percent() - expected_rate).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_concurrent_reconnection_prevention() {
        let reconn = Arc::new(MockReconnector::new(0, 200)); // 200ms delay
        let mgr = PushGatewayManager::new(reconn);

        // Start first reconnection
        let mgr_clone = mgr.clone();
        let handle1 = tokio::spawn(async move { mgr_clone.resume_low_power_session().await });

        // Give first reconnection time to start
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Second reconnection should fail with AlreadyInProgress
        let result = mgr.resume_low_power_session().await;
        assert!(matches!(result, Err(PushGatewayError::AlreadyInProgress)));

        // First should complete successfully
        let first_result = handle1.await.unwrap();
        assert!(first_result.is_ok());
    }

    #[tokio::test]
    async fn test_histogram_bucket_distribution() {
        let mut histogram = LatencyHistogram::default();

        // Add samples across different buckets
        histogram.record_sample(5); // ≤10ms bucket
        histogram.record_sample(75); // ≤100ms bucket
        histogram.record_sample(750); // ≤1000ms bucket
        histogram.record_sample(3000); // ≤5000ms bucket
        histogram.record_sample(45000); // ≤60000ms bucket
        histogram.record_sample(70000); // overflow bucket

        let distribution = histogram.bucket_distribution();

        // Verify we have entries in expected buckets
        let mut bucket_10_count = 0;
        let mut bucket_100_count = 0;
        let mut bucket_1000_count = 0;
        let mut bucket_5000_count = 0;
        let mut bucket_60000_count = 0;
        let mut overflow_count = 0;

        for (boundary, count) in distribution {
            match boundary {
                10 => bucket_10_count = count,
                100 => bucket_100_count = count,
                1000 => bucket_1000_count = count,
                5000 => bucket_5000_count = count,
                60000 => bucket_60000_count = count,
                u64::MAX => overflow_count = count,
                _ => {} // Other buckets
            }
        }

        assert_eq!(bucket_10_count, 1);
        assert_eq!(bucket_100_count, 1);
        assert_eq!(bucket_1000_count, 1);
        assert_eq!(bucket_5000_count, 1);
        assert_eq!(bucket_60000_count, 1);
        assert_eq!(overflow_count, 1);

        assert_eq!(histogram.total_samples, 6);
    }

    #[tokio::test]
    async fn test_custom_backoff_configuration() {
        let custom_backoff = JitteredBackoff::with_config(
            Duration::from_millis(50),
            Duration::from_secs(2),
            0.1, // Low jitter for predictable testing
        );

        let reconn = Arc::new(MockReconnector::new(0, 10));
        let mgr = PushGatewayManager::with_config(
            reconn,
            Duration::from_millis(100), // Debounce
            3,                          // Max retries
            custom_backoff,
        );

        let stats = mgr.stats();
        assert_eq!(stats.total_wake_events, 0);

        // Test configuration took effect
        mgr.configure_backoff(Duration::from_millis(25), Duration::from_secs(1), 0.2);

        // Test reconnection with custom config
        let result = mgr.resume_low_power_session().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_histogram_reset_functionality() {
        let reconn = Arc::new(MockReconnector::new(0, 50));
        let mgr = PushGatewayManager::new(reconn);

        // Generate some histogram data
        mgr.resume_low_power_session().await.unwrap();

        let stats_before = mgr.stats();
        assert!(stats_before.histogram_total_samples > 0);

        // Reset histogram
        mgr.reset_histogram();

        let stats_after = mgr.stats();
        assert_eq!(stats_after.histogram_total_samples, 0);
        assert!(stats_after.histogram_p50_ms.is_none());
        assert!(stats_after.histogram_mean_ms.is_none());

        // Legacy metrics should remain intact
        assert_eq!(
            stats_after.total_reconnect_success,
            stats_before.total_reconnect_success
        );
        assert!(stats_after.p50_latency_ms.is_some()); // Ring buffer not reset
    }

    #[test]
    fn test_stats_calculation_methods() {
        let stats = PushGatewayStats {
            total_wake_events: 10,
            debounced_wake_events: 3,
            total_reconnect_attempts: 8,
            total_reconnect_failures: 2,
            total_reconnect_success: 6,

            avg_reconnect_latency_ms: Some(250),
            p50_latency_ms: Some(200),
            p95_latency_ms: Some(400),

            histogram_p50_ms: Some(190),
            histogram_p95_ms: Some(420),
            histogram_p99_ms: Some(500),
            histogram_mean_ms: Some(245.0),
            histogram_total_samples: 6,

            latency_buckets: vec![(100, 1), (500, 4), (1000, 1)],
        };

        assert!((stats.success_rate_percent() - 75.0).abs() < 0.1);
        assert!((stats.failure_rate_percent() - 25.0).abs() < 0.1);
        assert!((stats.debounce_rate_percent() - 30.0).abs() < 0.1);

        let distribution_str = stats.format_histogram_distribution();
        assert!(distribution_str.contains("≤100ms: 1 samples"));
        assert!(distribution_str.contains("≤500ms: 4 samples"));
        assert!(distribution_str.contains("≤1000ms: 1 samples"));
    }
}
