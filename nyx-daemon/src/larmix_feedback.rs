//! LARMix++ Feedback Loop Implementation
//!
//! Implements the latency-aware routing feedback mechanism for dynamic hop count
//! adjustment and path quality optimization as specified in §4.2 of the design document.
//!
//! # Responsibilities
//! - Collect transport metrics from path validation probes
//! - Feed metrics to PathBuilder for routing decisions
//! - Dynamically adjust hop count based on network conditions
//! - Detect path degradation and trigger failover

use nyx_transport::path_validation::{PathMetrics as TransportPathMetrics, PathValidator};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, trace, warn};

/// LARMix++ errors
#[derive(Debug, Error)]
pub enum LarmixError {
    #[error("Path not found: {0}")]
    PathNotFound(String),
    #[error("Invalid hop count: {0}")]
    InvalidHopCount(usize),
    #[error("Metrics error: {0}")]
    MetricsError(String),
}

/// LARMix++ configuration
#[derive(Debug, Clone)]
pub struct LarmixConfig {
    /// Minimum hop count (default: 3)
    pub min_hop_count: usize,
    /// Maximum hop count (default: 7)
    pub max_hop_count: usize,
    /// Target latency threshold in ms (default: 200ms)
    pub target_latency_ms: u64,
    /// Packet loss threshold for degradation (default: 5%)
    pub loss_threshold: f64,
    /// Bandwidth degradation threshold (default: 50%)
    pub bandwidth_degradation_threshold: f64,
    /// Metrics update interval (default: 5s)
    pub metrics_update_interval: Duration,
    /// Path degradation detection window (default: 10s)
    pub degradation_window: Duration,
}

impl Default for LarmixConfig {
    fn default() -> Self {
        Self {
            min_hop_count: 3,
            max_hop_count: 7,
            target_latency_ms: 200,
            loss_threshold: 0.05, // 5%
            bandwidth_degradation_threshold: 0.5, // 50% drop
            metrics_update_interval: Duration::from_secs(5),
            degradation_window: Duration::from_secs(10),
        }
    }
}

/// Path state for feedback loop tracking
#[derive(Debug, Clone)]
struct PathState {
    /// Current hop count
    hop_count: usize,
    /// Historical metrics for trend analysis
    metrics_history: Vec<(Instant, TransportPathMetrics)>,
    /// Baseline bandwidth for degradation detection
    baseline_bandwidth: u64,
    /// Last adjustment time
    last_adjustment: Instant,
}

/// LARMix++ Feedback Loop Manager
pub struct LarmixFeedbackLoop {
    config: LarmixConfig,
    path_states: Arc<RwLock<HashMap<SocketAddr, PathState>>>,
    path_validator: Option<Arc<PathValidator>>,
    metrics: Arc<RwLock<LarmixMetrics>>,
}

/// LARMix++ metrics
#[derive(Debug, Clone, Default)]
pub struct LarmixMetrics {
    pub total_adjustments: u64,
    pub hop_increases: u64,
    pub hop_decreases: u64,
    pub degradation_events: u64,
    pub failovers: u64,
}

impl LarmixFeedbackLoop {
    /// Create new LARMix++ feedback loop
    pub fn new(config: LarmixConfig) -> Self {
        Self {
            config,
            path_states: Arc::new(RwLock::new(HashMap::new())),
            path_validator: None,
            metrics: Arc::new(RwLock::new(LarmixMetrics::default())),
        }
    }

    /// Set path validator reference
    pub fn set_path_validator(&mut self, validator: Arc<PathValidator>) {
        self.path_validator = Some(validator);
    }

    /// Start the feedback loop
    pub async fn start(self: Arc<Self>) {
        info!("Starting LARMix++ feedback loop");

        // Spawn metrics collection task
        let manager = self.clone();
        tokio::spawn(async move {
            manager.metrics_collection_loop().await;
        });

        // Spawn hop count adjustment task
        let manager = self.clone();
        tokio::spawn(async move {
            manager.hop_adjustment_loop().await;
        });

        // Spawn degradation detection task
        let manager = self.clone();
        tokio::spawn(async move {
            manager.degradation_detection_loop().await;
        });
    }

    /// Metrics collection loop
    ///
    /// Periodically collects metrics from path validator and updates path states
    async fn metrics_collection_loop(&self) {
        let mut interval_timer = interval(self.config.metrics_update_interval);

        loop {
            interval_timer.tick().await;

            if let Some(ref validator) = self.path_validator {
                // Get all path metrics from validator
                let metrics_map = validator.get_all_path_metrics();
                let mut states = self.path_states.write().await;

                for (path_addr, transport_metrics) in metrics_map {
                    // Update or create path state
                    let state = states.entry(path_addr).or_insert_with(|| PathState {
                        hop_count: self.config.min_hop_count,
                        metrics_history: Vec::new(),
                        baseline_bandwidth: transport_metrics.bandwidth_estimate,
                        last_adjustment: Instant::now(),
                    });

                    // Add to history (keep last 20 samples)
                    state.metrics_history.push((Instant::now(), transport_metrics.clone()));
                    if state.metrics_history.len() > 20 {
                        state.metrics_history.remove(0);
                    }

                    // Update baseline bandwidth if significantly improved
                    if transport_metrics.bandwidth_estimate > state.baseline_bandwidth * 2 {
                        state.baseline_bandwidth = transport_metrics.bandwidth_estimate;
                        debug!(
                            "Updated baseline bandwidth for path {} to {} bytes/sec",
                            path_addr, state.baseline_bandwidth
                        );
                    }

                    trace!(
                        "Updated metrics for path {}: RTT={:?}, Loss={:.2}%, BW={} bytes/sec",
                        path_addr,
                        transport_metrics.round_trip_time,
                        transport_metrics.packet_loss_rate * 100.0,
                        transport_metrics.bandwidth_estimate
                    );
                }
            }
        }
    }

    /// Hop count adjustment loop
    ///
    /// Dynamically adjusts hop count based on latency and network conditions
    async fn hop_adjustment_loop(&self) {
        let mut interval_timer = interval(Duration::from_secs(10));

        loop {
            interval_timer.tick().await;

            let mut states = self.path_states.write().await;

            for (path_addr, state) in states.iter_mut() {
                // Only adjust if enough time has passed since last adjustment
                if state.last_adjustment.elapsed() < Duration::from_secs(30) {
                    continue;
                }

                // Calculate average latency from recent history
                if state.metrics_history.is_empty() {
                    continue;
                }

                let avg_latency = {
                    let total: Duration = state
                        .metrics_history
                        .iter()
                        .map(|(_, m)| m.round_trip_time)
                        .sum();
                    total / state.metrics_history.len() as u32
                };

                let target_latency = Duration::from_millis(self.config.target_latency_ms);

                // Decide on hop count adjustment
                let new_hop_count = if avg_latency > target_latency * 2 {
                    // High latency: decrease hops to reduce routing overhead
                    if state.hop_count > self.config.min_hop_count {
                        state.hop_count - 1
                    } else {
                        state.hop_count
                    }
                } else if avg_latency < target_latency / 2 {
                    // Low latency: can afford more hops for better anonymity
                    if state.hop_count < self.config.max_hop_count {
                        state.hop_count + 1
                    } else {
                        state.hop_count
                    }
                } else {
                    state.hop_count // Keep current
                };

                if new_hop_count != state.hop_count {
                    info!(
                        "Adjusting hop count for path {} from {} to {} (avg latency: {:?})",
                        path_addr, state.hop_count, new_hop_count, avg_latency
                    );

                    // Update metrics
                    let mut metrics = self.metrics.write().await;
                    metrics.total_adjustments += 1;
                    if new_hop_count > state.hop_count {
                        metrics.hop_increases += 1;
                    } else {
                        metrics.hop_decreases += 1;
                    }

                    state.hop_count = new_hop_count;
                    state.last_adjustment = Instant::now();
                }
            }
        }
    }

    /// Path degradation detection loop
    ///
    /// Monitors paths for quality degradation and triggers failover events
    async fn degradation_detection_loop(&self) {
        let mut interval_timer = interval(Duration::from_secs(5));

        loop {
            interval_timer.tick().await;

            let states = self.path_states.read().await;
            let degradation_window = self.config.degradation_window;

            for (path_addr, state) in states.iter() {
                // Check recent metrics within degradation window
                let recent_metrics: Vec<_> = state
                    .metrics_history
                    .iter()
                    .filter(|(timestamp, _)| timestamp.elapsed() < degradation_window)
                    .map(|(_, m)| m)
                    .collect();

                if recent_metrics.is_empty() {
                    continue;
                }

                // Calculate average packet loss
                let avg_loss: f64 = recent_metrics.iter().map(|m| m.packet_loss_rate).sum::<f64>()
                    / recent_metrics.len() as f64;

                // Calculate average bandwidth
                let avg_bandwidth: u64 = recent_metrics
                    .iter()
                    .map(|m| m.bandwidth_estimate)
                    .sum::<u64>()
                    / recent_metrics.len() as u64;

                // Detect degradation conditions
                let excessive_loss = avg_loss > self.config.loss_threshold;
                let bandwidth_degraded = avg_bandwidth
                    < (state.baseline_bandwidth as f64
                        * self.config.bandwidth_degradation_threshold) as u64;

                if excessive_loss || bandwidth_degraded {
                    warn!(
                        "Path {} degraded - Loss: {:.2}%, Bandwidth: {} bytes/sec (baseline: {})",
                        path_addr,
                        avg_loss * 100.0,
                        avg_bandwidth,
                        state.baseline_bandwidth
                    );

                    // Update metrics
                    let mut metrics = self.metrics.write().await;
                    metrics.degradation_events += 1;

                    // Trigger failover event (to be handled by multipath scheduler)
                    // In a full implementation, this would notify the PathScheduler
                    // to prefer other paths and potentially remove this path
                }
            }
        }
    }

    /// Get hop count for a specific path
    pub async fn get_hop_count(&self, path_addr: SocketAddr) -> Result<usize, LarmixError> {
        let states = self.path_states.read().await;
        states
            .get(&path_addr)
            .map(|s| s.hop_count)
            .ok_or(LarmixError::PathNotFound(path_addr.to_string()))
    }

    /// Register a new path for tracking
    pub async fn register_path(&self, path_addr: SocketAddr) {
        let mut states = self.path_states.write().await;
        states.entry(path_addr).or_insert_with(|| PathState {
            hop_count: self.config.min_hop_count,
            metrics_history: Vec::new(),
            baseline_bandwidth: 1_000_000, // 1 MB/s default
            last_adjustment: Instant::now(),
        });
        debug!("Registered path {} for LARMix++ tracking", path_addr);
    }

    /// Unregister a path
    pub async fn unregister_path(&self, path_addr: SocketAddr) {
        let mut states = self.path_states.write().await;
        if states.remove(&path_addr).is_some() {
            debug!("Unregistered path {} from LARMix++ tracking", path_addr);
        }
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> LarmixMetrics {
        self.metrics.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feedback_loop_creation() {
        let config = LarmixConfig::default();
        let feedback_loop = LarmixFeedbackLoop::new(config);

        assert_eq!(feedback_loop.config.min_hop_count, 3);
        assert_eq!(feedback_loop.config.max_hop_count, 7);
    }

    #[tokio::test]
    async fn test_path_registration() {
        let feedback_loop = Arc::new(LarmixFeedbackLoop::new(LarmixConfig::default()));

        let path_addr = "127.0.0.1:8080".parse().unwrap();
        feedback_loop.register_path(path_addr).await;

        let hop_count = feedback_loop.get_hop_count(path_addr).await.unwrap();
        assert_eq!(hop_count, 3); // min_hop_count default
    }

    #[tokio::test]
    async fn test_path_unregistration() {
        let feedback_loop = Arc::new(LarmixFeedbackLoop::new(LarmixConfig::default()));

        let path_addr = "127.0.0.1:8080".parse().unwrap();
        feedback_loop.register_path(path_addr).await;
        feedback_loop.unregister_path(path_addr).await;

        let result = feedback_loop.get_hop_count(path_addr).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_metrics() {
        let feedback_loop = Arc::new(LarmixFeedbackLoop::new(LarmixConfig::default()));

        let metrics = feedback_loop.get_metrics().await;
        assert_eq!(metrics.total_adjustments, 0);
        assert_eq!(metrics.hop_increases, 0);
        assert_eq!(metrics.hop_decreases, 0);
    }

    #[test]
    fn test_config_defaults() {
        let config = LarmixConfig::default();
        assert_eq!(config.min_hop_count, 3);
        assert_eq!(config.max_hop_count, 7);
        assert_eq!(config.target_latency_ms, 200);
        assert_eq!(config.loss_threshold, 0.05);
        assert_eq!(config.bandwidth_degradation_threshold, 0.5);
    }

    #[test]
    fn test_config_custom() {
        let config = LarmixConfig {
            min_hop_count: 5,
            max_hop_count: 10,
            target_latency_ms: 100,
            ..Default::default()
        };
        assert_eq!(config.min_hop_count, 5);
        assert_eq!(config.max_hop_count, 10);
        assert_eq!(config.target_latency_ms, 100);
    }

    #[tokio::test]
    async fn test_hop_count_retrieval_for_unregistered_path() {
        let feedback_loop = Arc::new(LarmixFeedbackLoop::new(LarmixConfig::default()));
        let path_addr = "127.0.0.1:9090".parse().unwrap();

        let result = feedback_loop.get_hop_count(path_addr).await;
        assert!(matches!(result, Err(LarmixError::PathNotFound(_))));
    }

    #[tokio::test]
    async fn test_multiple_path_registration() {
        let feedback_loop = Arc::new(LarmixFeedbackLoop::new(LarmixConfig::default()));

        let path1 = "127.0.0.1:8001".parse().unwrap();
        let path2 = "127.0.0.1:8002".parse().unwrap();
        let path3 = "127.0.0.1:8003".parse().unwrap();

        feedback_loop.register_path(path1).await;
        feedback_loop.register_path(path2).await;
        feedback_loop.register_path(path3).await;

        assert_eq!(feedback_loop.get_hop_count(path1).await.unwrap(), 3);
        assert_eq!(feedback_loop.get_hop_count(path2).await.unwrap(), 3);
        assert_eq!(feedback_loop.get_hop_count(path3).await.unwrap(), 3);
    }

    #[tokio::test]
    async fn test_metrics_tracking() {
        let feedback_loop = Arc::new(LarmixFeedbackLoop::new(LarmixConfig::default()));

        let metrics_before = feedback_loop.get_metrics().await;
        assert_eq!(metrics_before.total_adjustments, 0);
        assert_eq!(metrics_before.hop_increases, 0);
        assert_eq!(metrics_before.hop_decreases, 0);
        assert_eq!(metrics_before.degradation_events, 0);
        assert_eq!(metrics_before.failovers, 0);
    }

    #[test]
    fn test_config_validation() {
        let config = LarmixConfig {
            min_hop_count: 3,
            max_hop_count: 7,
            target_latency_ms: 200,
            loss_threshold: 0.05,
            bandwidth_degradation_threshold: 0.5,
            ..Default::default()
        };

        assert!(config.min_hop_count < config.max_hop_count);
        assert!(config.loss_threshold > 0.0 && config.loss_threshold < 1.0);
        assert!(
            config.bandwidth_degradation_threshold > 0.0
                && config.bandwidth_degradation_threshold < 1.0
        );
    }
}
