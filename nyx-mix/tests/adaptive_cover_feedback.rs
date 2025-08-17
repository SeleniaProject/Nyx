
//! Adaptive Cover Traffic Validation Tests
//!
//! This test suite validates the adaptive cover traffic algorithm against
//! the mathematical properties and performance guarantees specified in
//! the design specification (docs/adaptive_cover_traffic_spec.md).

use nyx_mix::{cover::poisson_rate, cover_adaptive::apply_utilization, MixConfig};
use rand::thread_rng;

/// Test the fundamental monotonicity property: λ(u₁) ≤ λ(u₂) for u₁ ≤ u₂
/// This is critical for anonymity preservation.
#[test]
fn adaptive_cover_utilization_feedback_non_decreasing_lambda() {
	let cfg = MixConfig { base_cover_lambda: 10.0, low_power_ratio: 0.5, ..Default::default() };
	let mut prev = f32::MIN;
	for i in 0..=20 { // 0.0..=1.0 step
		let u = i as f32 / 20.0;
		let cur = apply_utilization(&cfg, u, false);
		assert!(cur >= prev, "Monotonicity violation: u={} prev={} cur={}", u, prev, cur);
		prev = cur;
	}
}

/// Test power mode scaling maintains correct ratio.
/// Validates battery optimization while preserving anonymity.
#[test]
fn low_power_reduces_base_rate() {
	let cfg = MixConfig { base_cover_lambda: 12.0, low_power_ratio: 0.3, ..Default::default() };
	let u = 0.4;
	let hi = apply_utilization(&cfg, u, false);
	let lo = apply_utilization(&cfg, u, true);
	assert!(lo < hi, "Low power should reduce rate");
	
	// Verify exact scaling at baseline (u=0)
	let lo0 = apply_utilization(&cfg, 0.0, true);
	let expected = 12.0 * 0.3;
	assert!((lo0 - expected).abs() < 1e-6, "Expected {}, got {}", expected, lo0);
	
	// Verify ratio maintained across utilization levels
	let ratio = lo / hi;
	let expected_ratio = cfg.low_power_ratio;
	assert!((ratio - expected_ratio).abs() < 1e-6, "Power ratio should be {}, got {}", expected_ratio, ratio);
}

/// Test input validation and error handling.
/// Ensures algorithm robustness against invalid inputs.
#[test]
fn utilization_is_clamped() {
	let cfg = MixConfig { base_cover_lambda: 5.0, low_power_ratio: 0.5, ..Default::default() };
	let below = apply_utilization(&cfg, -1.0, false);
	let within = apply_utilization(&cfg, 0.0, false);
	let above = apply_utilization(&cfg, 10.0, false);
	let max_valid = apply_utilization(&cfg, 1.0, false);
	
	// Below range should clamp to minimum
	assert!((below - within).abs() < 1e-6, "Below-range input should clamp to 0.0");
	// Above range should clamp to maximum  
	assert!((above - max_valid).abs() < 1e-6, "Above-range input should clamp to 1.0");
}

/// Test configuration parameter validation.
/// Ensures parameters stay within meaningful bounds.
#[test]
fn config_validation_ranges() {
	// Valid configuration should pass
	MixConfig::default().validate_ranges().unwrap();
	
	// Invalid low_power_ratio (above 1.0)
	let bad = MixConfig { low_power_ratio: 1.2, ..Default::default() };
	assert!(bad.validate_ranges().is_err(), "Should reject low_power_ratio > 1.0");
	
	// Invalid base_cover_lambda (too high)
	let bad2 = MixConfig { base_cover_lambda: 100_000.0, ..Default::default() };
	assert!(bad2.validate_ranges().is_err(), "Should reject excessive base_cover_lambda");
	
	// Invalid low_power_ratio (negative)
	let bad3 = MixConfig { low_power_ratio: -0.1, ..Default::default() };
	assert!(bad3.validate_ranges().is_err(), "Should reject negative low_power_ratio");
}

/// Test mathematical formula compliance.
/// Validates exact formula: λ(u) = λ_base × (1 + u) × power_factor
#[test] 
fn formula_mathematical_compliance() {
	let cfg = MixConfig::default();
	
	for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
		// Normal mode: power_factor = 1.0
		let normal = apply_utilization(&cfg, u, false);
		let expected = cfg.base_cover_lambda * (1.0 + u);
		assert!((normal - expected).abs() < f32::EPSILON, 
			"Formula mismatch (normal): u={}, expected={}, got={}", u, expected, normal);
		
		// Low power mode: power_factor = low_power_ratio  
		let power = apply_utilization(&cfg, u, true);
		let expected_power = cfg.base_cover_lambda * cfg.low_power_ratio * (1.0 + u);
		assert!((power - expected_power).abs() < f32::EPSILON,
			"Formula mismatch (power): u={}, expected={}, got={}", u, expected_power, power);
	}
}

/// Test bounded response property (2:1 ratio).
/// Critical for bandwidth efficiency.
#[test]
fn bounded_response_ratio() {
	let cfg = MixConfig::default();
	let min_rate = apply_utilization(&cfg, 0.0, false);
	let max_rate = apply_utilization(&cfg, 1.0, false);
	let ratio = max_rate / min_rate;
	
	assert!((ratio - 2.0).abs() < f32::EPSILON, 
		"Expected exactly 2:1 ratio, got {}", ratio);
}

/// Test Poisson distribution properties for cover traffic generation.
/// Validates statistical properties match theoretical expectations.
#[test]
fn poisson_rate_matches_lambda_on_average() {
	let mut rng = thread_rng();
	let lambda = 6.0;
	let trials = 2000; // keep test fast
	let mut sum: u64 = 0;
	for _ in 0..trials { sum += poisson_rate(lambda, &mut rng) as u64; }
	let avg = sum as f64 / trials as f64;
	// Within reasonable tolerance for Poisson fluctuation
	assert!((avg - lambda as f64).abs() < 0.5, "avg={} lambda={}", avg, lambda);
}

/// Performance benchmark - algorithm should be O(1) and fast.
/// Critical for real-time network adaptation.
#[test]
fn performance_benchmark() {
	let config = MixConfig::default();
	let start = std::time::Instant::now();
	
	// Perform many computations
	for i in 0..10_000 {
		let utilization = (i % 1000) as f32 / 1000.0;
		let _rate = apply_utilization(&config, utilization, i % 2 == 0);
	}
	
	let duration = start.elapsed();
	let per_call_ns = duration.as_nanos() / 10_000;
	
	// Should complete in under 1 microsecond per call
	assert!(per_call_ns < 1_000, 
		"Performance requirement violated: {} ns per call (limit: 1000 ns)", per_call_ns);
}

