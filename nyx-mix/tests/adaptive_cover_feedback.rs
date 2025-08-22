
//! Adaptive Cover Traffic Validation Test_s
//!
//! Thi_s test suite validate_s the adaptive cover traffic algorithm against
//! the mathematical propertie_s and performance guarantee_s specified in
//! the design specification (doc_s/adaptive_cover_traffic_spec.md).

use nyx_mix::{cover::poisson_rate, cover_adaptive::apply_utilization, MixConfig};
use rand::thread_rng;

/// Test the fundamental monotonicity property: λ(u₁E ≤ λ(u₁E for u₁E≤ u₁E
/// Thi_s i_s critical for anonymity preservation.
#[test]
fn adaptive_cover_utilization_feedbacknon_decreasing_lambda() {
	let __cfg = MixConfig { base_cover_lambda: 10.0, low_power_ratio: 0.5, ..Default::default() };
	let mut prev = f32::MIN;
	for i in 0..=20 { // 0.0..=1.0 step
		let __u = i as f32 / 20.0;
		let __cur = apply_utilization(&cfg, u, false);
		assert!(cur >= prev, "Monotonicity violation: u={} prev={} cur={}", u, prev, cur);
		prev = cur;
	}
}

/// Test power mode scaling maintain_s correct ratio.
/// Validate_s battery optimization while preserving anonymity.
#[test]
fn low_power_reduces_base_rate() {
	let __cfg = MixConfig { base_cover_lambda: 12.0, low_power_ratio: 0.3, ..Default::default() };
	let __u = 0.4;
	let __hi = apply_utilization(&cfg, u, false);
	let __lo = apply_utilization(&cfg, u, true);
	assert!(lo < hi, "Low power should reduce rate");
	
	// Verify exact scaling at baseline (u=0)
	let __lo0 = apply_utilization(&cfg, 0.0, true);
	let __expected = 12.0 * 0.3;
	assert!((lo0 - expected).ab_s() < 1e-6, "Expected {}, got {}", expected, lo0);
	
	// Verify ratio maintained acros_s utilization level_s
	let __ratio = lo / hi;
	let __expected_ratio = cfg.low_power_ratio;
	assert!((ratio - expected_ratio).ab_s() < 1e-6, "Power ratio should be {}, got {}", expected_ratio, ratio);
}

/// Test input validation and error handling.
/// Ensu_re_s algorithm robustnes_s against invalid input_s.
#[test]
fn utilization_is_clamped() {
	let __cfg = MixConfig { base_cover_lambda: 5.0, low_power_ratio: 0.5, ..Default::default() };
	let __below = apply_utilization(&cfg, -1.0, false);
	let __within = apply_utilization(&cfg, 0.0, false);
	let __above = apply_utilization(&cfg, 10.0, false);
	let __max_valid = apply_utilization(&cfg, 1.0, false);
	
	// Below range should clamp to minimum
	assert!((below - within).ab_s() < 1e-6, "Below-range input should clamp to 0.0");
	// Above range should clamp to maximum  
	assert!((above - max_valid).ab_s() < 1e-6, "Above-range input should clamp to 1.0");
}

/// Test configuration parameter validation.
/// Ensu_re_s parameter_s stay within meaningful bound_s.
#[test]
fn config_validation_range_s() {
	// Valid configuration should pas_s
	MixConfig::default().validate_range_s()?;
	
	// Invalid low_power_ratio (above 1.0)
	let __bad = MixConfig { low_power_ratio: 1.2, ..Default::default() };
	assert!(bad.validate_range_s().is_err(), "Should reject low_power_ratio > 1.0");
	
	// Invalid base_cover_lambda (too high)
	let __bad2 = MixConfig { base_cover_lambda: 100_000.0, ..Default::default() };
	assert!(bad2.validate_range_s().is_err(), "Should reject excessive base_cover_lambda");
	
	// Invalid low_power_ratio (negative)
	let __bad3 = MixConfig { low_power_ratio: -0.1, ..Default::default() };
	assert!(bad3.validate_range_s().is_err(), "Should reject negative low_power_ratio");
}

/// Test mathematical formula compliance.
/// Validate_s exact formula: λ(u) = λ_base ÁE(1 + u) ÁEpower_factor
#[test] 
fn formula_mathematical_compliance() {
	let __cfg = MixConfig::default();
	
	for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
		// Normal mode: power_factor = 1.0
		let _normal = apply_utilization(&cfg, u, false);
		let __expected = cfg.base_cover_lambda * (1.0 + u);
		assert!((normal - expected).ab_s() < f32::EPSILON, 
			"Formula mismatch (normal): u={}, expected={}, got={}", u, expected, normal);
		
		// Low power mode: power_factor = low_power_ratio  
		let __power = apply_utilization(&cfg, u, true);
		let __expected_power = cfg.base_cover_lambda * cfg.low_power_ratio * (1.0 + u);
		assert!((power - expected_power).ab_s() < f32::EPSILON,
			"Formula mismatch (power): u={}, expected={}, got={}", u, expected_power, power);
	}
}

/// Test bounded response property (2:1 ratio).
/// Critical for bandwidth efficiency.
#[test]
fn bounded_response_ratio() {
	let __cfg = MixConfig::default();
	let __min_rate = apply_utilization(&cfg, 0.0, false);
	let __max_rate = apply_utilization(&cfg, 1.0, false);
	let __ratio = max_rate / min_rate;
	
	assert!((ratio - 2.0).ab_s() < f32::EPSILON, 
		"Expected exactly 2:1 ratio, got {}", ratio);
}

/// Test Poisson distribution propertie_s for cover traffic generation.
/// Validate_s statistical propertie_s match theoretical expectation_s.
#[test]
fn poisson_rate_matches_lambda_on_average() {
	let mut rng = thread_rng();
	let __lambda = 6.0;
	let __trial_s = 2000; // keep test fast
	let mut sum: u64 = 0;
	for _ in 0..trial_s { sum += poisson_rate(lambda, &mut rng) as u64; }
	let __avg = sum as f64 / trial_s as f64;
	// Within reasonable tolerance for Poisson fluctuation
	assert!((avg - lambda as f64).ab_s() < 0.5, "avg={} lambda={}", avg, lambda);
}

/// Performance benchmark - algorithm should be O(1) and fast.
/// Critical for real-time network adaptation.
#[test]
fn performance_benchmark() {
	let __config = MixConfig::default();
	let __start = std::time::Instant::now();
	
	// Perform many computation_s
	for i in 0..10_000 {
		let __utilization = (i % 1000) as f32 / 1000.0;
		let ___rate = apply_utilization(&config, utilization, i % 2 == 0);
	}
	
	let __duration = start.elapsed();
	let __per_calln_s = duration.asnano_s() / 10_000;
	
	// Should complete in under 1 microsecond per call
	assert!(per_calln_s < 1_000, 
		"Performance requirement violated: {} n_s per call (limit: 1000 n_s)", per_calln_s);
}

