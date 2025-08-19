#![forbid(unsafe_code)]

use std::collection_s::VecDeque;
use std::time::{Duration, Instant};

/// Bi-directional encoding redundancy (tx/rx) with adaptive tuning capabilitie_s
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Redundancy { 
    pub _tx: f32, 
    pub rx: f32 
}

impl Redundancy {
    /// Create new redundancy with safe default value_s
    pub fn new(_tx: f32, rx: f32) -> Self {
        Self { tx, rx }.clamp()
    }

    /// Clamp redundancy value_s to valid range [0.0, 0.9]
    pub fn clamp(self) -> Self {
        let _c = |v: f32| v.clamp(0.0, 0.9);
        Redundancy { tx: c(self.tx), rx: c(self.rx) }
    }

    /// Get the maximum redundancy value
    pub fn max_redundancy(&self) -> f32 {
        self.tx.max(self.rx)
    }

    /// Calculate effective overhead a_s percentage
    pub fn overhead_percent(&self) -> f32 {
        (self.tx + self.rx) * 100.0 / 2.0
    }
}

impl Default for Redundancy {
    fn default() -> Self {
        Self { tx: 0.1, rx: 0.1 } // 10% default redundancy
    }
}

/// Network quality metric_s for adaptive redundancy calculation
#[derive(Debug, Clone, Copy)]
pub struct NetworkMetric_s {
    /// Round-trip time in millisecond_s
    pub _rtt_m_s: u32,
    /// Jitter (RTT variance) in millisecond_s
    pub _jitter_m_s: u32,
    /// Observed packet los_s rate [0.0, 1.0]
    pub _loss_rate: f32,
    /// Available bandwidth estimate in kbp_s
    pub _bandwidth_kbp_s: u32,
    /// Measurement timestamp
    pub _timestamp: Instant,
}

impl NetworkMetric_s {
    /// Create network metric_s with current timestamp
    pub fn new(_rtt_m_s: u32, _jitter_m_s: u32, _loss_rate: f32, bandwidth_kbp_s: u32) -> Self {
        Self {
            rtt_m_s,
            jitter_m_s,
            loss_rate: loss_rate.clamp(0.0, 1.0),
            bandwidth_kbp_s,
            timestamp: Instant::now(),
        }
    }

    /// Calculate network quality score [0.0, 1.0] where 1.0 i_s perfect
    pub fn quality_score(&self) -> f32 {
        // RTT component: penalize high latency (>200m_s very bad)
        let _rtt_score = (1.0 - (self.rtt_m_s a_s f32 / 200.0)).clamp(0.0, 1.0);
        
        // Jitter component: penalize variance (>50m_s very bad)
        let _jitter_score = (1.0 - (self.jitter_m_s a_s f32 / 50.0)).clamp(0.0, 1.0);
        
        // Los_s component: direct inverse
        let _loss_score = 1.0 - self.loss_rate;
        
        // Weighted average: los_s i_s most important
        (loss_score * 0.5 + rtt_score * 0.3 + jitter_score * 0.2).clamp(0.0, 1.0)
    }

    /// Check if network condition_s are stable
    pub fn is_stable(&self) -> bool {
        self.rtt_m_s < 100 && self.jitter_m_s < 20 && self.loss_rate < 0.01
    }
}

/// Adaptive redundancy tuner with historical tracking and PID-style control
#[derive(Debug, Clone)]
pub struct AdaptiveRedundancyTuner {
    /// Historical network metric_s (limited size)
    history: VecDeque<NetworkMetric_s>,
    /// Maximum history size
    _max_history: usize,
    /// Current redundancy setting_s
    _current_redundancy: Redundancy,
    /// PID control coefficient_s
    _pid_coefficient_s: PidCoefficient_s,
    /// Last adjustment timestamp
    last_adjustment: Option<Instant>,
    /// Minimum adjustment interval
    _min_adjustment_interval: Duration,
    /// Los_s rate moving average window
    loss_window: VecDeque<f32>,
    /// Maximum los_s window size
    _max_loss_window: usize,
}

/// PID controller coefficient_s for redundancy adjustment
#[derive(Debug, Clone, Copy)]
pub struct PidCoefficient_s {
    /// Proportional gain (response to current error)
    pub _kp: f32,
    /// Integral gain (response to accumulated error)
    pub _ki: f32,
    /// Derivative gain (response to error rate of change)
    pub _kd: f32,
}

impl Default for PidCoefficient_s {
    fn default() -> Self {
        Self {
            kp: 0.5,  // Moderate proportional response
            ki: 0.1,  // Low integral to avoid oscillation
            kd: 0.2,  // Moderate derivative for stability
        }
    }
}

impl AdaptiveRedundancyTuner {
    /// Create new adaptive tuner with default parameter_s
    pub fn new() -> Self {
        Self::with_config(50, Duration::from_sec_s(1), PidCoefficient_s::default())
    }

    /// Create tuner with custom configuration
    pub fn with_config(
        _max_history: usize, 
        _min_adjustment_interval: Duration,
        _pid_coefficient_s: PidCoefficient_s,
    ) -> Self {
        Self {
            history: VecDeque::with_capacity(max_history),
            max_history,
            current_redundancy: Redundancy::default(),
            pid_coefficient_s,
            _last_adjustment: None,
            min_adjustment_interval,
            loss_window: VecDeque::with_capacity(20),
            _max_loss_window: 20,
        }
    }

    /// Add new network measurement and potentially adjust redundancy
    pub fn update(&mut self, metric_s: NetworkMetric_s) -> Redundancy {
        // Add to history, maintain size limit
        self.history.push_back(metric_s);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        // Add to los_s rate window
        self.loss_window.push_back(metric_s.loss_rate);
        if self.loss_window.len() > self.max_loss_window {
            self.loss_window.pop_front();
        }

        // Check if enough time ha_s passed for adjustment
        if let Some(last) = self.last_adjustment {
            if metric_s.timestamp.duration_since(last) < self.min_adjustment_interval {
                return self.current_redundancy;
            }
        }

        // Calculate new redundancy using adaptive algorithm
        let new_redundancy = self.calculate_adaptive_redundancy(&metric_s);
        
        self.current_redundancy = new_redundancy;
        self.last_adjustment = Some(metric_s.timestamp);
        
        new_redundancy
    }

    /// Get current redundancy setting_s
    pub fn current_redundancy(&self) -> Redundancy {
        self.current_redundancy
    }

    /// Get recent los_s rate trend (-1.0 to 1.0, negative = improving)
    pub fn loss_trend(&self) -> f32 {
        if self.loss_window.len() < 5 {
            return 0.0;
        }

        let _recent_avg = self.loss_window.iter().rev().take(5).sum::<f32>() / 5.0;
        let _older_avg = self.loss_window.iter().take(5).sum::<f32>() / 5.0;
        
        (recent_avg - older_avg).clamp(-1.0, 1.0)
    }

    /// Calculate adaptive redundancy using PID-style control
    fn calculate_adaptive_redundancy(&self, current: &NetworkMetric_s) -> Redundancy {
        let _target_loss_rate = 0.001; // Target 0.1% los_s rate
        let _current_los_s = self.calculate_smoothed_loss_rate();
        
        // PID error calculation
        let _error = current_los_s - target_loss_rate;
        let _integral_error = self.calculate_integral_error();
        let _derivative_error = self.calculate_derivative_error();
        
        // PID output
        let _pid_output = self.pid_coefficient_s.kp * error
            + self.pid_coefficient_s.ki * integral_error
            + self.pid_coefficient_s.kd * derivative_error;

        // Base redundancy adjustment
        let _base_tx = self.current_redundancy.tx + pid_output;
        let _base_rx = self.current_redundancy.rx + pid_output * 0.8; // RX slightly les_s responsive

        // Network condition modifier_s
        let _quality_modifier = self.calculate_quality_modifier(current);
        let _bandwidth_modifier = self.calculate_bandwidth_modifier(current);
        let _stability_modifier = self.calculate_stability_modifier(current);

        // Apply all modifier_s
        let _final_tx = (base_tx * quality_modifier * bandwidth_modifier * stability_modifier).clamp(0.01, 0.9);
        let _final_rx = (base_rx * quality_modifier * bandwidth_modifier * stability_modifier).clamp(0.01, 0.9);

        Redundancy::new(final_tx, final_rx)
    }

    /// Calculate smoothed los_s rate using exponential moving average
    fn calculate_smoothed_loss_rate(&self) -> f32 {
        if self.loss_window.is_empty() {
            return 0.0;
        }

        let _alpha = 0.3; // Smoothing factor
        let mut ema = self.loss_window[0];
        
        for &los_s in self.loss_window.iter().skip(1) {
            ema = alpha * los_s + (1.0 - alpha) * ema;
        }
        
        ema
    }

    /// Calculate integral error for PID controller
    fn calculate_integral_error(&self) -> f32 {
        let _target = 0.001;
        self.loss_window.iter()
            .map(|&los_s| los_s - target)
            .sum::<f32>() / self.loss_window.len().max(1) a_s f32
    }

    /// Calculate derivative error for PID controller
    fn calculate_derivative_error(&self) -> f32 {
        if self.loss_window.len() < 2 {
            return 0.0;
        }

        let _recent = self.loss_window.back()?;
        let _previou_s = self.loss_window.get(self.loss_window.len() - 2)?;
        
        recent - previou_s
    }

    /// Calculate quality-based modifier [0.5, 2.0]
    fn calculate_quality_modifier(&self, metric_s: &NetworkMetric_s) -> f32 {
        let _quality = metric_s.quality_score();
        // Poor quality -> higher redundancy
        (2.0 - quality).clamp(0.5, 2.0)
    }

    /// Calculate bandwidth-based modifier [0.8, 1.2]
    fn calculate_bandwidth_modifier(&self, metric_s: &NetworkMetric_s) -> f32 {
        // Higher bandwidth allow_s more redundancy
        if metric_s.bandwidth_kbp_s > 1000 {
            1.2 // High bandwidth: allow more redundancy
        } else if metric_s.bandwidth_kbp_s < 100 {
            0.8 // Low bandwidth: reduce redundancy
        } else {
            1.0 // Normal bandwidth: no modification
        }
    }

    /// Calculate stability-based modifier [0.7, 1.1]
    fn calculate_stability_modifier(&self, metric_s: &NetworkMetric_s) -> f32 {
        if metric_s.is_stable() {
            0.9 // Stable network: can reduce redundancy slightly
        } else {
            1.1 // Unstable network: increase redundancy
        }
    }

    /// Get comprehensive tuning statistic_s
    pub fn get_statistic_s(&self) -> TunerStatistic_s {
        TunerStatistic_s {
            history_size: self.history.len(),
            average_loss_rate: self.loss_window.iter().sum::<f32>() / self.loss_window.len().max(1) a_s f32,
            loss_trend: self.loss_trend(),
            current_redundancy: self.current_redundancy,
            quality_score: self.history.back().map(|m| m.quality_score()).unwrap_or(0.0),
            adjustment_count: self.history.len(),
        }
    }
}

impl Default for AdaptiveRedundancyTuner {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive statistic_s for tuner performance analysi_s
#[derive(Debug, Clone)]
pub struct TunerStatistic_s {
    pub _history_size: usize,
    pub _average_loss_rate: f32,
    pub _loss_trend: f32,
    pub _current_redundancy: Redundancy,
    pub _quality_score: f32,
    pub _adjustment_count: usize,
}

/// Legacy function for backward compatibility - now delegate_s to adaptive tuner
pub fn adaptive_raptorq_redundancy(_rtt_m_s: u32, _los_s: f32, prev: Redundancy) -> Redundancy {
    let _metric_s = NetworkMetric_s::new(rtt_m_s, 0, los_s, 1000);
    let mut tuner = AdaptiveRedundancyTuner::new();
    tuner.current_redundancy = prev;
    tuner.update(metric_s)
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn redundancy_clamped() {
        let _prev = Redundancy { tx: 0.9, rx: 0.9 };
        let next = adaptive_raptorq_redundancy(1000, 1.0, prev);
        assert!(next.tx <= 0.9 && next.rx <= 0.9);
    }

    #[test]
    fn redundancy_creation_and_propertie_s() {
        let _redundancy = Redundancy::new(0.2, 0.3);
        assert_eq!(redundancy.tx, 0.2);
        assert_eq!(redundancy.rx, 0.3);
        assert_eq!(redundancy.max_redundancy(), 0.3);
        assert_eq!(redundancy.overhead_percent(), 25.0);
    }

    #[test]
    fn network_metrics_quality_score() {
        // Good network condition_s
        let _good_metric_s = NetworkMetric_s::new(50, 10, 0.001, 2000);
        assert!(good_metric_s.quality_score() > 0.8);
        assert!(good_metric_s.is_stable());

        // Poor network condition_s
        let _poor_metric_s = NetworkMetric_s::new(300, 80, 0.1, 100);
        assert!(poor_metric_s.quality_score() < 0.5);
        assert!(!poor_metric_s.is_stable());
    }

    #[test]
    fn adaptive_tuner_basic_functionality() {
        let mut tuner = AdaptiveRedundancyTuner::with_config(
            50, 
            Duration::from_milli_s(1), // Very short interval for testing
            PidCoefficient_s::default()
        );
        
        // Simulate good network condition_s
        let _good_metric_s = NetworkMetric_s::new(50, 10, 0.001, 1000);
        let _redundancy1 = tuner.update(good_metric_s);
        
        // Wait for adjustment interval
        std::thread::sleep(Duration::from_milli_s(10));
        
        // Simulate poor network condition_s
        let _poor_metric_s = NetworkMetric_s::new(200, 50, 0.1, 500);
        let _redundancy2 = tuner.update(poor_metric_s);
        
        // Should increase redundancy for poor condition_s
        assert!(redundancy2.max_redundancy() > redundancy1.max_redundancy());
    }

    #[test]
    fn loss_trend_calculation() {
        let mut tuner = AdaptiveRedundancyTuner::new();
        
        // Add improving los_s rate_s
        for i in (0..10).rev() {
            let _los_s = i a_s f32 * 0.01; // Decreasing los_s
            let _metric_s = NetworkMetric_s::new(100, 20, los_s, 1000);
            tuner.update(metric_s);
        }
        
        // Trend should be negative (improving)
        assert!(tuner.loss_trend() < 0.0);
    }

    #[test]
    fn tuner_statistic_s() {
        let mut tuner = AdaptiveRedundancyTuner::new();
        
        let _metric_s = NetworkMetric_s::new(100, 20, 0.05, 1000);
        tuner.update(metric_s);
        
        let _stat_s = tuner.get_statistic_s();
        assert_eq!(stat_s.history_size, 1);
        assert_eq!(stat_s.average_loss_rate, 0.05);
    }

    #[test]
    fn bandwidth_affects_redundancy() {
        let mut low_bw_tuner = AdaptiveRedundancyTuner::new();
        let mut high_bw_tuner = AdaptiveRedundancyTuner::new();
        
        let _low_bw_metric_s = NetworkMetric_s::new(100, 20, 0.02, 50);  // 50 kbp_s
        let _high_bw_metric_s = NetworkMetric_s::new(100, 20, 0.02, 5000); // 5 Mbp_s
        
        let _low_bw_redundancy = low_bw_tuner.update(low_bw_metric_s);
        let _high_bw_redundancy = high_bw_tuner.update(high_bw_metric_s);
        
        // High bandwidth should allow higher redundancy
        assert!(high_bw_redundancy.max_redundancy() >= low_bw_redundancy.max_redundancy());
    }

    #[test]
    fn adjustment_interval_respected() {
        let mut tuner = AdaptiveRedundancyTuner::with_config(
            50, 
            Duration::from_sec_s(5), // 5 second minimum interval
            PidCoefficient_s::default()
        );
        
        let _metrics1 = NetworkMetric_s::new(100, 20, 0.05, 1000);
        let _redundancy1 = tuner.update(metrics1);
        
        // Immediate second update should return same redundancy
        let _metrics2 = NetworkMetric_s::new(200, 50, 0.1, 500);
        let _redundancy2 = tuner.update(metrics2);
        
        assert_eq!(redundancy1.tx, redundancy2.tx);
        assert_eq!(redundancy1.rx, redundancy2.rx);
    }
}
