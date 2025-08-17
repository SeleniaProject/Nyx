
use nyx_mix::{cover::poisson_rate, cover_adaptive::apply_utilization, MixConfig};
use rand::thread_rng;

#[test]
fn adaptive_cover_utilization_feedback_non_decreasing_lambda() {
	let cfg = MixConfig { base_cover_lambda: 10.0, low_power_ratio: 0.5, ..Default::default() };
	let mut prev = f32::MIN;
	for i in 0..=20 { // 0.0..=1.0 step
		let u = i as f32 / 20.0;
		let cur = apply_utilization(&cfg, u, false);
		assert!(cur >= prev, "u={} prev={} cur={}", u, prev, cur);
		prev = cur;
	}
}

#[test]
fn low_power_reduces_base_rate() {
	let cfg = MixConfig { base_cover_lambda: 12.0, low_power_ratio: 0.3, ..Default::default() };
	let u = 0.4;
	let hi = apply_utilization(&cfg, u, false);
	let lo = apply_utilization(&cfg, u, true);
	assert!(lo < hi);
	// exact expectation at u=0: lo == base*ratio
	let lo0 = apply_utilization(&cfg, 0.0, true);
	assert!((lo0 - 12.0 * 0.3).abs() < 1e-6);
}

#[test]
fn utilization_is_clamped() {
	let cfg = MixConfig { base_cover_lambda: 5.0, low_power_ratio: 0.5, ..Default::default() };
	let below = apply_utilization(&cfg, -1.0, false);
	let within = apply_utilization(&cfg, 0.0, false);
	let above = apply_utilization(&cfg, 10.0, false);
	assert!((below - within).abs() < 1e-6);
	assert!(above >= within);
}

#[test]
fn config_validation_ranges() {
	// ok
	MixConfig::default().validate_ranges().unwrap();
	// invalid low_power_ratio
	let bad = MixConfig { low_power_ratio: 1.2, ..Default::default() };
	assert!(bad.validate_ranges().is_err());
	// invalid base_cover_lambda
	let bad2 = MixConfig { base_cover_lambda: 100_000.0, ..Default::default() };
	assert!(bad2.validate_ranges().is_err());
}

#[test]
fn poisson_rate_matches_lambda_on_average() {
	let mut rng = thread_rng();
	let lambda = 6.0;
	let trials = 2000; // keep test fast
	let mut sum: u64 = 0;
	for _ in 0..trials { sum += poisson_rate(lambda, &mut rng) as u64; }
	let avg = sum as f64 / trials as f64;
	// within reasonable tolerance for Poisson fluctuation
	assert!((avg - lambda as f64).abs() < 0.5, "avg={} lambda={}", avg, lambda);
}

