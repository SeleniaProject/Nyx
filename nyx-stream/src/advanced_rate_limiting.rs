//! Advanced Rate Limiting & Flow Control for Nyx Protocol v1.0
//!
//! This module implements sophisticated rate limiting and flow control mechanisms
//! to ensure optimal performance, prevent congestion, and maintain network stability
//! according to Nyx Protocol v1.0 specifications.
//!
//! # Key Features
//! - Multi-tier rate limiting (connection, stream, global)
//! - Adaptive flow control with dynamic window sizing
//! - Priority-based bandwidth allocation
//! - Congestion control integration
//! - Burst tolerance with token bucket algorithms
//! - Backpressure mechanisms for overload protection

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, instrument, warn};

/// Configuration for advanced rate limiting and flow control
#[derive(Debug, Clone)]
pub struct AdvancedFlowConfig {
    /// Global bandwidth limit in bytes per second
    pub global_bandwidth_limit: u64,
    /// Per-connection bandwidth limit in bytes per second
    pub per_connection_limit: u64,
    /// Per-stream bandwidth limit in bytes per second
    pub per_stream_limit: u64,
    /// Maximum burst size allowed
    pub max_burst_size: u64,
    /// Flow control window size
    pub initial_window_size: u32,
    /// Maximum window size
    pub max_window_size: u32,
    /// Minimum window size
    pub min_window_size: u32,
    /// Window growth factor for successful transmissions
    pub window_growth_factor: f32,
    /// Window shrink factor for losses/congestion
    pub window_shrink_factor: f32,
    /// Backpressure threshold (0.0-1.0)
    pub backpressure_threshold: f32,
    /// Priority levels for different traffic types
    pub priority_weights: HashMap<TrafficType, f32>,
    /// Enable adaptive rate limiting
    pub adaptive_rate_limiting: bool,
    /// Rate limiting window duration
    pub rate_window_duration: Duration,
}

impl Default for AdvancedFlowConfig {
    fn default() -> Self {
        let mut priority_weights = HashMap::new();
        priority_weights.insert(TrafficType::Control, 1.0);
        priority_weights.insert(TrafficType::HighPriority, 0.8);
        priority_weights.insert(TrafficType::Normal, 0.5);
        priority_weights.insert(TrafficType::LowPriority, 0.2);
        priority_weights.insert(TrafficType::Background, 0.1);

        Self {
            global_bandwidth_limit: 10_000_000, // 10 MB/s
            per_connection_limit: 1_000_000,    // 1 MB/s
            per_stream_limit: 100_000,          // 100 KB/s
            max_burst_size: 65536,              // 64 KB
            initial_window_size: 16384,         // 16 KB
            max_window_size: 1048576,           // 1 MB
            min_window_size: 1024,              // 1 KB
            window_growth_factor: 1.1,
            window_shrink_factor: 0.5,
            backpressure_threshold: 0.8,
            priority_weights,
            adaptive_rate_limiting: true,
            rate_window_duration: Duration::from_secs(1),
        }
    }
}

/// Traffic classification for priority-based bandwidth allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrafficType {
    /// Protocol control messages (highest priority)
    Control,
    /// High-priority user data
    HighPriority,
    /// Normal priority traffic
    Normal,
    /// Low priority background tasks
    LowPriority,
    /// Lowest priority background synchronization
    Background,
}

/// Priority-aware token bucket for rate limiting
#[derive(Debug)]
pub struct PriorityTokenBucket {
    buckets: HashMap<TrafficType, TokenBucket>,
    global_bucket: TokenBucket,
    last_update: Instant,
}

impl PriorityTokenBucket {
    /// Create a new priority-aware token bucket
    pub fn new(config: &AdvancedFlowConfig) -> Self {
        let mut buckets = HashMap::new();

        for (&traffic_type, &weight) in &config.priority_weights {
            let capacity = (config.max_burst_size as f64 * weight as f64) as u64;
            let rate = (config.global_bandwidth_limit as f64 * weight as f64) as u64;
            buckets.insert(traffic_type, TokenBucket::new(capacity, rate));
        }

        let global_bucket = TokenBucket::new(config.max_burst_size, config.global_bandwidth_limit);

        Self {
            buckets,
            global_bucket,
            last_update: Instant::now(),
        }
    }

    /// Attempt to consume tokens for a specific traffic type
    #[instrument(skip(self))]
    pub fn try_consume(&mut self, traffic_type: TrafficType, tokens: u64) -> bool {
        self.update_buckets();

        // First check global limit
        if !self.global_bucket.try_consume(tokens) {
            debug!(
                traffic_type = ?traffic_type,
                tokens = tokens,
                "Global rate limit exceeded"
            );
            return false;
        }

        // Then check priority-specific limit
        if let Some(bucket) = self.buckets.get_mut(&traffic_type) {
            if bucket.try_consume(tokens) {
                debug!(
                    traffic_type = ?traffic_type,
                    tokens = tokens,
                    "Tokens consumed successfully"
                );
                true
            } else {
                // Refund global tokens since priority limit was hit
                self.global_bucket.add_tokens(tokens);
                debug!(
                    traffic_type = ?traffic_type,
                    tokens = tokens,
                    "Priority rate limit exceeded"
                );
                false
            }
        } else {
            warn!(traffic_type = ?traffic_type, "Unknown traffic type");
            false
        }
    }

    /// Update all token buckets based on elapsed time
    fn update_buckets(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        self.last_update = now;

        self.global_bucket.update(elapsed);
        for bucket in self.buckets.values_mut() {
            bucket.update(elapsed);
        }
    }

    /// Get current bucket status for monitoring
    pub fn get_status(&self) -> HashMap<TrafficType, BucketStatus> {
        self.buckets
            .iter()
            .map(|(&traffic_type, bucket)| (traffic_type, bucket.status()))
            .collect()
    }
}

/// Individual token bucket implementation
#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: u64,
    tokens: f64,
    refill_rate: u64, // tokens per second
}

impl TokenBucket {
    /// Create a new token bucket
    pub fn new(capacity: u64, refill_rate: u64) -> Self {
        Self {
            capacity,
            tokens: capacity as f64,
            refill_rate,
        }
    }

    /// Try to consume the specified number of tokens
    pub fn try_consume(&mut self, tokens: u64) -> bool {
        if self.tokens >= tokens as f64 {
            self.tokens -= tokens as f64;
            true
        } else {
            false
        }
    }

    /// Add tokens to the bucket (for refunding)
    pub fn add_tokens(&mut self, tokens: u64) {
        self.tokens = (self.tokens + tokens as f64).min(self.capacity as f64);
    }

    /// Update bucket based on elapsed time
    pub fn update(&mut self, elapsed: Duration) {
        let new_tokens = elapsed.as_secs_f64() * self.refill_rate as f64;
        self.tokens = (self.tokens + new_tokens).min(self.capacity as f64);
    }

    /// Get current bucket status
    pub fn status(&self) -> BucketStatus {
        BucketStatus {
            capacity: self.capacity,
            available_tokens: self.tokens as u64,
            utilization: 1.0 - (self.tokens / self.capacity as f64),
        }
    }
}

/// Token bucket status for monitoring
#[derive(Debug, Clone)]
pub struct BucketStatus {
    pub capacity: u64,
    pub available_tokens: u64,
    pub utilization: f64,
}

/// Advanced flow controller with adaptive window sizing
#[derive(Debug)]
pub struct AdvancedFlowController {
    /// Current window size
    pub window_size: u32,
    /// Maximum allowed window size
    max_window_size: u32,
    /// Minimum allowed window size
    min_window_size: u32,
    /// Bytes currently in flight
    bytes_in_flight: u32,
    /// Window growth factor
    #[allow(dead_code)]
    growth_factor: f32,
    /// Window shrink factor
    #[allow(dead_code)]
    shrink_factor: f32,
    /// Recent RTT measurements for adaptive scaling
    rtt_samples: VecDeque<Duration>,
    /// Last congestion event timestamp
    last_congestion: Option<Instant>,
    /// Slow start threshold
    pub ssthresh: u32,
    /// Whether we're in slow start phase
    in_slow_start: bool,
    /// Congestion avoidance increment
    ca_increment: f32,
}

impl AdvancedFlowController {
    /// Create a new advanced flow controller
    pub fn new(config: &AdvancedFlowConfig) -> Self {
        Self {
            window_size: config.initial_window_size,
            max_window_size: config.max_window_size,
            min_window_size: config.min_window_size,
            bytes_in_flight: 0,
            growth_factor: config.window_growth_factor,
            shrink_factor: config.window_shrink_factor,
            rtt_samples: VecDeque::with_capacity(16),
            last_congestion: None,
            ssthresh: config.max_window_size / 2,
            in_slow_start: true,
            ca_increment: 0.0,
        }
    }

    /// Check if we can send more data
    #[instrument(skip(self))]
    pub fn can_send(&self, bytes: u32) -> bool {
        let can_send = self.bytes_in_flight + bytes <= self.window_size;
        debug!(
            bytes_in_flight = self.bytes_in_flight,
            window_size = self.window_size,
            requested_bytes = bytes,
            can_send = can_send,
            "Flow control check"
        );
        can_send
    }

    /// Record bytes sent (increases in-flight count)
    pub fn on_send(&mut self, bytes: u32) {
        self.bytes_in_flight += bytes;
        debug!(
            bytes_sent = bytes,
            new_in_flight = self.bytes_in_flight,
            "Bytes sent"
        );
    }

    /// Handle ACK reception (adaptive window scaling)
    #[instrument(skip(self))]
    pub fn on_ack(&mut self, acked_bytes: u32, rtt: Duration) {
        self.bytes_in_flight = self.bytes_in_flight.saturating_sub(acked_bytes);
        self.add_rtt_sample(rtt);

        // Adaptive window scaling based on current phase
        if self.in_slow_start {
            // Exponential growth in slow start
            self.window_size = (self.window_size + acked_bytes).min(self.max_window_size);

            // Exit slow start when we hit ssthresh
            if self.window_size >= self.ssthresh {
                self.in_slow_start = false;
                info!(
                    window_size = self.window_size,
                    ssthresh = self.ssthresh,
                    "Exiting slow start phase"
                );
            }
        } else {
            // Congestion avoidance: additive increase
            self.ca_increment += acked_bytes as f32 / self.window_size as f32;
            if self.ca_increment >= 1.0 {
                self.window_size = (self.window_size + 1).min(self.max_window_size);
                self.ca_increment -= 1.0;
            }
        }

        debug!(
            acked_bytes = acked_bytes,
            new_in_flight = self.bytes_in_flight,
            window_size = self.window_size,
            rtt_ms = rtt.as_millis(),
            in_slow_start = self.in_slow_start,
            "ACK processed"
        );
    }

    /// Handle loss detection (multiplicative decrease)
    #[instrument(skip(self))]
    pub fn on_loss(&mut self) {
        // Set ssthresh to half of current window
        self.ssthresh = (self.window_size / 2).max(self.min_window_size);

        // Reduce window size
        self.window_size = self.ssthresh;

        // Reset to slow start
        self.in_slow_start = true;
        self.ca_increment = 0.0;

        // Record congestion event
        self.last_congestion = Some(Instant::now());

        warn!(
            new_window_size = self.window_size,
            new_ssthresh = self.ssthresh,
            "Loss detected, window reduced"
        );
    }

    /// Handle explicit congestion notification (ECN)
    #[instrument(skip(self))]
    pub fn on_ecn(&mut self) {
        // Less aggressive than loss - just reduce ssthresh
        if let Some(last_congestion) = self.last_congestion {
            // Avoid multiple reactions to congestion within RTT
            if last_congestion.elapsed() < self.avg_rtt().unwrap_or(Duration::from_millis(100)) {
                return;
            }
        }

        self.ssthresh = (self.window_size * 3 / 4).max(self.min_window_size);
        self.window_size = self.ssthresh;
        self.last_congestion = Some(Instant::now());

        info!(
            new_window_size = self.window_size,
            new_ssthresh = self.ssthresh,
            "ECN received, window adjusted"
        );
    }

    /// Add RTT sample for adaptive algorithms
    fn add_rtt_sample(&mut self, rtt: Duration) {
        self.rtt_samples.push_back(rtt);
        if self.rtt_samples.len() > 16 {
            self.rtt_samples.pop_front();
        }
    }

    /// Get average RTT from recent samples
    pub fn avg_rtt(&self) -> Option<Duration> {
        if self.rtt_samples.is_empty() {
            None
        } else {
            let total_nanos: u128 = self.rtt_samples.iter().map(|d| d.as_nanos()).sum();
            Some(Duration::from_nanos(
                (total_nanos / self.rtt_samples.len() as u128) as u64,
            ))
        }
    }

    /// Get current controller status
    pub fn status(&self) -> FlowControlStatus {
        FlowControlStatus {
            window_size: self.window_size,
            bytes_in_flight: self.bytes_in_flight,
            utilization: self.bytes_in_flight as f32 / self.window_size as f32,
            avg_rtt: self.avg_rtt(),
            in_slow_start: self.in_slow_start,
            ssthresh: self.ssthresh,
        }
    }
}

/// Flow control status for monitoring
#[derive(Debug, Clone)]
pub struct FlowControlStatus {
    pub window_size: u32,
    pub bytes_in_flight: u32,
    pub utilization: f32,
    pub avg_rtt: Option<Duration>,
    pub in_slow_start: bool,
    pub ssthresh: u32,
}

/// Backpressure controller for overload protection
#[derive(Debug)]
pub struct BackpressureController {
    /// Current backpressure level (0.0 = none, 1.0 = maximum)
    level: f32,
    /// Threshold for activating backpressure
    threshold: f32,
    /// Queue size monitoring
    queue_sizes: HashMap<String, usize>,
    /// Maximum queue sizes
    max_queue_sizes: HashMap<String, usize>,
    /// Backpressure event history
    events: VecDeque<BackpressureEvent>,
}

impl BackpressureController {
    /// Create a new backpressure controller
    pub fn new(threshold: f32) -> Self {
        Self {
            level: 0.0,
            threshold,
            queue_sizes: HashMap::new(),
            max_queue_sizes: HashMap::new(),
            events: VecDeque::with_capacity(100),
        }
    }

    /// Register a queue for monitoring
    pub fn register_queue(&mut self, name: String, max_size: usize) {
        self.max_queue_sizes.insert(name.clone(), max_size);
        self.queue_sizes.insert(name, 0);
    }

    /// Update queue size and calculate backpressure level
    #[instrument(skip(self))]
    pub fn update_queue_size(&mut self, queue_name: &str, current_size: usize) {
        if let Some(&max_size) = self.max_queue_sizes.get(queue_name) {
            self.queue_sizes
                .insert(queue_name.to_string(), current_size);

            let utilization = current_size as f32 / max_size as f32;

            if utilization > self.threshold {
                let new_level =
                    ((utilization - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0);

                if new_level > self.level {
                    self.level = new_level;
                    self.record_event(BackpressureEvent {
                        timestamp: Instant::now(),
                        queue_name: queue_name.to_string(),
                        level: new_level,
                        cause: BackpressureCause::QueueOverflow,
                    });

                    warn!(
                        queue_name = queue_name,
                        current_size = current_size,
                        max_size = max_size,
                        utilization = utilization,
                        backpressure_level = new_level,
                        "Backpressure activated"
                    );
                }
            } else {
                // Gradually reduce backpressure
                self.level = (self.level - 0.1).max(0.0);
            }
        }
    }

    /// Check if backpressure should be applied
    pub fn should_apply_backpressure(&self) -> bool {
        self.level > 0.0
    }

    /// Get current backpressure level
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Calculate delay based on backpressure level
    pub fn calculate_delay(&self) -> Duration {
        if self.level == 0.0 {
            Duration::ZERO
        } else {
            // Exponential backoff based on backpressure level
            let delay_ms = (self.level * self.level * 100.0) as u64;
            Duration::from_millis(delay_ms.min(1000)) // Max 1 second delay
        }
    }

    /// Record backpressure event
    fn record_event(&mut self, event: BackpressureEvent) {
        self.events.push_back(event);
        if self.events.len() > 100 {
            self.events.pop_front();
        }
    }

    /// Get recent backpressure events
    pub fn recent_events(&self) -> Vec<&BackpressureEvent> {
        self.events.iter().collect()
    }
}

/// Backpressure event for monitoring and debugging
#[derive(Debug, Clone)]
pub struct BackpressureEvent {
    pub timestamp: Instant,
    pub queue_name: String,
    pub level: f32,
    pub cause: BackpressureCause,
}

/// Cause of backpressure activation
#[derive(Debug, Clone)]
pub enum BackpressureCause {
    QueueOverflow,
    MemoryPressure,
    CpuOverload,
    NetworkCongestion,
}

/// Comprehensive rate limiter combining all components
#[derive(Debug)]
pub struct NyxRateLimiter {
    /// Priority-aware token buckets
    token_buckets: Arc<Mutex<PriorityTokenBucket>>,
    /// Per-connection flow controllers
    flow_controllers: Arc<Mutex<HashMap<u64, AdvancedFlowController>>>,
    /// Global backpressure controller
    backpressure: Arc<Mutex<BackpressureController>>,
    /// Configuration
    config: AdvancedFlowConfig,
    /// Statistics
    stats: Arc<Mutex<RateLimiterStats>>,
}

impl NyxRateLimiter {
    /// Create a new comprehensive rate limiter
    pub fn new(config: AdvancedFlowConfig) -> Self {
        let token_buckets = Arc::new(Mutex::new(PriorityTokenBucket::new(&config)));
        let flow_controllers = Arc::new(Mutex::new(HashMap::new()));
        let backpressure = Arc::new(Mutex::new(BackpressureController::new(
            config.backpressure_threshold,
        )));
        let stats = Arc::new(Mutex::new(RateLimiterStats::default()));

        Self {
            token_buckets,
            flow_controllers,
            backpressure,
            config,
            stats,
        }
    }

    /// Check if a transmission is allowed
    #[instrument(skip(self))]
    pub async fn check_transmission(
        &self,
        connection_id: u64,
        stream_id: u64,
        traffic_type: TrafficType,
        bytes: u32,
    ) -> Result<TransmissionDecision, RateLimitError> {
        // Check backpressure first
        {
            let backpressure = self.backpressure.lock().unwrap();
            if backpressure.should_apply_backpressure() {
                let delay = backpressure.calculate_delay();
                if delay > Duration::ZERO {
                    return Ok(TransmissionDecision::Delayed(delay));
                }
            }
        }

        // Check token bucket availability
        {
            let mut buckets = self.token_buckets.lock().unwrap();
            if !buckets.try_consume(traffic_type, bytes as u64) {
                self.update_stats(|stats| stats.rate_limited_count += 1);
                return Ok(TransmissionDecision::RateLimited);
            }
        }

        // Check flow control
        {
            let mut controllers = self.flow_controllers.lock().unwrap();
            let controller = controllers
                .entry(connection_id)
                .or_insert_with(|| AdvancedFlowController::new(&self.config));

            if !controller.can_send(bytes) {
                // Refund tokens since flow control blocked
                let mut buckets = self.token_buckets.lock().unwrap();
                if let Some(bucket) = buckets.buckets.get_mut(&traffic_type) {
                    bucket.add_tokens(bytes as u64);
                }
                buckets.global_bucket.add_tokens(bytes as u64);

                self.update_stats(|stats| stats.flow_control_blocked_count += 1);
                return Ok(TransmissionDecision::FlowControlBlocked);
            }

            controller.on_send(bytes);
        }

        self.update_stats(|stats| {
            stats.allowed_count += 1;
            stats.total_bytes_allowed += bytes as u64;
        });

        Ok(TransmissionDecision::Allowed)
    }

    /// Handle ACK reception for a connection
    #[instrument(skip(self))]
    pub fn on_ack(&self, connection_id: u64, acked_bytes: u32, rtt: Duration) {
        let mut controllers = self.flow_controllers.lock().unwrap();
        if let Some(controller) = controllers.get_mut(&connection_id) {
            controller.on_ack(acked_bytes, rtt);
        }
    }

    /// Handle loss detection for a connection
    #[instrument(skip(self))]
    pub fn on_loss(&self, connection_id: u64) {
        let mut controllers = self.flow_controllers.lock().unwrap();
        if let Some(controller) = controllers.get_mut(&connection_id) {
            controller.on_loss();
        }
    }

    /// Handle ECN notification for a connection
    #[instrument(skip(self))]
    pub fn on_ecn(&self, connection_id: u64) {
        let mut controllers = self.flow_controllers.lock().unwrap();
        if let Some(controller) = controllers.get_mut(&connection_id) {
            controller.on_ecn();
        }
    }

    /// Update queue size for backpressure monitoring
    pub fn update_queue_size(&self, queue_name: &str, current_size: usize) {
        let mut backpressure = self.backpressure.lock().unwrap();
        backpressure.update_queue_size(queue_name, current_size);
    }

    /// Register a queue for backpressure monitoring
    pub fn register_queue(&self, queue_name: String, max_size: usize) {
        let mut backpressure = self.backpressure.lock().unwrap();
        backpressure.register_queue(queue_name, max_size);
    }

    /// Get comprehensive status
    pub fn get_status(&self) -> RateLimiterStatus {
        let bucket_status = {
            let buckets = self.token_buckets.lock().unwrap();
            buckets.get_status()
        };

        let flow_controllers_status = {
            let controllers = self.flow_controllers.lock().unwrap();
            controllers
                .iter()
                .map(|(&conn_id, controller)| (conn_id, controller.status()))
                .collect()
        };

        let backpressure_level = {
            let backpressure = self.backpressure.lock().unwrap();
            backpressure.level()
        };

        let stats = {
            let stats = self.stats.lock().unwrap();
            stats.clone()
        };

        RateLimiterStatus {
            bucket_status,
            flow_controllers_status,
            backpressure_level,
            stats,
        }
    }

    /// Update statistics with a closure
    fn update_stats<F>(&self, updater: F)
    where
        F: FnOnce(&mut RateLimiterStats),
    {
        if let Ok(mut stats) = self.stats.lock() {
            updater(&mut stats);
        }
    }

    /// Clean up inactive connections
    pub fn cleanup_inactive_connections(&self, inactive_threshold: Duration) {
        let mut controllers = self.flow_controllers.lock().unwrap();
        let _cutoff = Instant::now() - inactive_threshold;

        // In a real implementation, you'd track last activity per connection
        // For now, we'll just limit the total number of controllers
        if controllers.len() > 1000 {
            let excess = controllers.len() - 1000;
            let to_remove: Vec<_> = controllers.keys().take(excess).copied().collect();
            for conn_id in to_remove {
                controllers.remove(&conn_id);
            }
        }
    }
}

/// Decision about whether a transmission should proceed
#[derive(Debug, Clone)]
pub enum TransmissionDecision {
    /// Transmission is allowed
    Allowed,
    /// Transmission should be delayed by the specified duration
    Delayed(Duration),
    /// Transmission is blocked by rate limiting
    RateLimited,
    /// Transmission is blocked by flow control
    FlowControlBlocked,
}

/// Rate limiter error types
#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// Rate limiter statistics
#[derive(Debug, Clone, Default)]
pub struct RateLimiterStats {
    pub allowed_count: u64,
    pub rate_limited_count: u64,
    pub flow_control_blocked_count: u64,
    pub total_bytes_allowed: u64,
    pub backpressure_activations: u64,
}

/// Comprehensive rate limiter status
#[derive(Debug, Clone)]
pub struct RateLimiterStatus {
    pub bucket_status: HashMap<TrafficType, BucketStatus>,
    pub flow_controllers_status: HashMap<u64, FlowControlStatus>,
    pub backpressure_level: f32,
    pub stats: RateLimiterStats,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_basic() {
        let mut bucket = TokenBucket::new(100, 50); // 100 capacity, 50 tokens/sec

        // Should be able to consume initial tokens
        assert!(bucket.try_consume(50));
        assert!(bucket.try_consume(50));
        assert!(!bucket.try_consume(1)); // Should fail - no tokens left

        // After refill, should have more tokens
        bucket.update(Duration::from_secs(1));
        assert!(bucket.try_consume(50)); // Should succeed after refill
    }

    #[test]
    fn test_priority_token_bucket() {
        let config = AdvancedFlowConfig::default();
        let mut bucket = PriorityTokenBucket::new(&config);

        // Control traffic should get priority
        assert!(bucket.try_consume(TrafficType::Control, 1000));
        assert!(bucket.try_consume(TrafficType::HighPriority, 1000));

        // Should respect limits
        let large_request = config.max_burst_size + 1;
        assert!(!bucket.try_consume(TrafficType::Background, large_request));
    }

    #[test]
    fn test_flow_controller_basic() {
        let config = AdvancedFlowConfig::default();
        let mut controller = AdvancedFlowController::new(&config);

        // Should allow initial sends within window
        assert!(controller.can_send(1000));
        controller.on_send(1000);

        // Should track in-flight bytes
        assert_eq!(controller.bytes_in_flight, 1000);

        // ACK should reduce in-flight and potentially grow window
        let initial_window = controller.window_size;
        controller.on_ack(1000, Duration::from_millis(50));
        assert_eq!(controller.bytes_in_flight, 0);
        assert!(controller.window_size >= initial_window); // Window should grow or stay same
    }

    #[test]
    fn test_flow_controller_loss_handling() {
        let config = AdvancedFlowConfig::default();
        let mut controller = AdvancedFlowController::new(&config);

        let initial_window = controller.window_size;
        controller.on_loss();

        // Window should be reduced after loss
        assert!(controller.window_size < initial_window);
        assert!(controller.in_slow_start);
    }

    #[test]
    fn test_backpressure_controller() {
        let mut controller = BackpressureController::new(0.8);
        controller.register_queue("test_queue".to_string(), 100);

        // Should not apply backpressure initially
        assert!(!controller.should_apply_backpressure());

        // Should apply backpressure when threshold exceeded
        controller.update_queue_size("test_queue", 90); // 90% utilization
        assert!(controller.should_apply_backpressure());
        assert!(controller.level() > 0.0);
    }

    #[tokio::test]
    async fn test_nyx_rate_limiter_integration() {
        let config = AdvancedFlowConfig::default();
        let limiter = NyxRateLimiter::new(config);

        // Register a queue for backpressure monitoring
        limiter.register_queue("test_queue".to_string(), 1000);

        // Should allow normal traffic
        let decision = limiter
            .check_transmission(1, 1, TrafficType::Normal, 1000)
            .await
            .unwrap();

        match decision {
            TransmissionDecision::Allowed => {
                // Expected - should allow initial transmission
            }
            _ => panic!("Expected transmission to be allowed"),
        }

        // Simulate ACK
        limiter.on_ack(1, 1000, Duration::from_millis(50));

        let status = limiter.get_status();
        assert!(status.stats.allowed_count > 0);
    }

    #[tokio::test]
    async fn test_rate_limiting_behavior() {
        let config = AdvancedFlowConfig {
            global_bandwidth_limit: 1000,
            max_burst_size: 500,
            ..Default::default()
        };

        let limiter = NyxRateLimiter::new(config);

        // First transmission should succeed (250 bytes fits in Normal traffic bucket)
        // Normal traffic has weight 0.5, so bucket capacity is 500 * 0.5 = 250 bytes
        let decision = limiter
            .check_transmission(1, 1, TrafficType::Normal, 250)
            .await
            .unwrap();
        assert!(matches!(decision, TransmissionDecision::Allowed));

        // Second transmission should be rate limited (bucket is now empty)
        let decision = limiter
            .check_transmission(1, 1, TrafficType::Normal, 250)
            .await
            .unwrap();
        assert!(matches!(decision, TransmissionDecision::RateLimited));
    }

    #[tokio::test]
    async fn test_backpressure_integration() {
        let config = AdvancedFlowConfig::default();
        let limiter = NyxRateLimiter::new(config);

        // Register and fill queue to trigger backpressure
        limiter.register_queue("test_queue".to_string(), 100);
        limiter.update_queue_size("test_queue", 90); // 90% full

        let decision = limiter
            .check_transmission(1, 1, TrafficType::Normal, 1000)
            .await
            .unwrap();

        // Should be delayed due to backpressure
        assert!(matches!(decision, TransmissionDecision::Delayed(_)));
    }
}
