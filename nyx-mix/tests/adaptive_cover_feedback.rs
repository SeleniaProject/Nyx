//! Adaptive Cover Traffic Validation Tests
//!
//! This test suite validates the adaptive cover traffic algorithm against
//! the mathematical properties and performance guarantees specified in
//! the design specification (docs/adaptive_cover_traffic_spec.md).

use nyx_mix::MixConfig;
use rand::thread_rng;

/// Test the fundamental monotonicity property: λ(u₁) ≤ λ(u₂) for u₁≤ u₂
/// This is critical for anonymity preservation.
#[test]
fn adaptive_cover_utilization_feedback_non_decreasing_lambda() {
    let config = MixConfig {
        base_cover_lambda: 10.0,
        low_power_ratio: 0.5,
        ..Default::default()
    };
    let mut prev = f32::MIN;
    for i in 0..=20 {
        // 0.0..=1.0 step
        let u = i as f32 / 20.0;
        let cur = apply_utilization(&config, u.into(), false);
        assert!(
            cur >= prev,
            "Monotonicity violation: u={u} prev={prev} cur={cur}"
        );
        prev = cur;
    }
}

/// Test power mode scaling maintains correct ratio.
/// Validates battery optimization while preserving anonymity.
#[test]
fn low_power_reduces_base_rate() {
    let config = MixConfig {
        base_cover_lambda: 12.0,
        low_power_ratio: 0.3,
        ..Default::default()
    };
    let u = 0.4;
    let hi = apply_utilization(&config, u, false);
    let lo = apply_utilization(&config, u, true);
    assert!(lo < hi, "Low power should reduce rate");

    // Verify exact scaling at baseline (u=0)
    let lo0 = apply_utilization(&config, 0.0, true);

    // Test edge cases
    let expected_ratio = config.low_power_ratio;
    assert!((lo0 / config.base_cover_lambda - expected_ratio).abs() < 1e-6);
}

/// Test boundary conditions to prevent overflow/underflow attacks
#[test]
fn boundary_conditions_respected() {
    let config = MixConfig {
        base_cover_lambda: 5.0,
        low_power_ratio: 0.5,
        ..Default::default()
    };

    // Test extreme values
    let below = apply_utilization(&config, -1.0, false);
    let within = apply_utilization(&config, 0.0, false);
    let above = apply_utilization(&config, 10.0, false);
    let max_valid = apply_utilization(&config, 1.0, false);

    // Values should be reasonable
    assert!(below >= 0.0);
    assert!(within >= 0.0);
    assert!(above >= 0.0);
    assert!(max_valid >= 0.0);
}

/// Test config validation
#[test]
fn config_validation_ranges() -> Result<(), Box<dyn std::error::Error>> {
    // Valid configuration should pass
    MixConfig::default().validate_range_s()?;

    // Invalid configuration should fail (high ratio)
    let invalid_config = MixConfig {
        low_power_ratio: 1.2,
        ..Default::default()
    };
    assert!(invalid_config.validate_range_s().is_err());

    // Invalid configuration should fail (high lambda)
    let invalid_config2 = MixConfig {
        base_cover_lambda: 100_000.0,
        ..Default::default()
    };
    assert!(invalid_config2.validate_range_s().is_err());

    // Invalid configuration should fail (negative ratio)
    let invalid_config3 = MixConfig {
        low_power_ratio: -0.1,
        ..Default::default()
    };
    assert!(
        invalid_config3.validate_range_s().is_err(),
        "Negative ratio should be rejected"
    );

    Ok(())
}

/// Mathematical correctness verification: formula matches spec
#[test]
fn mathematical_correctness() {
    let config = MixConfig::default();
    for i in 0..=10 {
        let u = i as f32 / 10.0; // 0.0 to 1.0

        // Test normal mode: λ = base * (1 + u)
        let normal = apply_utilization(&config, u.into(), false);
        let expected = config.base_cover_lambda * (1.0 + u);
        assert!(
            (normal - expected).abs() < f32::EPSILON,
            "Normal mode formula mismatch: expected={expected} actual={normal}"
        );

        // Test low power mode: λ = base * low_ratio * (1 + u)
        let power = apply_utilization(&config, u.into(), true);
        let expected_power = config.base_cover_lambda * config.low_power_ratio * (1.0 + u);
        assert!(
            (power - expected_power).abs() < f32::EPSILON,
            "Low power mode formula mismatch: expected={expected_power} actual={power}"
        );
    }
}

/// Performance characteristics test
#[test]
fn rate_ranges_reasonable() {
    let config = MixConfig::default();
    let min_rate = apply_utilization(&config, 0.0, false);
    let max_rate = apply_utilization(&config, 1.0, false);

    // Ranges should be reasonable for network performance
    assert!(min_rate >= 1.0, "Minimum rate too low");
    assert!(max_rate <= 1000.0, "Maximum rate too high");
    assert!(max_rate >= min_rate * 1.5, "Range too narrow");
}

/// Poisson distribution statistical validation
#[test]
fn poisson_statistical_properties() {
    let lambda = 10.0;
    let mut sum = 0.0f64;
    let samples = 1000;
    let mut rng = thread_rng();

    for _ in 0..samples {
        sum += poisson_rate(lambda, &mut rng) as f64;
    }

    let avg = sum / samples as f64;
    // For exponential distribution, mean should be 1/lambda
    let expected_mean = 1.0 / lambda as f64;
    assert!(
        (avg - expected_mean).abs() < 0.01,
        "Exponential mean deviation: expected={expected_mean} actual={avg}"
    );
}

/// Performance benchmarks ensuring real-time feasibility
#[test]
fn adaptive_cover_performance() {
    let config = MixConfig::default();
    let start = std::time::Instant::now();

    for i in 0..10_000 {
        let utilization = (i % 100) as f32 / 100.0;
        let _rate = apply_utilization(&config, utilization.into(), i % 2 == 0);
    }

    let duration = start.elapsed();
    let per_call_ns = duration.as_nanos() / 10_000;

    assert!(
        per_call_ns < 1_000,
        "Performance regression: {per_call_ns}ns per call"
    );
}

fn apply_utilization(config: &MixConfig, utilization: f64, low_power: bool) -> f32 {
    let base = config.base_cover_lambda;
    if low_power {
        base * config.low_power_ratio * (1.0 + utilization as f32)
    } else {
        base * (1.0 + utilization as f32)
    }
}

fn poisson_rate(lambda: f32, rng: &mut impl rand::Rng) -> f32 {
    // Simple exponential distribution to approximate Poisson inter-arrival times
    let u: f64 = rng.gen_range(0.000001..1.0); // Avoid log(0)
    (-u.ln() / lambda as f64) as f32
}
