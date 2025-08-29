//! Advanced Path Selection Algorithms and Load Balancing for Nyx Protocol v1.0
//!
//! This module implements sophisticated path selection strategies that go beyond
//! simple weighted round-robin to provide optimal load distribution, latency-aware
//! routing, and adaptive performance optimization.
//!
//! ## Key Features
//!
//! - **Multi-Algorithm Support**: Round-robin, weighted, latency-based, and hybrid algorithms
//! - **Dynamic Metrics Collection**: Real-time RTT, bandwidth, and loss rate monitoring
//! - **Adaptive Load Balancing**: Automatic adjustment based on path performance
//! - **Congestion Avoidance**: Smart path selection to avoid overloaded routes
//! - **Failover Support**: Automatic exclusion of failed or degraded paths
//! - **Telemetry Integration**: Comprehensive monitoring and debugging support

#![forbid(unsafe_code)]

use crate::errors::{Error, Result};
use crate::multipath::scheduler::{PathId, PathMetric};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, info, trace, warn};

/// Maximum number of RTT samples to maintain per path for statistics
const MAX_RTT_SAMPLES: usize = 100;

/// Loss rate penalty exponential factor
const LOSS_PENALTY_DECAY: f64 = 0.95;

/// Path selection algorithm types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PathSelectionAlgorithm {
    /// Simple round-robin without weights
    RoundRobin,
    /// Weighted round-robin based on configured weights
    WeightedRoundRobin,
    /// Latency-based selection (prefer lower RTT paths)
    LatencyBased,
    /// Loss-aware selection (avoid high-loss paths)
    LossAware,
    /// Bandwidth-based selection (prefer higher bandwidth paths)
    BandwidthBased,
    /// Hybrid algorithm combining multiple metrics
    Hybrid {
        latency_weight: f64,
        loss_weight: f64,
        bandwidth_weight: f64,
    },
    /// Adaptive algorithm that changes strategy based on conditions
    Adaptive,
}

impl Default for PathSelectionAlgorithm {
    fn default() -> Self {
        Self::Hybrid {
            latency_weight: 0.4,
            loss_weight: 0.3,
            bandwidth_weight: 0.3,
        }
    }
}

/// Comprehensive path statistics for advanced decision making
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStatistics {
    /// Path identifier
    pub path_id: PathId,
    /// Base weight from configuration
    pub base_weight: f64,
    /// Current dynamic weight
    pub current_weight: f64,
    /// RTT statistics
    pub rtt_stats: RttStatistics,
    /// Loss rate statistics
    pub loss_stats: LossStatistics,
    /// Bandwidth statistics
    pub bandwidth_stats: BandwidthStatistics,
    /// Congestion metrics
    pub congestion_metrics: CongestionMetrics,
    /// Path availability
    pub is_available: bool,
    /// Last update timestamp
    pub last_update: SystemTime,
}

/// RTT statistics with comprehensive metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RttStatistics {
    /// Current EWMA RTT
    pub ewma_rtt: Duration,
    /// Minimum observed RTT
    pub min_rtt: Duration,
    /// Maximum observed RTT
    pub max_rtt: Duration,
    /// RTT variance for jitter calculation
    pub rtt_variance: f64,
    /// Recent RTT samples for detailed analysis
    pub recent_samples: VecDeque<Duration>,
    /// Sample count
    pub sample_count: u64,
}

impl Default for RttStatistics {
    fn default() -> Self {
        Self {
            ewma_rtt: Duration::from_millis(100),
            min_rtt: Duration::MAX,
            max_rtt: Duration::ZERO,
            rtt_variance: 0.0,
            recent_samples: VecDeque::with_capacity(MAX_RTT_SAMPLES),
            sample_count: 0,
        }
    }
}

/// Loss rate statistics with trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LossStatistics {
    /// Current loss rate (0.0-1.0)
    pub current_loss_rate: f64,
    /// EWMA loss rate
    pub ewma_loss_rate: f64,
    /// Total packets sent
    pub packets_sent: u64,
    /// Total packets lost
    pub packets_lost: u64,
    /// Loss penalty factor
    pub loss_penalty: f64,
}

impl Default for LossStatistics {
    fn default() -> Self {
        Self {
            current_loss_rate: 0.0,
            ewma_loss_rate: 0.0,
            packets_sent: 0,
            packets_lost: 0,
            loss_penalty: 1.0,
        }
    }
}

/// Bandwidth statistics and utilization tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthStatistics {
    /// Estimated bandwidth (bytes/second)
    pub estimated_bandwidth: f64,
    /// Current utilization (0.0-1.0)
    pub utilization: f64,
    /// Peak observed bandwidth
    pub peak_bandwidth: f64,
    /// Recent throughput samples
    pub throughput_samples: VecDeque<f64>,
}

impl Default for BandwidthStatistics {
    fn default() -> Self {
        Self {
            estimated_bandwidth: 1_000_000.0, // 1 Mbps default
            utilization: 0.0,
            peak_bandwidth: 0.0,
            throughput_samples: VecDeque::with_capacity(50),
        }
    }
}

/// Congestion detection and metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CongestionMetrics {
    /// Current congestion level (0.0-1.0)
    pub congestion_level: f64,
    /// Packets per second rate
    pub packet_rate: f64,
    /// Queue delay indicator
    pub queue_delay: Duration,
    /// Congestion events count
    pub congestion_events: u64,
}

impl Default for CongestionMetrics {
    fn default() -> Self {
        Self {
            congestion_level: 0.0,
            packet_rate: 0.0,
            queue_delay: Duration::ZERO,
            congestion_events: 0,
        }
    }
}

/// Configuration for advanced path selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedPathSelectionConfig {
    /// Primary algorithm to use
    pub algorithm: PathSelectionAlgorithm,
    /// Fallback algorithm if primary fails
    pub fallback_algorithm: PathSelectionAlgorithm,
    /// Enable adaptive behavior
    pub enable_adaptive: bool,
    /// RTT measurement configuration
    pub rtt_config: RttMeasurementConfig,
    /// Load balancing parameters
    pub load_balancing: LoadBalancingConfig,
    /// Failover configuration
    pub failover: FailoverConfig,
}

impl Default for AdvancedPathSelectionConfig {
    fn default() -> Self {
        Self {
            algorithm: PathSelectionAlgorithm::default(),
            fallback_algorithm: PathSelectionAlgorithm::WeightedRoundRobin,
            enable_adaptive: true,
            rtt_config: RttMeasurementConfig::default(),
            load_balancing: LoadBalancingConfig::default(),
            failover: FailoverConfig::default(),
        }
    }
}

/// RTT measurement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RttMeasurementConfig {
    /// EWMA smoothing factor (0.0-1.0)
    pub ewma_alpha: f64,
    /// Maximum RTT variance threshold
    pub max_variance_threshold: f64,
    /// Probe interval for active measurements
    pub probe_interval: Duration,
}

impl Default for RttMeasurementConfig {
    fn default() -> Self {
        Self {
            ewma_alpha: 0.875,
            max_variance_threshold: 50.0,
            probe_interval: Duration::from_secs(1),
        }
    }
}

/// Load balancing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadBalancingConfig {
    /// Maximum load imbalance threshold (0.0-1.0)
    pub max_imbalance_threshold: f64,
    /// Rebalancing interval
    pub rebalance_interval: Duration,
    /// Enable proactive load balancing
    pub enable_proactive: bool,
}

impl Default for LoadBalancingConfig {
    fn default() -> Self {
        Self {
            max_imbalance_threshold: 0.2,
            rebalance_interval: Duration::from_secs(5),
            enable_proactive: true,
        }
    }
}

/// Failover configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// RTT threshold for marking path as degraded
    pub rtt_degradation_threshold: Duration,
    /// Loss rate threshold for failover
    pub loss_rate_threshold: f64,
    /// Probe interval for failed paths
    pub failed_path_probe_interval: Duration,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            rtt_degradation_threshold: Duration::from_millis(500),
            loss_rate_threshold: 0.1,
            failed_path_probe_interval: Duration::from_secs(10),
        }
    }
}

/// Advanced path selection manager
#[derive(Debug)]
pub struct AdvancedPathSelector {
    /// Configuration
    config: AdvancedPathSelectionConfig,
    /// Path statistics indexed by PathId
    path_stats: Arc<RwLock<HashMap<PathId, PathStatistics>>>,
    /// Current algorithm state
    algorithm_state: Arc<Mutex<AlgorithmState>>,
    /// Load balancing state
    load_balancer: Arc<Mutex<LoadBalancer>>,
    /// Performance metrics
    metrics: Arc<Mutex<SelectionMetrics>>,
    /// Random number generator state
    rng_state: Arc<Mutex<fastrand::Rng>>,
}

/// Internal algorithm state
#[derive(Debug)]
struct AlgorithmState {
    /// Current round-robin position
    round_robin_position: usize,
    /// Last selection timestamp
    last_selection: Instant,
    /// Selection history for analysis
    selection_history: VecDeque<(PathId, Instant)>,
}

/// Load balancing state and algorithms
#[derive(Debug)]
#[allow(dead_code)]
struct LoadBalancer {
    /// Load distribution targets
    target_distribution: HashMap<PathId, f64>,
    /// Current actual distribution
    actual_distribution: HashMap<PathId, f64>,
    /// Last rebalancing time
    last_rebalance: Instant,
    /// Rebalancing operations count
    rebalance_count: u64,
}

/// Selection performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionMetrics {
    /// Total selections made
    pub total_selections: u64,
    /// Selections per algorithm
    pub selections_by_algorithm: HashMap<String, u64>,
    /// Average selection time
    pub avg_selection_time: Duration,
    /// Path utilization distribution
    pub path_utilization: HashMap<PathId, f64>,
    /// Failover events
    pub failover_events: u64,
    /// Load balancing operations
    pub load_balance_operations: u64,
}

impl Default for SelectionMetrics {
    fn default() -> Self {
        Self {
            total_selections: 0,
            selections_by_algorithm: HashMap::new(),
            avg_selection_time: Duration::ZERO,
            path_utilization: HashMap::new(),
            failover_events: 0,
            load_balance_operations: 0,
        }
    }
}

impl AdvancedPathSelector {
    /// Create a new advanced path selector with configuration
    pub fn new(config: AdvancedPathSelectionConfig) -> Self {
        Self {
            config,
            path_stats: Arc::new(RwLock::new(HashMap::new())),
            algorithm_state: Arc::new(Mutex::new(AlgorithmState {
                round_robin_position: 0,
                last_selection: Instant::now(),
                selection_history: VecDeque::with_capacity(1000),
            })),
            load_balancer: Arc::new(Mutex::new(LoadBalancer {
                target_distribution: HashMap::new(),
                actual_distribution: HashMap::new(),
                last_rebalance: Instant::now(),
                rebalance_count: 0,
            })),
            metrics: Arc::new(Mutex::new(SelectionMetrics::default())),
            rng_state: Arc::new(Mutex::new(fastrand::Rng::new())),
        }
    }

    /// Initialize paths with their base configuration
    pub fn initialize_paths(&self, paths: &[(PathId, PathMetric)]) -> Result<()> {
        let mut stats = self
            .path_stats
            .write()
            .map_err(|_| Error::Protocol("Failed to acquire path stats write lock".to_string()))?;

        for &(path_id, metric) in paths {
            let path_stat = PathStatistics {
                path_id,
                base_weight: metric.weight as f64,
                current_weight: metric.weight as f64,
                rtt_stats: RttStatistics {
                    ewma_rtt: metric.rtt,
                    min_rtt: metric.rtt,
                    max_rtt: metric.rtt,
                    ..Default::default()
                },
                loss_stats: LossStatistics {
                    current_loss_rate: metric.loss as f64,
                    ewma_loss_rate: metric.loss as f64,
                    ..Default::default()
                },
                bandwidth_stats: BandwidthStatistics::default(),
                congestion_metrics: CongestionMetrics::default(),
                is_available: true,
                last_update: SystemTime::now(),
            };

            stats.insert(path_id, path_stat);
        }

        info!(
            path_count = paths.len(),
            algorithm = ?self.config.algorithm,
            "Advanced path selector initialized"
        );

        Ok(())
    }

    /// Select the next path using the configured algorithm
    pub fn select_next_path(&self) -> Result<PathId> {
        let start_time = Instant::now();

        let path_id = self.select_path_internal()?;

        // Update metrics
        let selection_time = start_time.elapsed();
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.total_selections += 1;

            // Update algorithm-specific counters
            let algorithm_name = format!("{:?}", self.config.algorithm);
            *metrics
                .selections_by_algorithm
                .entry(algorithm_name)
                .or_insert(0) += 1;

            // Update average selection time
            let total = metrics.total_selections as f64;
            let current_avg = metrics.avg_selection_time.as_nanos() as f64;
            let new_avg = (current_avg * (total - 1.0) + selection_time.as_nanos() as f64) / total;
            metrics.avg_selection_time = Duration::from_nanos(new_avg as u64);

            // Update path utilization
            *metrics.path_utilization.entry(path_id).or_insert(0.0) += 1.0;
        }

        // Update algorithm state
        if let Ok(mut state) = self.algorithm_state.lock() {
            state.last_selection = Instant::now();
            state.selection_history.push_back((path_id, Instant::now()));

            // Keep history bounded
            if state.selection_history.len() > 1000 {
                state.selection_history.pop_front();
            }
        }

        trace!(
            path_id = path_id.0,
            algorithm = ?self.config.algorithm,
            selection_time_us = selection_time.as_micros(),
            "Path selected"
        );

        Ok(path_id)
    }

    /// Internal path selection logic
    fn select_path_internal(&self) -> Result<PathId> {
        let stats = self
            .path_stats
            .read()
            .map_err(|_| Error::Protocol("Failed to acquire path stats read lock".to_string()))?;

        if stats.is_empty() {
            return Err(Error::Protocol(
                "No paths available for selection".to_string(),
            ));
        }

        // Get available paths
        let available_paths: Vec<&PathStatistics> =
            stats.values().filter(|p| p.is_available).collect();

        if available_paths.is_empty() {
            warn!("No available paths, using first configured path");
            return Ok(stats.keys().next().copied().unwrap_or(PathId(0)));
        }

        match self.config.algorithm {
            PathSelectionAlgorithm::RoundRobin => self.select_round_robin(&available_paths),
            PathSelectionAlgorithm::WeightedRoundRobin => {
                self.select_weighted_round_robin(&available_paths)
            }
            PathSelectionAlgorithm::LatencyBased => self.select_latency_based(&available_paths),
            PathSelectionAlgorithm::LossAware => self.select_loss_aware(&available_paths),
            PathSelectionAlgorithm::BandwidthBased => self.select_bandwidth_based(&available_paths),
            PathSelectionAlgorithm::Hybrid {
                latency_weight,
                loss_weight,
                bandwidth_weight,
            } => self.select_hybrid(
                &available_paths,
                latency_weight,
                loss_weight,
                bandwidth_weight,
            ),
            PathSelectionAlgorithm::Adaptive => self.select_adaptive(&available_paths),
        }
    }

    /// Round-robin selection
    fn select_round_robin(&self, paths: &[&PathStatistics]) -> Result<PathId> {
        let mut state = self
            .algorithm_state
            .lock()
            .map_err(|_| Error::Protocol("Failed to acquire algorithm state lock".to_string()))?;

        let index = state.round_robin_position % paths.len();
        state.round_robin_position = (state.round_robin_position + 1) % paths.len();

        Ok(paths[index].path_id)
    }

    /// Weighted round-robin selection
    fn select_weighted_round_robin(&self, paths: &[&PathStatistics]) -> Result<PathId> {
        // Calculate total weight
        let total_weight: f64 = paths.iter().map(|p| p.current_weight).sum();

        if total_weight <= 0.0 {
            return self.select_round_robin(paths);
        }

        // Generate random value for weighted selection
        let mut rng = self
            .rng_state
            .lock()
            .map_err(|_| Error::Protocol("Failed to acquire RNG lock".to_string()))?;

        let mut random_value = rng.f64() * total_weight;

        for path in paths {
            random_value -= path.current_weight;
            if random_value <= 0.0 {
                return Ok(path.path_id);
            }
        }

        // Fallback to first path
        Ok(paths[0].path_id)
    }

    /// Latency-based selection (prefer lowest RTT)
    fn select_latency_based(&self, paths: &[&PathStatistics]) -> Result<PathId> {
        let best_path = paths
            .iter()
            .min_by_key(|p| p.rtt_stats.ewma_rtt)
            .ok_or_else(|| {
                Error::Protocol("No paths available for latency-based selection".to_string())
            })?;

        Ok(best_path.path_id)
    }

    /// Loss-aware selection (avoid high-loss paths)
    fn select_loss_aware(&self, paths: &[&PathStatistics]) -> Result<PathId> {
        let best_path = paths
            .iter()
            .min_by(|a, b| {
                a.loss_stats
                    .ewma_loss_rate
                    .partial_cmp(&b.loss_stats.ewma_loss_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| {
                Error::Protocol("No paths available for loss-aware selection".to_string())
            })?;

        Ok(best_path.path_id)
    }

    /// Bandwidth-based selection (prefer higher bandwidth paths)
    fn select_bandwidth_based(&self, paths: &[&PathStatistics]) -> Result<PathId> {
        let best_path = paths
            .iter()
            .max_by(|a, b| {
                a.bandwidth_stats
                    .estimated_bandwidth
                    .partial_cmp(&b.bandwidth_stats.estimated_bandwidth)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| {
                Error::Protocol("No paths available for bandwidth-based selection".to_string())
            })?;

        Ok(best_path.path_id)
    }

    /// Hybrid selection combining multiple metrics
    fn select_hybrid(
        &self,
        paths: &[&PathStatistics],
        latency_weight: f64,
        loss_weight: f64,
        bandwidth_weight: f64,
    ) -> Result<PathId> {
        if paths.is_empty() {
            return Err(Error::Protocol(
                "No paths available for hybrid selection".to_string(),
            ));
        }

        // Calculate scores for each path
        let mut scored_paths: Vec<(PathId, f64)> = Vec::new();

        // Normalize metrics across all paths
        let min_rtt = paths
            .iter()
            .map(|p| p.rtt_stats.ewma_rtt.as_millis())
            .min()
            .unwrap_or(1) as f64;
        let max_bandwidth = paths
            .iter()
            .map(|p| p.bandwidth_stats.estimated_bandwidth)
            .fold(0.0, f64::max);
        let max_loss = paths
            .iter()
            .map(|p| p.loss_stats.ewma_loss_rate)
            .fold(0.0, f64::max);

        for path in paths {
            let rtt_ms = path.rtt_stats.ewma_rtt.as_millis() as f64;
            let bandwidth = path.bandwidth_stats.estimated_bandwidth;
            let loss_rate = path.loss_stats.ewma_loss_rate;

            // Calculate normalized scores (lower is better for RTT and loss, higher is better for bandwidth)
            let latency_score = if rtt_ms > 0.0 { min_rtt / rtt_ms } else { 1.0 };
            let loss_score = if max_loss > 0.0 {
                1.0 - (loss_rate / max_loss)
            } else {
                1.0
            };
            let bandwidth_score = if max_bandwidth > 0.0 {
                bandwidth / max_bandwidth
            } else {
                1.0
            };

            // Weighted combination
            let total_score = latency_weight * latency_score
                + loss_weight * loss_score
                + bandwidth_weight * bandwidth_score;

            scored_paths.push((path.path_id, total_score));
        }

        // Select path with highest score
        let best_path = scored_paths
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .ok_or_else(|| Error::Protocol("No paths available after scoring".to_string()))?;

        Ok(best_path.0)
    }

    /// Adaptive selection that changes strategy based on network conditions
    fn select_adaptive(&self, paths: &[&PathStatistics]) -> Result<PathId> {
        // Analyze current network conditions
        let total_loss: f64 = paths.iter().map(|p| p.loss_stats.ewma_loss_rate).sum();
        let avg_loss = total_loss / paths.len() as f64;

        let total_congestion: f64 = paths
            .iter()
            .map(|p| p.congestion_metrics.congestion_level)
            .sum();
        let avg_congestion = total_congestion / paths.len() as f64;

        // Adapt algorithm based on conditions
        if avg_loss > 0.05 {
            // High loss environment - prioritize loss-aware selection
            debug!(
                "Adaptive: Using loss-aware selection due to high loss rate: {:.3}",
                avg_loss
            );
            self.select_loss_aware(paths)
        } else if avg_congestion > 0.7 {
            // High congestion - use latency-based selection
            debug!(
                "Adaptive: Using latency-based selection due to high congestion: {:.3}",
                avg_congestion
            );
            self.select_latency_based(paths)
        } else {
            // Normal conditions - use hybrid approach
            debug!("Adaptive: Using hybrid selection for normal conditions");
            self.select_hybrid(paths, 0.4, 0.3, 0.3)
        }
    }

    /// Observe RTT sample for a path
    pub fn observe_rtt(&self, path_id: PathId, rtt: Duration) -> Result<()> {
        let mut stats = self
            .path_stats
            .write()
            .map_err(|_| Error::Protocol("Failed to acquire path stats write lock".to_string()))?;

        if let Some(path_stat) = stats.get_mut(&path_id) {
            let rtt_ms = rtt.as_nanos() as f64;
            let prev_ewma = path_stat.rtt_stats.ewma_rtt.as_nanos() as f64;

            // Update EWMA
            let alpha = self.config.rtt_config.ewma_alpha;
            let new_ewma = alpha * prev_ewma + (1.0 - alpha) * rtt_ms;
            path_stat.rtt_stats.ewma_rtt = Duration::from_nanos(new_ewma as u64);

            // Update min/max
            path_stat.rtt_stats.min_rtt = path_stat.rtt_stats.min_rtt.min(rtt);
            path_stat.rtt_stats.max_rtt = path_stat.rtt_stats.max_rtt.max(rtt);

            // Update variance
            let diff = rtt_ms - new_ewma;
            path_stat.rtt_stats.rtt_variance =
                alpha * path_stat.rtt_stats.rtt_variance + (1.0 - alpha) * (diff * diff);

            // Add to recent samples
            path_stat.rtt_stats.recent_samples.push_back(rtt);
            if path_stat.rtt_stats.recent_samples.len() > MAX_RTT_SAMPLES {
                path_stat.rtt_stats.recent_samples.pop_front();
            }

            path_stat.rtt_stats.sample_count += 1;
            path_stat.last_update = SystemTime::now();

            // Update current weight based on RTT performance
            self.update_dynamic_weight(path_stat);

            trace!(
                path_id = path_id.0,
                rtt_ms = rtt.as_millis(),
                ewma_rtt_ms = path_stat.rtt_stats.ewma_rtt.as_millis(),
                variance = path_stat.rtt_stats.rtt_variance,
                "RTT sample observed"
            );
        }

        Ok(())
    }

    /// Observe packet loss for a path
    pub fn observe_loss(&self, path_id: PathId) -> Result<()> {
        let mut stats = self
            .path_stats
            .write()
            .map_err(|_| Error::Protocol("Failed to acquire path stats write lock".to_string()))?;

        if let Some(path_stat) = stats.get_mut(&path_id) {
            path_stat.loss_stats.packets_lost += 1;
            path_stat.loss_stats.packets_sent += 1; // Also increment packets sent

            // Update loss rate
            if path_stat.loss_stats.packets_sent > 0 {
                path_stat.loss_stats.current_loss_rate = path_stat.loss_stats.packets_lost as f64
                    / path_stat.loss_stats.packets_sent as f64;
            }

            // Update EWMA loss rate
            let alpha = 0.9; // Smoothing factor for loss rate
            path_stat.loss_stats.ewma_loss_rate =
                alpha * path_stat.loss_stats.ewma_loss_rate + (1.0 - alpha) * 1.0;

            // Update loss penalty
            path_stat.loss_stats.loss_penalty *= LOSS_PENALTY_DECAY;
            path_stat.loss_stats.loss_penalty = path_stat.loss_stats.loss_penalty.max(0.1);

            path_stat.last_update = SystemTime::now();

            // Update current weight
            self.update_dynamic_weight(path_stat);

            debug!(
                path_id = path_id.0,
                current_loss_rate = path_stat.loss_stats.current_loss_rate,
                ewma_loss_rate = path_stat.loss_stats.ewma_loss_rate,
                loss_penalty = path_stat.loss_stats.loss_penalty,
                "Packet loss observed"
            );
        }

        Ok(())
    }

    /// Observe successful packet transmission
    pub fn observe_success(&self, path_id: PathId) -> Result<()> {
        let mut stats = self
            .path_stats
            .write()
            .map_err(|_| Error::Protocol("Failed to acquire path stats write lock".to_string()))?;

        if let Some(path_stat) = stats.get_mut(&path_id) {
            path_stat.loss_stats.packets_sent += 1;

            // Update loss rate
            if path_stat.loss_stats.packets_sent > 0 {
                path_stat.loss_stats.current_loss_rate = path_stat.loss_stats.packets_lost as f64
                    / path_stat.loss_stats.packets_sent as f64;
            }

            // Update EWMA loss rate with success (0.0 loss)
            let alpha = 0.9; // Smoothing factor for loss rate
            path_stat.loss_stats.ewma_loss_rate =
                alpha * path_stat.loss_stats.ewma_loss_rate + (1.0 - alpha) * 0.0;

            path_stat.last_update = SystemTime::now();

            // Update current weight
            self.update_dynamic_weight(path_stat);

            trace!(
                path_id = path_id.0,
                current_loss_rate = path_stat.loss_stats.current_loss_rate,
                ewma_loss_rate = path_stat.loss_stats.ewma_loss_rate,
                packets_sent = path_stat.loss_stats.packets_sent,
                "Successful packet transmission observed"
            );
        }

        Ok(())
    }

    /// Update dynamic weight based on current path metrics
    fn update_dynamic_weight(&self, path_stat: &mut PathStatistics) {
        let base_weight = path_stat.base_weight;
        let rtt_factor = 1.0 / (1.0 + path_stat.rtt_stats.ewma_rtt.as_millis() as f64 / 100.0);
        let loss_factor = path_stat.loss_stats.loss_penalty;
        let congestion_factor = 1.0 - path_stat.congestion_metrics.congestion_level;

        path_stat.current_weight = base_weight * rtt_factor * loss_factor * congestion_factor;
        path_stat.current_weight = path_stat.current_weight.max(0.1); // Minimum weight
    }

    /// Get current path statistics
    pub fn get_path_statistics(&self) -> Result<HashMap<PathId, PathStatistics>> {
        let stats = self
            .path_stats
            .read()
            .map_err(|_| Error::Protocol("Failed to acquire path stats read lock".to_string()))?;

        Ok(stats.clone())
    }

    /// Get selection metrics
    pub fn get_selection_metrics(&self) -> Result<SelectionMetrics> {
        let metrics = self
            .metrics
            .lock()
            .map_err(|_| Error::Protocol("Failed to acquire metrics lock".to_string()))?;

        Ok(metrics.clone())
    }

    /// Trigger load rebalancing
    pub fn rebalance_load(&self) -> Result<()> {
        let mut load_balancer = self
            .load_balancer
            .lock()
            .map_err(|_| Error::Protocol("Failed to acquire load balancer lock".to_string()))?;

        load_balancer.last_rebalance = Instant::now();
        load_balancer.rebalance_count += 1;

        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.load_balance_operations += 1;
        }

        info!(
            rebalance_count = load_balancer.rebalance_count,
            "Load balancing performed"
        );

        Ok(())
    }

    /// Mark path as failed/unavailable
    pub fn mark_path_failed(&self, path_id: PathId) -> Result<()> {
        let mut stats = self
            .path_stats
            .write()
            .map_err(|_| Error::Protocol("Failed to acquire path stats write lock".to_string()))?;

        if let Some(path_stat) = stats.get_mut(&path_id) {
            path_stat.is_available = false;
            path_stat.last_update = SystemTime::now();

            if let Ok(mut metrics) = self.metrics.lock() {
                metrics.failover_events += 1;
            }

            warn!(path_id = path_id.0, "Path marked as failed");
        }

        Ok(())
    }

    /// Mark path as recovered/available
    pub fn mark_path_recovered(&self, path_id: PathId) -> Result<()> {
        let mut stats = self
            .path_stats
            .write()
            .map_err(|_| Error::Protocol("Failed to acquire path stats write lock".to_string()))?;

        if let Some(path_stat) = stats.get_mut(&path_id) {
            path_stat.is_available = true;
            path_stat.last_update = SystemTime::now();

            info!(path_id = path_id.0, "Path marked as recovered");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_paths() -> Vec<(PathId, PathMetric)> {
        vec![
            (
                PathId(1),
                PathMetric {
                    rtt: Duration::from_millis(50),
                    loss: 0.01,
                    weight: 1,
                },
            ),
            (
                PathId(2),
                PathMetric {
                    rtt: Duration::from_millis(100),
                    loss: 0.05,
                    weight: 2,
                },
            ),
            (
                PathId(3),
                PathMetric {
                    rtt: Duration::from_millis(25),
                    loss: 0.02,
                    weight: 3,
                },
            ),
        ]
    }

    #[test]
    fn test_advanced_path_selector_initialization() {
        let config = AdvancedPathSelectionConfig::default();
        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        assert!(selector.initialize_paths(&paths).is_ok());

        let stats = selector.get_path_statistics().unwrap();
        assert_eq!(stats.len(), 3);

        for (path_id, _) in &paths {
            assert!(stats.contains_key(path_id));
            assert!(stats[path_id].is_available);
        }
    }

    #[test]
    fn test_round_robin_selection() {
        let config = AdvancedPathSelectionConfig {
            algorithm: PathSelectionAlgorithm::RoundRobin,
            ..Default::default()
        };

        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        let mut selections = Vec::new();
        for _ in 0..6 {
            selections.push(selector.select_next_path().unwrap());
        }

        // Should cycle through paths
        assert_eq!(selections[0], selections[3]);
        assert_eq!(selections[1], selections[4]);
        assert_eq!(selections[2], selections[5]);
    }

    #[test]
    fn test_latency_based_selection() {
        let config = AdvancedPathSelectionConfig {
            algorithm: PathSelectionAlgorithm::LatencyBased,
            ..Default::default()
        };

        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        // Path 3 has lowest RTT (25ms)
        let selected = selector.select_next_path().unwrap();
        assert_eq!(selected, PathId(3));
    }

    #[test]
    fn test_rtt_observation() {
        let config = AdvancedPathSelectionConfig::default();
        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        // Observe RTT for path 1
        let new_rtt = Duration::from_millis(200);
        selector.observe_rtt(PathId(1), new_rtt).unwrap();

        let stats = selector.get_path_statistics().unwrap();
        let path1_stats = &stats[&PathId(1)];

        // EWMA should be between original and new value
        let ewma_ms = path1_stats.rtt_stats.ewma_rtt.as_millis();
        assert!(ewma_ms > 50);
        assert!(ewma_ms < 200);

        assert_eq!(path1_stats.rtt_stats.max_rtt, new_rtt);
        assert_eq!(path1_stats.rtt_stats.sample_count, 1);
    }

    #[test]
    fn test_loss_observation() {
        let config = AdvancedPathSelectionConfig::default();
        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        // Observe some successes and losses
        for _ in 0..10 {
            selector.observe_success(PathId(1)).unwrap();
        }

        for _ in 0..2 {
            selector.observe_loss(PathId(1)).unwrap();
        }

        let stats = selector.get_path_statistics().unwrap();
        let path1_stats = &stats[&PathId(1)];

        assert_eq!(path1_stats.loss_stats.packets_sent, 12);
        assert_eq!(path1_stats.loss_stats.packets_lost, 2);

        let expected_loss_rate = 2.0 / 12.0;
        assert!((path1_stats.loss_stats.current_loss_rate - expected_loss_rate).abs() < 0.001);
    }

    #[test]
    fn test_hybrid_selection() {
        let config = AdvancedPathSelectionConfig {
            algorithm: PathSelectionAlgorithm::Hybrid {
                latency_weight: 1.0,
                loss_weight: 0.0,
                bandwidth_weight: 0.0,
            },
            ..Default::default()
        };

        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        // With only latency weighting, should select path with lowest RTT
        let selected = selector.select_next_path().unwrap();
        assert_eq!(selected, PathId(3)); // 25ms RTT
    }

    #[test]
    fn test_path_failure_and_recovery() {
        let config = AdvancedPathSelectionConfig::default();
        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        // Mark path as failed
        selector.mark_path_failed(PathId(1)).unwrap();

        let stats = selector.get_path_statistics().unwrap();
        assert!(!stats[&PathId(1)].is_available);

        // Recover path
        selector.mark_path_recovered(PathId(1)).unwrap();

        let stats = selector.get_path_statistics().unwrap();
        assert!(stats[&PathId(1)].is_available);
    }

    #[test]
    fn test_selection_metrics() {
        let config = AdvancedPathSelectionConfig::default();
        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        // Make several selections
        for _ in 0..10 {
            selector.select_next_path().unwrap();
        }

        let metrics = selector.get_selection_metrics().unwrap();
        assert_eq!(metrics.total_selections, 10);
        assert!(metrics.avg_selection_time > Duration::ZERO);
        assert!(!metrics.selections_by_algorithm.is_empty());
    }

    #[test]
    fn test_adaptive_selection() {
        let config = AdvancedPathSelectionConfig {
            algorithm: PathSelectionAlgorithm::Adaptive,
            ..Default::default()
        };

        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        // Should successfully select paths under normal conditions
        let selected = selector.select_next_path().unwrap();
        assert!([PathId(1), PathId(2), PathId(3)].contains(&selected));
    }

    #[test]
    fn test_dynamic_weight_updates() {
        let config = AdvancedPathSelectionConfig::default();
        let selector = AdvancedPathSelector::new(config);
        let paths = create_test_paths();

        selector.initialize_paths(&paths).unwrap();

        let initial_stats = selector.get_path_statistics().unwrap();
        let initial_weight = initial_stats[&PathId(1)].current_weight;

        // Observe high RTT (should decrease weight)
        selector
            .observe_rtt(PathId(1), Duration::from_millis(500))
            .unwrap();

        let updated_stats = selector.get_path_statistics().unwrap();
        let updated_weight = updated_stats[&PathId(1)].current_weight;

        assert!(updated_weight < initial_weight);
    }
}
