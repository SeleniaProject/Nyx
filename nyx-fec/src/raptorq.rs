#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Bi-directional encoding redundancy (tx/rx) with adaptive tuning capabilities
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Redundancy {
    pub tx: f32,
    pub rx: f32,
}

impl Redundancy {
    /// Create new redundancy with safe default values
    pub fn new(tx: f32, rx: f32) -> Self {
        Self { tx, rx }.clamp()
    }

    /// Clamp redundancy values to valid range [0.0, 0.9]
    pub fn clamp(self) -> Self {
        let c = |v: f32| v.clamp(0.0, 0.9);
        Redundancy {
            tx: c(self.tx),
            rx: c(self.rx),
        }
    }

    /// Get the maximum redundancy value
    pub fn max_redundancy(&self) -> f32 {
        self.tx.max(self.rx)
    }

    /// Calculate effective overhead as percentage
    pub fn overhead_percent(&self) -> f32 {
        (self.tx + self.rx) * 100.0 / 2.0
    }
}

impl Default for Redundancy {
    fn default() -> Self {
        Self { tx: 0.1, rx: 0.1 } // 10% default redundancy
    }
}

/// Network quality metrics for adaptive redundancy calculation
#[derive(Debug, Clone, Copy)]
pub struct NetworkMetrics {
    /// Round-trip time in milliseconds
    pub rtt_ms: u32,
    /// Jitter (RTT variance) in milliseconds
    pub jitter_ms: u32,
    /// Observed packet loss rate [0.0, 1.0]
    pub loss_rate: f32,
    /// Available bandwidth estimate in kbps
    pub bandwidth_kbps: u32,
    /// Measurement timestamp
    pub timestamp: Instant,
}

impl NetworkMetrics {
    /// Create network metrics with current timestamp
    pub fn new(rtt_ms: u32, jitter_ms: u32, loss_rate: f32, bandwidth_kbps: u32) -> Self {
        Self {
            rtt_ms,
            jitter_ms,
            loss_rate: loss_rate.clamp(0.0, 1.0),
            bandwidth_kbps,
            timestamp: Instant::now(),
        }
    }

    /// Calculate network quality score [0.0, 1.0] where 1.0 is perfect
    pub fn quality_score(&self) -> f32 {
        // RTT component: penalize high latency (>200ms very bad)
        let rtt_score = (1.0 - (self.rtt_ms as f32 / 200.0)).clamp(0.0, 1.0);

        // Jitter component: penalize variance (>50ms very bad)
        let jitter_score = (1.0 - (self.jitter_ms as f32 / 50.0)).clamp(0.0, 1.0);

        // Loss component: direct inverse
        let loss_score = 1.0 - self.loss_rate;

        // Weighted average: loss is most important
        (loss_score * 0.5 + rtt_score * 0.3 + jitter_score * 0.2).clamp(0.0, 1.0)
    }

    /// Check if network conditions are stable
    pub fn is_stable(&self) -> bool {
        self.rtt_ms < 100 && self.jitter_ms < 20 && self.loss_rate < 0.01
    }
}

/// Adaptive redundancy tuner with historical tracking and PID-style control
#[derive(Debug, Clone)]
pub struct AdaptiveRedundancyTuner {
    /// Historical network metrics (limited size)
    history: VecDeque<NetworkMetrics>,
    /// Maximum history size
    max_history: usize,
    /// Current redundancy settings
    current_redundancy: Redundancy,
    /// PID control coefficients
    pid_coefficients: PidCoefficients,
    /// Last adjustment timestamp
    last_adjustment: Option<Instant>,
    /// Minimum adjustment interval
    min_adjustment_interval: Duration,
    /// Loss rate moving average window
    loss_window: VecDeque<f32>,
    /// Maximum loss window size
    max_loss_window: usize,
}

/// PID controller coefficients for redundancy adjustment
#[derive(Debug, Clone, Copy)]
pub struct PidCoefficients {
    /// Proportional gain (response to current error)
    pub kp: f32,
    /// Integral gain (response to accumulated error)
    pub ki: f32,
    /// Derivative gain (response to error rate of change)
    pub kd: f32,
}

impl Default for PidCoefficients {
    fn default() -> Self {
        Self {
            kp: 0.5, // Moderate proportional response
            ki: 0.1, // Low integral to avoid oscillation
            kd: 0.2, // Moderate derivative for stability
        }
    }
}

impl AdaptiveRedundancyTuner {
    /// Create new adaptive tuner with default parameters
    pub fn new() -> Self {
        Self::with_config(50, Duration::from_secs(1), PidCoefficients::default())
    }

    /// Create tuner with custom configuration
    pub fn with_config(
        max_history: usize,
        min_adjustment_interval: Duration,
        pid_coefficients: PidCoefficients,
    ) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
            current_redundancy: Redundancy::default(),
            pid_coefficients,
            last_adjustment: None,
            min_adjustment_interval,
            loss_window: VecDeque::with_capacity(20),
            max_loss_window: 20,
        }
    }

    /// Add new network measurement and potentially adjust redundancy
    pub fn update(&mut self, metrics: NetworkMetrics) -> Redundancy {
        // Add to history, maintain size limit
        self.history.push_back(metrics);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        // Add to loss rate window
        self.loss_window.push_back(metrics.loss_rate);
        if self.loss_window.len() > self.max_loss_window {
            self.loss_window.pop_front();
        }

        // Check if enough time has passed for adjustment
        if let Some(last) = self.last_adjustment {
            if metrics.timestamp.duration_since(last) < self.min_adjustment_interval {
                return self.current_redundancy;
            }
        }

        // Calculate new redundancy using adaptive algorithm
        let new_redundancy = self.calculate_adaptive_redundancy(&metrics);

        self.current_redundancy = new_redundancy;
        self.last_adjustment = Some(metrics.timestamp);

        new_redundancy
    }

    /// Get current redundancy settings
    pub fn current_redundancy(&self) -> Redundancy {
        self.current_redundancy
    }

    /// Get recent loss rate trend (-1.0 to 1.0, negative = improving)
    pub fn loss_trend(&self) -> f32 {
        if self.loss_window.len() < 5 {
            return 0.0;
        }

        let recent_avg = self.loss_window.iter().rev().take(5).sum::<f32>() / 5.0;
        let older_avg = self.loss_window.iter().take(5).sum::<f32>() / 5.0;

        (recent_avg - older_avg).clamp(-1.0, 1.0)
    }

    /// Calculate adaptive redundancy using PID-style control
    fn calculate_adaptive_redundancy(&self, current: &NetworkMetrics) -> Redundancy {
        let target_loss_rate = 0.001; // Target 0.1% loss rate
        let current_loss = self.calculate_smoothed_loss_rate();

        // PID error calculation
        let error = current_loss - target_loss_rate;
        let integral_error = self.calculate_integral_error();
        let derivative_error = self.calculate_derivative_error();

        // PID output
        let pid_output = self.pid_coefficients.kp * error
            + self.pid_coefficients.ki * integral_error
            + self.pid_coefficients.kd * derivative_error;

        // Base redundancy adjustment
        let base_tx = self.current_redundancy.tx + pid_output;
        let base_rx = self.current_redundancy.rx + pid_output * 0.8; // RX slightly less responsive

        // Network condition modifiers
        let quality_modifier = self.calculate_quality_modifier(current);
        let bandwidth_modifier = self.calculate_bandwidth_modifier(current);
        let stability_modifier = self.calculate_stability_modifier(current);

        // Apply all modifiers
        let final_tx =
            (base_tx * quality_modifier * bandwidth_modifier * stability_modifier).clamp(0.01, 0.9);
        let final_rx =
            (base_rx * quality_modifier * bandwidth_modifier * stability_modifier).clamp(0.01, 0.9);

        Redundancy::new(final_tx, final_rx)
    }

    /// Calculate smoothed loss rate using exponential moving average
    fn calculate_smoothed_loss_rate(&self) -> f32 {
        if self.loss_window.is_empty() {
            return 0.0;
        }

        let alpha = 0.3; // Smoothing factor
        let mut ema = self.loss_window[0];

        for &loss in self.loss_window.iter().skip(1) {
            ema = alpha * loss + (1.0 - alpha) * ema;
        }

        ema
    }

    /// Calculate integral error for PID controller
    fn calculate_integral_error(&self) -> f32 {
        let target = 0.001;
        self.loss_window
            .iter()
            .map(|&loss| loss - target)
            .sum::<f32>()
            / self.loss_window.len().max(1) as f32
    }

    /// Calculate derivative error for PID controller
    fn calculate_derivative_error(&self) -> f32 {
        if self.loss_window.len() < 2 {
            return 0.0;
        }

        let recent = self.loss_window.back().unwrap();
        let previous = self.loss_window.get(self.loss_window.len() - 2).unwrap();

        recent - previous
    }

    /// Calculate quality-based modifier [0.5, 2.0]
    fn calculate_quality_modifier(&self, metrics: &NetworkMetrics) -> f32 {
        let quality = metrics.quality_score();
        // Poor quality -> higher redundancy
        (2.0 - quality).clamp(0.5, 2.0)
    }

    /// Calculate bandwidth-based modifier [0.8, 1.2]
    fn calculate_bandwidth_modifier(&self, metrics: &NetworkMetrics) -> f32 {
        // Higher bandwidth allows more redundancy
        if metrics.bandwidth_kbps > 1000 {
            1.2 // High bandwidth: allow more redundancy
        } else if metrics.bandwidth_kbps < 100 {
            0.8 // Low bandwidth: reduce redundancy
        } else {
            1.0 // Normal bandwidth: no modification
        }
    }

    /// Calculate stability-based modifier [0.7, 1.1]
    fn calculate_stability_modifier(&self, metrics: &NetworkMetrics) -> f32 {
        if metrics.is_stable() {
            0.9 // Stable network: can reduce redundancy slightly
        } else {
            1.1 // Unstable network: increase redundancy
        }
    }

    /// Get comprehensive tuning statistics
    pub fn get_statistics(&self) -> TunerStatistics {
        TunerStatistics {
            history_size: self.history.len(),
            average_loss_rate: self.loss_window.iter().sum::<f32>()
                / self.loss_window.len().max(1) as f32,
            loss_trend: self.loss_trend(),
            current_redundancy: self.current_redundancy,
            quality_score: self
                .history
                .back()
                .map(|m| m.quality_score())
                .unwrap_or(0.0),
            adjustment_count: self.history.len(),
        }
    }
}

impl Default for AdaptiveRedundancyTuner {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive statistics for tuner performance analysis
#[derive(Debug, Clone)]
pub struct TunerStatistics {
    pub history_size: usize,
    pub average_loss_rate: f32,
    pub loss_trend: f32,
    pub current_redundancy: Redundancy,
    pub quality_score: f32,
    pub adjustment_count: usize,
}

/// Legacy function for backward compatibility - now delegates to adaptive tuner
pub fn adaptive_raptorq_redundancy(rtt_ms: u32, loss: f32, prev: Redundancy) -> Redundancy {
    let metrics = NetworkMetrics::new(rtt_ms, 0, loss, 1000);
    let mut tuner = AdaptiveRedundancyTuner::new();
    tuner.current_redundancy = prev;
    tuner.update(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redundancy_clamped() {
        let prev = Redundancy { tx: 0.9, rx: 0.9 };
        let next = adaptive_raptorq_redundancy(1000, 1.0, prev);
        assert!(next.tx <= 0.9 && next.rx <= 0.9);
    }

    #[test]
    fn redundancy_creation_and_properties() {
        let redundancy = Redundancy::new(0.2, 0.3);
        assert_eq!(redundancy.tx, 0.2);
        assert_eq!(redundancy.rx, 0.3);
        assert_eq!(redundancy.max_redundancy(), 0.3);
        assert_eq!(redundancy.overhead_percent(), 25.0);
    }

    #[test]
    fn network_metrics_quality_score() {
        // Good network conditions
        let good_metrics = NetworkMetrics::new(50, 10, 0.001, 2000);
        assert!(good_metrics.quality_score() > 0.8);
        assert!(good_metrics.is_stable());

        // Poor network conditions
        let poor_metrics = NetworkMetrics::new(300, 80, 0.1, 100);
        assert!(poor_metrics.quality_score() < 0.5);
        assert!(!poor_metrics.is_stable());
    }

    #[test]
    fn adaptive_tuner_basic_functionality() {
        let mut tuner = AdaptiveRedundancyTuner::with_config(
            50,
            Duration::from_millis(1), // Very short interval for testing
            PidCoefficients::default(),
        );

        // Simulate good network conditions
        let good_metrics = NetworkMetrics::new(50, 10, 0.001, 1000);
        let redundancy1 = tuner.update(good_metrics);

        // Wait for adjustment interval
        std::thread::sleep(Duration::from_millis(10));

        // Simulate poor network conditions
        let poor_metrics = NetworkMetrics::new(200, 50, 0.1, 500);
        let redundancy2 = tuner.update(poor_metrics);

        // Should increase redundancy for poor conditions
        assert!(redundancy2.max_redundancy() > redundancy1.max_redundancy());
    }

    #[test]
    fn loss_trend_calculation() {
        let mut tuner = AdaptiveRedundancyTuner::new();

        // Add improving loss rates
        for i in (0..10).rev() {
            let loss = i as f32 * 0.01; // Decreasing loss
            let metrics = NetworkMetrics::new(100, 20, loss, 1000);
            tuner.update(metrics);
        }

        // Trend should be negative (improving)
        assert!(tuner.loss_trend() < 0.0);
    }

    #[test]
    fn tuner_statistics() {
        let mut tuner = AdaptiveRedundancyTuner::new();

        let metrics = NetworkMetrics::new(100, 20, 0.05, 1000);
        tuner.update(metrics);

        let stats = tuner.get_statistics();
        assert_eq!(stats.history_size, 1);
        assert_eq!(stats.average_loss_rate, 0.05);
    }

    #[test]
    fn bandwidth_affects_redundancy() {
        let mut low_bw_tuner = AdaptiveRedundancyTuner::new();
        let mut high_bw_tuner = AdaptiveRedundancyTuner::new();

        let low_bw_metrics = NetworkMetrics::new(100, 20, 0.02, 50); // 50 kbps
        let high_bw_metrics = NetworkMetrics::new(100, 20, 0.02, 5000); // 5 Mbps

        let low_bw_redundancy = low_bw_tuner.update(low_bw_metrics);
        let high_bw_redundancy = high_bw_tuner.update(high_bw_metrics);

        // High bandwidth should allow higher redundancy
        assert!(high_bw_redundancy.max_redundancy() >= low_bw_redundancy.max_redundancy());
    }

    #[test]
    fn adjustment_interval_respected() {
        let mut tuner = AdaptiveRedundancyTuner::with_config(
            50,
            Duration::from_secs(5), // 5 second minimum interval
            PidCoefficients::default(),
        );

        let metrics1 = NetworkMetrics::new(100, 20, 0.05, 1000);
        let redundancy1 = tuner.update(metrics1);

        // Immediate second update should return same redundancy
        let metrics2 = NetworkMetrics::new(200, 50, 0.1, 500);
        let redundancy2 = tuner.update(metrics2);

        assert_eq!(redundancy1.tx, redundancy2.tx);
        assert_eq!(redundancy1.rx, redundancy2.rx);
    }
}
