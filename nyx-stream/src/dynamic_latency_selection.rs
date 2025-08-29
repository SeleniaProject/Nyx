//! Dynamic Latency-based Path Selection for Nyx Protocol v1.0
//!
//! This module implements sophisticated latency-aware path selection that dynamically
//! adapts to network conditions, providing optimal routing based on real-time latency
//! measurements and statistical analysis.

use crate::multipath::scheduler::PathId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tracing::{debug, info, trace};

/// Minimum number of latency samples required for reliable statistics
const MIN_LATENCY_SAMPLES: usize = 10;

/// Maximum number of latency samples to keep for each path
const MAX_LATENCY_SAMPLES: usize = 100;

/// Latency measurement window in seconds
const LATENCY_WINDOW_SECONDS: u64 = 60;

/// Adaptive threshold adjustment factor
const THRESHOLD_ADJUSTMENT_FACTOR: f64 = 0.1;

/// Configuration for dynamic latency-based path selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicLatencyConfig {
    /// Minimum number of samples needed for reliable statistics
    pub min_samples: usize,
    /// Maximum samples to maintain per path
    pub max_samples: usize,
    /// Measurement window duration in seconds
    pub window_seconds: u64,
    /// Weight factor for latency in path scoring
    pub latency_weight: f64,
    /// Enable adaptive threshold adjustment
    pub adaptive_thresholds: bool,
    /// Jitter sensitivity factor
    pub jitter_sensitivity: f64,
    /// Latency change detection threshold
    pub change_detection_threshold: f64,
}

impl Default for DynamicLatencyConfig {
    fn default() -> Self {
        Self {
            min_samples: MIN_LATENCY_SAMPLES,
            max_samples: MAX_LATENCY_SAMPLES,
            window_seconds: LATENCY_WINDOW_SECONDS,
            latency_weight: 1.0,
            adaptive_thresholds: true,
            jitter_sensitivity: 0.3,
            change_detection_threshold: 0.2,
        }
    }
}

/// Latency measurement sample
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct LatencySample {
    /// Measured round-trip time
    rtt: Duration,
    /// Timestamp when measurement was taken
    timestamp: Instant,
    /// Sequence number for ordering
    sequence: u64,
}

/// Latency statistics for a path
#[derive(Debug, Clone)]
pub struct LatencyStats {
    /// Current average latency
    pub average: Duration,
    /// Minimum observed latency
    pub minimum: Duration,
    /// Maximum observed latency
    pub maximum: Duration,
    /// Standard deviation of latency
    pub std_deviation: Duration,
    /// Jitter (variance in latency)
    pub jitter: Duration,
    /// 95th percentile latency
    pub p95: Duration,
    /// 99th percentile latency
    pub p99: Duration,
    /// Number of samples used
    pub sample_count: usize,
    /// Trend direction (positive = increasing, negative = decreasing)
    pub trend: f64,
    /// Last update timestamp
    pub last_update: Instant,
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self {
            average: Duration::ZERO,
            minimum: Duration::MAX,
            maximum: Duration::ZERO,
            std_deviation: Duration::ZERO,
            jitter: Duration::ZERO,
            p95: Duration::ZERO,
            p99: Duration::ZERO,
            sample_count: 0,
            trend: 0.0,
            last_update: Instant::now(),
        }
    }
}

/// Path latency information
#[derive(Debug)]
struct PathLatencyInfo {
    /// Recent latency samples
    samples: VecDeque<LatencySample>,
    /// Computed statistics
    stats: LatencyStats,
    /// Current latency classification
    classification: LatencyClassification,
    /// Adaptive threshold values
    adaptive_thresholds: AdaptiveThresholds,
    /// Sequence counter for samples
    sequence_counter: u64,
}

impl PathLatencyInfo {
    fn new() -> Self {
        Self {
            samples: VecDeque::new(),
            stats: LatencyStats::default(),
            classification: LatencyClassification::Unknown,
            adaptive_thresholds: AdaptiveThresholds::default(),
            sequence_counter: 0,
        }
    }
}

/// Latency classification for paths
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyClassification {
    /// Very low latency path (< 25th percentile)
    VeryLow,
    /// Low latency path (25th-50th percentile)
    Low,
    /// Medium latency path (50th-75th percentile)
    Medium,
    /// High latency path (75th-90th percentile)
    High,
    /// Very high latency path (> 90th percentile)
    VeryHigh,
    /// Degraded path (significant latency increase)
    Degraded,
    /// Unknown classification (insufficient data)
    Unknown,
}

/// Adaptive threshold values for latency classification
#[derive(Debug, Clone)]
struct AdaptiveThresholds {
    /// Low latency threshold
    low_threshold: Duration,
    /// Medium latency threshold
    medium_threshold: Duration,
    /// High latency threshold
    high_threshold: Duration,
    /// Degradation threshold
    degradation_threshold: Duration,
    /// Last adjustment timestamp
    last_adjustment: Instant,
}

impl Default for AdaptiveThresholds {
    fn default() -> Self {
        Self {
            low_threshold: Duration::from_millis(50),
            medium_threshold: Duration::from_millis(100),
            high_threshold: Duration::from_millis(200),
            degradation_threshold: Duration::from_millis(500),
            last_adjustment: Instant::now(),
        }
    }
}

/// Dynamic latency-based path selector
pub struct DynamicLatencySelector {
    /// Configuration
    config: DynamicLatencyConfig,
    /// Path latency information
    paths: HashMap<PathId, PathLatencyInfo>,
    /// Global latency statistics for comparison
    global_stats: Option<LatencyStats>,
    /// Available paths sorted by latency
    sorted_paths: Vec<PathId>,
    /// Last path selection timestamp
    last_selection: Instant,
    /// Selection counter for round-robin fallback
    selection_counter: u64,
}

impl DynamicLatencySelector {
    /// Create a new dynamic latency selector
    pub fn new(config: DynamicLatencyConfig) -> Self {
        info!("Initializing dynamic latency-based path selector");
        Self {
            config,
            paths: HashMap::new(),
            global_stats: None,
            sorted_paths: Vec::new(),
            last_selection: Instant::now(),
            selection_counter: 0,
        }
    }

    /// Add a new path for latency tracking
    pub fn add_path(&mut self, path_id: PathId) {
        debug!("Adding path {:?} to latency tracker", path_id);
        self.paths.insert(path_id, PathLatencyInfo::new());
        self.update_sorted_paths();
    }

    /// Remove a path from latency tracking
    pub fn remove_path(&mut self, path_id: PathId) {
        debug!("Removing path {:?} from latency tracker", path_id);
        self.paths.remove(&path_id);
        self.sorted_paths.retain(|&id| id != path_id);
    }

    /// Record a latency measurement for a path
    pub fn record_latency(&mut self, path_id: PathId, latency: Duration) {
        trace!("Recording latency {:?} for path {:?}", latency, path_id);

        if let Some(path_info) = self.paths.get_mut(&path_id) {
            let sample = LatencySample {
                rtt: latency,
                timestamp: Instant::now(),
                sequence: path_info.sequence_counter,
            };

            path_info.sequence_counter += 1;
            path_info.samples.push_back(sample);

            // Maintain sample window size
            while path_info.samples.len() > self.config.max_samples {
                path_info.samples.pop_front();
            }

            // Remove old samples outside the time window
            let window_start = Instant::now() - Duration::from_secs(self.config.window_seconds);
            while let Some(front) = path_info.samples.front() {
                if front.timestamp < window_start {
                    path_info.samples.pop_front();
                } else {
                    break;
                }
            }

            // Update statistics if we have enough samples
            if path_info.samples.len() >= self.config.min_samples {
                self.update_path_statistics(path_id);
                self.update_path_classification(path_id);

                if self.config.adaptive_thresholds {
                    self.update_adaptive_thresholds(path_id);
                }
            }
        }

        // Update global statistics
        self.update_global_statistics();
        self.update_sorted_paths();
    }

    /// Select the best path based on current latency conditions
    pub fn select_path(&mut self) -> Option<PathId> {
        self.selection_counter += 1;
        self.last_selection = Instant::now();

        // Create available paths vector by collecting references
        let available_paths: Vec<_> = self
            .paths
            .iter()
            .filter(|(_, info)| info.samples.len() >= self.config.min_samples)
            .map(|(&path_id, _)| path_id)
            .collect();

        if available_paths.is_empty() {
            // Fallback to round-robin if no latency data available
            return self.fallback_selection();
        }

        // Select based on latency classification and current conditions
        self.select_optimal_latency_path_ids(&available_paths)
    }

    /// Get current latency statistics for a path
    pub fn get_path_stats(&self, path_id: PathId) -> Option<&LatencyStats> {
        self.paths.get(&path_id).map(|info| &info.stats)
    }

    /// Get latency classification for a path
    pub fn get_path_classification(&self, path_id: PathId) -> Option<LatencyClassification> {
        self.paths.get(&path_id).map(|info| info.classification)
    }

    /// Get all available paths sorted by latency
    pub fn get_sorted_paths(&self) -> &[PathId] {
        &self.sorted_paths
    }

    /// Get global latency statistics
    pub fn get_global_stats(&self) -> Option<&LatencyStats> {
        self.global_stats.as_ref()
    }

    // Private implementation methods

    fn update_path_statistics(&mut self, path_id: PathId) {
        if let Some(path_info) = self.paths.get_mut(&path_id) {
            let samples: Vec<_> = path_info.samples.iter().map(|s| s.rtt).collect();

            if samples.is_empty() {
                return;
            }

            // Calculate basic statistics
            let sum: Duration = samples.iter().sum();
            let count = samples.len();
            let average = sum / count as u32;

            let minimum = samples.iter().min().copied().unwrap_or(Duration::ZERO);
            let maximum = samples.iter().max().copied().unwrap_or(Duration::ZERO);

            // Calculate standard deviation and jitter
            let variance_sum: u128 = samples
                .iter()
                .map(|&sample| {
                    let diff = sample.abs_diff(average);
                    diff.as_nanos().pow(2)
                })
                .sum();

            let variance = variance_sum / count as u128;
            let std_deviation = Duration::from_nanos((variance as f64).sqrt() as u64);

            // Calculate jitter (inter-packet variance)
            let mut jitter_sum = 0u128;
            let mut jitter_count = 0;
            let samples_vec: Vec<_> = path_info.samples.iter().collect();
            for window in samples_vec.windows(2) {
                let diff = window[1].rtt.abs_diff(window[0].rtt);
                jitter_sum += diff.as_nanos();
                jitter_count += 1;
            }

            let jitter = if jitter_count > 0 {
                Duration::from_nanos((jitter_sum / jitter_count as u128) as u64)
            } else {
                Duration::ZERO
            };

            // Calculate percentiles
            let mut sorted_samples = samples.clone();
            sorted_samples.sort();

            let p95_index = (count as f64 * 0.95) as usize;
            let p99_index = (count as f64 * 0.99) as usize;

            let p95 = sorted_samples
                .get(p95_index.min(count - 1))
                .copied()
                .unwrap_or(Duration::ZERO);
            let p99 = sorted_samples
                .get(p99_index.min(count - 1))
                .copied()
                .unwrap_or(Duration::ZERO);

            // Calculate trend (simple linear regression on recent samples)
            let trend = Self::calculate_trend_from_samples(&samples);

            // Update statistics
            path_info.stats = LatencyStats {
                average,
                minimum,
                maximum,
                std_deviation,
                jitter,
                p95,
                p99,
                sample_count: count,
                trend,
                last_update: Instant::now(),
            };

            debug!(
                "Updated latency stats for path {:?}: avg={:?}, min={:?}, max={:?}, jitter={:?}, trend={:.3}",
                path_id, average, minimum, maximum, jitter, trend
            );
        }
    }

    fn calculate_trend_from_samples(samples: &[Duration]) -> f64 {
        if samples.len() < 5 {
            return 0.0; // Not enough data for trend analysis
        }

        // Use the most recent samples for trend calculation
        let recent_samples: Vec<_> = samples.iter().rev().take(20).collect();
        let n = recent_samples.len() as f64;

        if n < 2.0 {
            return 0.0;
        }

        // Simple linear regression
        let sum_x: f64 = (0..recent_samples.len()).map(|i| i as f64).sum();
        let sum_y: f64 = recent_samples.iter().map(|s| s.as_nanos() as f64).sum();
        let sum_xy: f64 = recent_samples
            .iter()
            .enumerate()
            .map(|(i, s)| i as f64 * s.as_nanos() as f64)
            .sum();
        let sum_x2: f64 = (0..recent_samples.len()).map(|i| (i as f64).powi(2)).sum();

        let denominator = n * sum_x2 - sum_x.powi(2);
        if denominator.abs() < f64::EPSILON {
            return 0.0;
        }

        // Slope indicates trend direction and magnitude
        (n * sum_xy - sum_x * sum_y) / denominator
    }

    fn update_path_classification(&mut self, path_id: PathId) {
        if let Some(path_info) = self.paths.get_mut(&path_id) {
            let stats = &path_info.stats;
            let thresholds = &path_info.adaptive_thresholds;

            let classification = if stats.sample_count < self.config.min_samples {
                LatencyClassification::Unknown
            } else if stats.average > thresholds.degradation_threshold {
                LatencyClassification::Degraded
            } else if stats.average <= thresholds.low_threshold {
                if stats.jitter < Duration::from_millis(5) {
                    LatencyClassification::VeryLow
                } else {
                    LatencyClassification::Low
                }
            } else if stats.average <= thresholds.medium_threshold {
                LatencyClassification::Medium
            } else if stats.average <= thresholds.high_threshold {
                LatencyClassification::High
            } else {
                LatencyClassification::VeryHigh
            };

            if classification != path_info.classification {
                info!(
                    "Path {:?} classification changed: {:?} -> {:?} (avg latency: {:?})",
                    path_id, path_info.classification, classification, stats.average
                );
                path_info.classification = classification;
            }
        }
    }

    fn update_adaptive_thresholds(&mut self, path_id: PathId) {
        if let Some(path_info) = self.paths.get_mut(&path_id) {
            let stats = &path_info.stats;
            let thresholds = &mut path_info.adaptive_thresholds;

            // Adjust thresholds based on observed latency distribution
            let adjustment_factor =
                self.config.change_detection_threshold * THRESHOLD_ADJUSTMENT_FACTOR;

            // Adjust low threshold based on minimum observed latency
            let target_low = stats.minimum + (stats.average - stats.minimum) / 4;
            let low_diff =
                target_low.as_nanos() as i128 - thresholds.low_threshold.as_nanos() as i128;
            let low_adjustment =
                Duration::from_nanos(((low_diff as f64) * adjustment_factor).abs() as u64);

            if target_low > thresholds.low_threshold {
                thresholds.low_threshold += low_adjustment;
            } else {
                thresholds.low_threshold = thresholds.low_threshold.saturating_sub(low_adjustment);
            }

            // Adjust other thresholds proportionally
            thresholds.medium_threshold = thresholds.low_threshold * 2;
            thresholds.high_threshold = thresholds.low_threshold * 4;
            thresholds.degradation_threshold = thresholds.low_threshold * 8;

            thresholds.last_adjustment = Instant::now();

            trace!(
                "Adjusted adaptive thresholds for path {:?}: low={:?}, medium={:?}, high={:?}",
                path_id,
                thresholds.low_threshold,
                thresholds.medium_threshold,
                thresholds.high_threshold
            );
        }
    }

    fn update_global_statistics(&mut self) {
        let all_samples: Vec<Duration> = self
            .paths
            .values()
            .flat_map(|info| info.samples.iter().map(|s| s.rtt))
            .collect();

        if all_samples.is_empty() {
            self.global_stats = None;
            return;
        }

        let sum: Duration = all_samples.iter().sum();
        let count = all_samples.len();
        let average = sum / count as u32;

        let minimum = all_samples.iter().min().copied().unwrap_or(Duration::ZERO);
        let maximum = all_samples.iter().max().copied().unwrap_or(Duration::ZERO);

        // Calculate global statistics
        let mut sorted_samples = all_samples.clone();
        sorted_samples.sort();

        let p95_index = (count as f64 * 0.95) as usize;
        let p99_index = (count as f64 * 0.99) as usize;

        let p95 = sorted_samples
            .get(p95_index.min(count - 1))
            .copied()
            .unwrap_or(Duration::ZERO);
        let p99 = sorted_samples
            .get(p99_index.min(count - 1))
            .copied()
            .unwrap_or(Duration::ZERO);

        self.global_stats = Some(LatencyStats {
            average,
            minimum,
            maximum,
            std_deviation: Duration::ZERO, // Simplified for global stats
            jitter: Duration::ZERO,
            p95,
            p99,
            sample_count: count,
            trend: 0.0,
            last_update: Instant::now(),
        });
    }

    fn update_sorted_paths(&mut self) {
        let mut path_scores: Vec<(PathId, f64)> = self
            .paths
            .iter()
            .filter(|(_, info)| info.samples.len() >= self.config.min_samples)
            .map(|(&path_id, info)| {
                let score = self.calculate_path_score(info);
                (path_id, score)
            })
            .collect();

        // Sort by score (lower is better for latency)
        path_scores.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        self.sorted_paths = path_scores
            .into_iter()
            .map(|(path_id, _)| path_id)
            .collect();

        trace!("Updated sorted paths: {:?}", self.sorted_paths);
    }

    fn calculate_path_score(&self, path_info: &PathLatencyInfo) -> f64 {
        let stats = &path_info.stats;

        // Base score from average latency
        let mut score = stats.average.as_nanos() as f64;

        // Penalize high jitter
        score += (stats.jitter.as_nanos() as f64) * self.config.jitter_sensitivity;

        // Penalize negative trends (increasing latency)
        if stats.trend > 0.0 {
            score += stats.trend * 1000.0; // Amplify trend impact
        }

        // Bonus for very stable paths (low jitter)
        if stats.jitter < Duration::from_millis(5) {
            score *= 0.9; // 10% bonus
        }

        // Penalty for degraded paths
        if matches!(path_info.classification, LatencyClassification::Degraded) {
            score *= 2.0; // Heavy penalty
        }

        score
    }

    fn select_optimal_latency_path_ids(&self, available_path_ids: &[PathId]) -> Option<PathId> {
        // Prioritize paths by classification
        let classifications_priority = [
            LatencyClassification::VeryLow,
            LatencyClassification::Low,
            LatencyClassification::Medium,
            LatencyClassification::High,
            LatencyClassification::VeryHigh,
        ];

        for &target_class in &classifications_priority {
            let candidate_paths: Vec<_> = available_path_ids
                .iter()
                .filter_map(|&path_id| self.paths.get(&path_id).map(|info| (path_id, info)))
                .filter(|(_, info)| info.classification == target_class)
                .collect();

            if !candidate_paths.is_empty() {
                // Select best path within this classification
                let best_path = candidate_paths
                    .iter()
                    .min_by(|a, b| {
                        let score_a = self.calculate_path_score(a.1);
                        let score_b = self.calculate_path_score(b.1);
                        score_a
                            .partial_cmp(&score_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(path_id, _)| *path_id);

                if let Some(selected_path) = best_path {
                    debug!(
                        "Selected path {:?} with classification {:?}",
                        selected_path, target_class
                    );
                    return Some(selected_path);
                }
            }
        }

        // Fallback to best available path by score
        available_path_ids
            .iter()
            .filter_map(|&path_id| self.paths.get(&path_id).map(|info| (path_id, info)))
            .min_by(|a, b| {
                let score_a = self.calculate_path_score(a.1);
                let score_b = self.calculate_path_score(b.1);
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(path_id, _)| path_id)
    }

    fn fallback_selection(&mut self) -> Option<PathId> {
        if self.paths.is_empty() {
            return None;
        }

        // Simple round-robin fallback when no latency data is available
        let path_ids: Vec<_> = self.paths.keys().copied().collect();
        let index = (self.selection_counter as usize) % path_ids.len();

        debug!(
            "Using fallback round-robin selection: path {:?}",
            path_ids[index]
        );
        Some(path_ids[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_latency_selector_creation() {
        let config = DynamicLatencyConfig::default();
        let selector = DynamicLatencySelector::new(config);

        assert!(selector.paths.is_empty());
        assert!(selector.global_stats.is_none());
        assert!(selector.sorted_paths.is_empty());
    }

    #[test]
    fn test_path_management() {
        let config = DynamicLatencyConfig::default();
        let mut selector = DynamicLatencySelector::new(config);

        let path1 = PathId(1);
        let path2 = PathId(2);

        selector.add_path(path1);
        selector.add_path(path2);

        assert!(selector.paths.contains_key(&path1));
        assert!(selector.paths.contains_key(&path2));

        selector.remove_path(path1);
        assert!(!selector.paths.contains_key(&path1));
        assert!(selector.paths.contains_key(&path2));
    }

    #[test]
    fn test_latency_recording() {
        let config = DynamicLatencyConfig::default();
        let mut selector = DynamicLatencySelector::new(config);

        let path_id = PathId(1);
        selector.add_path(path_id);

        // Record multiple latency measurements
        for i in 0..15 {
            let latency = Duration::from_millis(50 + i * 10);
            selector.record_latency(path_id, latency);
        }

        let stats = selector.get_path_stats(path_id).unwrap();
        assert!(stats.sample_count >= 10);
        assert!(stats.average > Duration::ZERO);
        assert!(stats.minimum <= stats.average);
        assert!(stats.maximum >= stats.average);
    }

    #[test]
    fn test_path_classification() {
        let config = DynamicLatencyConfig::default();
        let mut selector = DynamicLatencySelector::new(config);

        let path_id = PathId(1);
        selector.add_path(path_id);

        // Record low latency measurements
        for _ in 0..15 {
            selector.record_latency(path_id, Duration::from_millis(20));
        }

        let classification = selector.get_path_classification(path_id).unwrap();
        assert!(matches!(
            classification,
            LatencyClassification::VeryLow | LatencyClassification::Low
        ));
    }

    #[test]
    fn test_path_selection() {
        let config = DynamicLatencyConfig::default();
        let mut selector = DynamicLatencySelector::new(config);

        let path1 = PathId(1);
        let path2 = PathId(2);

        selector.add_path(path1);
        selector.add_path(path2);

        // Record different latencies for each path
        for _ in 0..15 {
            selector.record_latency(path1, Duration::from_millis(20)); // Low latency
            selector.record_latency(path2, Duration::from_millis(100)); // Higher latency
        }

        // Path 1 should be preferred due to lower latency
        let selected = selector.select_path();
        assert_eq!(selected, Some(path1));
    }

    #[test]
    fn test_adaptive_thresholds() {
        let config = DynamicLatencyConfig {
            adaptive_thresholds: true,
            ..Default::default()
        };
        let mut selector = DynamicLatencySelector::new(config);

        let path_id = PathId(1);
        selector.add_path(path_id);

        // Record varying latencies to trigger threshold adaptation
        for i in 0..20 {
            let latency = Duration::from_millis(30 + (i % 5) * 10);
            selector.record_latency(path_id, latency);
        }

        let stats = selector.get_path_stats(path_id).unwrap();
        assert!(stats.sample_count >= 15);

        // Verify adaptive thresholds were updated
        let path_info = selector.paths.get(&path_id).unwrap();
        assert!(
            path_info.adaptive_thresholds.last_adjustment > Instant::now() - Duration::from_secs(1)
        );
    }
}
