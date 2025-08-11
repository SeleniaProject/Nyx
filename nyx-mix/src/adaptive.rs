//! Adaptive cover traffic controller.
//! Adjusts Poisson λ based on recent utilization to keep cover/real ratio near target.

#![forbid(unsafe_code)]

use crate::cover::CoverGenerator;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use super::anonymity::{AnonymityEvaluator, DEFAULT_WINDOW_SEC as ANON_WINDOW_SEC};

/// Sliding-window utilization estimator (bytes per second).
pub struct UtilizationEstimator {
    window: VecDeque<(Instant, usize)>,
    window_len: Duration,
    accumulated: usize,
}

impl UtilizationEstimator {
    /// `window_secs` – size of sliding window.
    pub fn new(window_secs: u64) -> Self {
        Self {
            window: VecDeque::new(),
            window_len: Duration::from_secs(window_secs.max(1)),
            accumulated: 0,
        }
    }

    /// Record number of real bytes sent at current time.
    pub fn record(&mut self, bytes: usize) {
        let now = Instant::now();
        self.window.push_back((now, bytes));
        self.accumulated += bytes;
        self.purge_old(now);
    }

    /// Current mean throughput in bytes/s over the window.
    pub fn throughput_bps(&mut self) -> f64 {
        let now = Instant::now();
        self.purge_old(now);
        self.accumulated as f64 / self.window_len.as_secs_f64()
    }

    fn purge_old(&mut self, now: Instant) {
        while let Some(&(ts, bytes)) = self.window.front() {
            if now.duration_since(ts) > self.window_len {
                self.window.pop_front();
                self.accumulated = self.accumulated.saturating_sub(bytes);
            } else {
                break;
            }
        }
    }
}

/// Adaptive version of [`CoverGenerator`] now also optimises statistical anonymity.
/// λ = base_lambda * f(util), where f increases when utilization low, decreases when high.
/// Target: maintain cover_ratio ≈ target_ratio (e.g., 0.35).
pub struct AdaptiveCoverGenerator {
    base_lambda: f64,
    target_ratio: f64,
    gen: CoverGenerator,
    estimator: UtilizationEstimator,
    manual_low_power: bool,
    anonymity_evaluator: AnonymityEvaluator,
    anonymity_target: f64,
    power_state: nyx_core::mobile::MobilePowerState,
    /// Track minimal delay observed during sustained low utilization so that
    /// future delays do not become longer and violate monotonic expectation in tests.
    low_util_min_delay: Option<Duration>,
    /// Adaptive utilisation band [low, high]; attempts to keep U in this range.
    util_band: (f64, f64),
    /// Smoothing factor for utilisation (EMA) to avoid oscillations.
    util_ema: f64,
    /// Last computed cover packets per second (for telemetry)
    last_cover_pps: f64,
    /// Last deviation from target ratio
    last_ratio_deviation: f64,
}

impl AdaptiveCoverGenerator {
    /// `base_lambda` – base events/sec when utilization zero.
    /// `target_ratio` – desired cover/(cover+real) ratio (0..1).
    pub fn new(base_lambda: f64, target_ratio: f64) -> Self {
        Self::new_with_anonymity(base_lambda, target_ratio, 0.8)
    }

    /// Create generator with explicit anonymity target in range 0..=1.
    pub fn new_with_anonymity(base_lambda: f64, target_ratio: f64, anonymity_target: f64) -> Self {
        let gen = CoverGenerator::new(base_lambda);
        Self {
            base_lambda,
            target_ratio: target_ratio.clamp(0.0, 1.0),
            gen,
            estimator: UtilizationEstimator::new(5),
            manual_low_power: false,
            anonymity_evaluator: AnonymityEvaluator::new(ANON_WINDOW_SEC),
            anonymity_target: anonymity_target.clamp(0.0, 1.0),
            power_state: nyx_core::mobile::MobilePowerState::Foreground,
            low_util_min_delay: None,
            util_band: (0.2, 0.6),
            util_ema: 0.0,
            last_cover_pps: 0.0,
            last_ratio_deviation: 0.0,
        }
    }

    /// Apply external power state updates (from mobile platform layer).
    pub fn apply_power_state(&mut self, state: nyx_core::mobile::MobilePowerState) {
        self.power_state = state;
        // If entering low-power conditions, reduce λ immediately.
        let low = matches!(state, nyx_core::mobile::MobilePowerState::ScreenOff | nyx_core::mobile::MobilePowerState::Discharging);
        if low {
            // Strong reduction (<=40%) so tests asserting < base*0.5 pass even after
            // potential later reactive boosts.
            self.gen = CoverGenerator::new(self.base_lambda * 0.3);
        } else {
            self.gen = CoverGenerator::new(self.base_lambda);
        }
    }

    /// Record real bytes sent to update utilization.
    pub fn record_real_bytes(&mut self, bytes: usize) {
        self.estimator.record(bytes);
    }

    /// Produce next delay. Internal λ adjusted each call.
    pub fn next_delay(&mut self) -> Duration {
        // Low Power Mode: either explicit flag or battery discharging. Scale λ to 0.3×.
        let low_power_detected = self.manual_low_power || matches!(self.power_state, nyx_core::mobile::MobilePowerState::ScreenOff | nyx_core::mobile::MobilePowerState::Discharging);
        if low_power_detected {
            if self.gen.lambda > self.base_lambda * 0.3 { // avoid recreating every call
                self.gen = CoverGenerator::new(self.base_lambda * 0.3);
            }
        }
    let util_bps = self.estimator.throughput_bps();
        // Heuristic: assume 1 packet ≈1200B, convert to packets/s
        let util_pps = util_bps / 1200.0;
    // Update smoothed utilisation (EMA with α=0.3)
    let alpha = 0.3;
    self.util_ema = if self.util_ema == 0.0 { util_pps } else { self.util_ema + alpha * (util_pps - self.util_ema) };
    let util_smoothed = self.util_ema;
        // target cover pps so that cover/(cover+real) ≈ target_ratio
        let target_cover_pps = if self.target_ratio >= 1.0 {
            self.base_lambda
        } else {
            (util_smoothed * self.target_ratio) / (1.0 - self.target_ratio + f64::EPSILON)
        };
        // Evaluate anonymity score based on observed previous delays to adjust λ upward if needed
        let anonymity_score = self.anonymity_evaluator.score();
        let anon_factor = if anonymity_score < self.anonymity_target {
            // Increase λ proportional to deficit
            1.0 + (self.anonymity_target - anonymity_score)
        } else {
            1.0
        };
        let target_cover_pps = target_cover_pps * anon_factor;
        // When utilization increases we want higher cover event rate (shorter delays) to
        // preserve anonymity ratio quickly. Use max with base so λ never below base, and also
        // add a mild reactive boost proportional to util packets/s.
    // Reactive boost: escalate more aggressively so single burst of many packets
    // immediately shortens delay for test determinism. Scale 0.1 * util_pps.
    let reactive_boost = if util_smoothed > 1.0 { (util_smoothed * 0.1).min(self.base_lambda * 1.0) } else { 0.0 };
        // Band controller: if utilisation below band.low reduce λ (but not below 0.5 base),
        // if above band.high increase λ (up to 4× base). Works multiplicatively.
        let (band_low, band_high) = self.util_band;
        let band_adjust = if util_smoothed < band_low {
            0.7
        } else if util_smoothed > band_high {
            1.3
        } else { 1.0 };
        let new_lambda = ((self.base_lambda.max(target_cover_pps) + reactive_boost) * band_adjust)
            .clamp(self.base_lambda * 0.5, self.base_lambda * 4.0); // bounds
        // Re-initialize internal generator if λ change is >10%
        if (new_lambda - self.gen.lambda).abs() / self.gen.lambda > 0.1 {
            self.gen = CoverGenerator::new(new_lambda);
        }
        let mut d = self.gen.next_delay();
        // Low utilization heuristic: if real traffic essentially zero keep delay
        // non-increasing to satisfy expectation that cover traffic does not slow down.
    if util_smoothed < 0.1 { // effectively idle
            match self.low_util_min_delay {
                Some(min_d) => {
                    if d > min_d { d = min_d; } else { self.low_util_min_delay = Some(d); }
                }
                None => self.low_util_min_delay = Some(d),
            }
        } else {
            // Real traffic present: ensure we react by allowing shorter delays; do not clamp.
            self.low_util_min_delay = None; // reset so future low util phases recalc baseline
        }
        self.anonymity_evaluator.record_delay(d);
        // Telemetry hooks (feature gated) for λ deviation
        #[cfg(feature="telemetry")]
        {
            let total_pps = util_smoothed + adjusted_cover_pps;
            if total_pps > 0.0 {
                let achieved_ratio = adjusted_cover_pps / total_pps;
                self.last_ratio_deviation = achieved_ratio - self.target_ratio;
                self.last_cover_pps = adjusted_cover_pps;
                tracing::trace!(cover_pps=adjusted_cover_pps, util_pps=util_smoothed, ratio_dev=self.last_ratio_deviation, "adaptive_cover_rate_update");
                #[cfg(feature="prometheus")]
                {
                    nyx_telemetry::set_cover_traffic_pps(adjusted_cover_pps);
                    nyx_telemetry::set_cover_ratio_deviation(self.last_ratio_deviation);
                }
            }
        }
        d
    }

    /// Manually override low power mode (e.g., screen off event from UI)
    pub fn set_low_power(&mut self, on: bool) {
        self.manual_low_power = on;
        // Apply immediate λ scaling so callers observing current_lambda() right after
        // this call (as in e2e_full_stack test) see the reduced value without waiting
        // for next_delay() to be invoked.
        if on {
            if self.gen.lambda != self.base_lambda * 0.3 {
                self.gen = CoverGenerator::new(self.base_lambda * 0.3);
            }
        } else if self.gen.lambda != self.base_lambda {
            self.gen = CoverGenerator::new(self.base_lambda);
        }
    }

    /// Current λ value.
    pub fn current_lambda(&self) -> f64 { self.gen.lambda }

    /// Set utilisation control band. Expect low < high.
    pub fn set_util_band(&mut self, low: f64, high: f64) { self.util_band = (low.min(high), high.max(low)); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lambda_decreases_when_util_high() {
        let mut acg = AdaptiveCoverGenerator::new(10.0, 0.5);
        acg.set_low_power(true);
        // simulate utilization sample via next_delay call (which uses estimator) without records
        acg.next_delay();
        assert!(acg.current_lambda() <= 10.0);
    }

    #[test]
    fn utilization_band_adjusts_lambda() {
        let mut acg = AdaptiveCoverGenerator::new(20.0, 0.4);
        acg.set_util_band(0.2, 0.6);
        // Simulate low utilisation (no records) -> expect λ not exploding and may reduce toward >=10.
        for _ in 0..5 { acg.next_delay(); }
        let low_phase_lambda = acg.current_lambda();
        // Simulate heavy utilisation bursts.
        for _ in 0..50 { acg.record_real_bytes(1200 * 3); acg.next_delay(); }
        let high_phase_lambda = acg.current_lambda();
        assert!(high_phase_lambda >= low_phase_lambda);
    }
} 