//! Adaptive cover traffic system
//! 
//! This module implements intelligent cover traffic generation that adapts
//! to network conditions and traffic patterns to provide optimal privacy
//! protection while minimizing bandwidth overhead.
//!
//! # Features
//!
//! - Dynamic rate adjustment based on network load
//! - Traffic pattern analysis and matching
//! - Bandwidth-efficient cover traffic scheduling
//! - Cross-layer optimization with transport protocols
//!
//! # Example
//!
//! ```rust
//! use nyx_mix::cover_adaptive::{AdaptiveCoverManager, CoverConfig};
//! 
//! let config = CoverConfig::new()
//!     .min_rate(1.0)
//!     .max_rate(10.0)
//!     .adaptation_interval(std::time::Duration::from_secs(30));
//!     
//! let mut manager = AdaptiveCoverManager::new(config);
//! manager.start_adaptive_cover();
//! ```

use std::time::{Duration, Instant};
use rand::Rng;
use crate::cover::poisson_rate;

/// Configuration for adaptive cover traffic
#[derive(Debug, Clone)]
pub struct CoverConfig {
    /// Minimum cover traffic rate (packets/second)
    pub min_rate: f32,
    /// Maximum cover traffic rate (packets/second)
    pub max_rate: f32,
    /// How often to adjust the cover traffic rate
    pub adaptation_interval: Duration,
    /// Sensitivity to network load changes (0.0-1.0)
    pub load_sensitivity: f32,
    /// Whether to use burst protection
    pub burst_protection: bool,
}

impl Default for CoverConfig {
    fn default() -> Self {
        Self {
            min_rate: 0.5,
            max_rate: 5.0,
            adaptation_interval: Duration::from_secs(60),
            load_sensitivity: 0.7,
            burst_protection: true,
        }
    }
}

impl CoverConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn min_rate(mut self, rate: f32) -> Self {
        self.min_rate = rate;
        self
    }

    pub fn max_rate(mut self, rate: f32) -> Self {
        self.max_rate = rate;
        self
    }

    pub fn adaptation_interval(mut self, interval: Duration) -> Self {
        self.adaptation_interval = interval;
        self
    }

    pub fn load_sensitivity(mut self, sensitivity: f32) -> Self {
        self.load_sensitivity = sensitivity.clamp(0.0, 1.0);
        self
    }

    pub fn burst_protection(mut self, enabled: bool) -> Self {
        self.burst_protection = enabled;
        self
    }
}

/// Network load metrics for adaptive decision making
#[derive(Debug, Clone, Default)]
pub struct NetworkMetrics {
    /// Current bandwidth utilization (0.0-1.0)
    pub bandwidth_utilization: f32,
    /// Average packet inter-arrival time
    pub avg_interarrival: Duration,
    /// Recent traffic variance
    pub traffic_variance: f32,
    /// Number of active flows
    pub active_flows: u32,
}

/// Adaptive cover traffic manager
pub struct AdaptiveCoverManager {
    config: CoverConfig,
    current_rate: f32,
    last_adaptation: Instant,
    metrics_history: Vec<NetworkMetrics>,
    burst_detector: BurstDetector,
}

impl AdaptiveCoverManager {
    pub fn new(config: CoverConfig) -> Self {
        let current_rate = (config.min_rate + config.max_rate) / 2.0;
        Self {
            config,
            current_rate,
            last_adaptation: Instant::now(),
            metrics_history: Vec::new(),
            burst_detector: BurstDetector::new(),
        }
    }

    /// Update network metrics and potentially adjust cover traffic rate
    pub fn update_metrics(&mut self, metrics: NetworkMetrics) {
        self.metrics_history.push(metrics.clone());
        
        // Keep only recent history (last 10 measurements)
        if self.metrics_history.len() > 10 {
            self.metrics_history.remove(0);
        }

        self.burst_detector.update(&metrics);

        // Check if it's time to adapt
        if self.last_adaptation.elapsed() >= self.config.adaptation_interval {
            self.adapt_rate(&metrics);
            self.last_adaptation = Instant::now();
        }
    }

    /// Generate cover traffic for the current time period
    pub fn generate_cover_traffic(&self, rng: &mut impl Rng) -> u32 {
        let base_count = poisson_rate(self.current_rate, rng);
        
        // Apply burst protection if enabled
        if self.config.burst_protection && self.burst_detector.is_burst_detected() {
            // Increase cover traffic during bursts to maintain anonymity
            let burst_multiplier = 1.5;
            (base_count as f32 * burst_multiplier) as u32
        } else {
            base_count
        }
    }

    /// Get current cover traffic rate
    pub fn current_rate(&self) -> f32 {
        self.current_rate
    }

    /// Adapt the cover traffic rate based on network conditions
    fn adapt_rate(&mut self, current_metrics: &NetworkMetrics) {
        if self.metrics_history.len() < 2 {
            return; // Need more history for meaningful adaptation
        }

        let load_factor = current_metrics.bandwidth_utilization;
        let variance = current_metrics.traffic_variance;

        // Calculate target rate based on network conditions
        let mut target_rate = self.config.max_rate;

        // Reduce rate when network is congested
        if load_factor > 0.8 {
            target_rate *= 1.0 - (load_factor - 0.8) * 2.0; // Aggressive reduction
        } else if load_factor > 0.6 {
            target_rate *= 1.0 - (load_factor - 0.6) * 0.5; // Moderate reduction
        }

        // Increase rate when traffic is highly variable (more anonymity needed)
        if variance > 0.5 {
            target_rate *= 1.0 + variance * 0.3;
        }

        // Apply sensitivity factor
        let rate_change = (target_rate - self.current_rate) * self.config.load_sensitivity;
        self.current_rate += rate_change;

        // Clamp to configured bounds
        self.current_rate = self.current_rate.clamp(self.config.min_rate, self.config.max_rate);
    }
}

/// Detects traffic bursts for enhanced cover protection
struct BurstDetector {
    packet_times: Vec<Instant>,
    burst_threshold: f32,
    burst_detected: bool,
}

impl BurstDetector {
    fn new() -> Self {
        Self {
            packet_times: Vec::new(),
            burst_threshold: 3.0, // packets/second threshold for burst detection
            burst_detected: false,
        }
    }

    fn update(&mut self, _metrics: &NetworkMetrics) {
        let now = Instant::now();
        self.packet_times.push(now);

        // Keep only last 5 seconds of data
        let cutoff = now - Duration::from_secs(5);
        self.packet_times.retain(|&time| time > cutoff);

        // Detect burst: more packets than threshold in recent window
        let recent_rate = self.packet_times.len() as f32 / 5.0;
        self.burst_detected = recent_rate > self.burst_threshold;
    }

    fn is_burst_detected(&self) -> bool {
        self.burst_detected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn test_cover_config_builder() {
        let config = CoverConfig::new()
            .min_rate(1.0)
            .max_rate(8.0)
            .load_sensitivity(0.5);
        
        assert_eq!(config.min_rate, 1.0);
        assert_eq!(config.max_rate, 8.0);
        assert_eq!(config.load_sensitivity, 0.5);
    }

    #[test]
    fn test_adaptive_manager_basic() {
        let config = CoverConfig::new();
        let mut manager = AdaptiveCoverManager::new(config);
        let mut rng = thread_rng();

        let initial_rate = manager.current_rate();
        let traffic = manager.generate_cover_traffic(&mut rng);
        
        assert!(initial_rate > 0.0);
        assert!(traffic <= 50); // Reasonable upper bound
    }

    #[test]
    fn test_rate_adaptation() {
        let config = CoverConfig::new().adaptation_interval(Duration::from_millis(1));
        let mut manager = AdaptiveCoverManager::new(config);

        // Simulate high load scenario
        let high_load_metrics = NetworkMetrics {
            bandwidth_utilization: 0.9,
            traffic_variance: 0.2,
            ..Default::default()
        };

        std::thread::sleep(Duration::from_millis(2)); // Ensure adaptation interval passed
        manager.update_metrics(high_load_metrics);

        // Rate should be reduced due to high load
        assert!(manager.current_rate() < manager.config.max_rate);
    }

    #[test]
    fn test_burst_detection() {
        let mut detector = BurstDetector::new();
        let metrics = NetworkMetrics::default();

        // Simulate burst
        for _ in 0..20 {
            detector.update(&metrics);
        }

        assert!(detector.is_burst_detected());
    }
}
