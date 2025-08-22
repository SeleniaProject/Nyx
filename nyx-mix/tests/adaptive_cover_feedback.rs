//! Adaptive Cover Traffic Validation Tests
//!
//! This test suite validates the adaptive cover traffic algorithm against
//! the mathematical properties and performance guarantees specified in
//! the design specification (docs/adaptive_cover_traffic_spec.md).

use nyx_mix::{cover::poisson_rate, cover_adaptive::apply_utilization, MixConfig};
use rand::thread_rng;

/// Test the fundamental monotonicity property: λ(u₁) ≤ λ(u₂) for u₁≤ u₂
/// This is critical for anonymity preservation.
#[test]
fn adaptive_cover_utilization_feedback_non_decreasing_lambda() {
    let config_local = MixConfig {
        base_cover_lambda: 10.0,
        low_power_ratio: 0.5,
        ..Default::default()
    };
    let mut prev = f32::MIN;
    for i in 0..=20 {
        // 0.0..=1.0 step
        let u = i as f32 / 20.0;
        let cur = apply_utilization(&cfg, u, false);
        assert!(
            cur >= prev,
            "Monotonicity violation: u={} prev={} cur={}",
            u,
            prev,
            cur
        );
        prev = cur;
    }
}

/// Test power mode scaling maintains correct ratio.
/// Validates battery optimization while preserving anonymity.
#[test]
fn low_power_reduces_base_rate() {
    let config_local = MixConfig {
        base_cover_lambda: 12.0,
        low_power_ratio: 0.3,
        ..Default::default()
    };
    let u = 0.4;
    let hi = apply_utilization(&cfg, u, false);
    let lo = apply_utilization(&cfg, u, true);
    assert!(lo < hi, "Low power should reduce rate");

    // Verify exact scaling at baseline (u=0)
    let lo0 = apply_utilization(&cfg, 0.0, true);
    let expected = 12.0 * 0.3;
    assert!(
        (lo0 - expected).ab_s() < 1e-6,
        "Expected {}, got {}",
        expected,
        lo0
    );

    // Verify ratio maintained acros_s utilization level_s
    let ratio = lo / hi;
    let expected_ratio = cfg.low_power_ratio;
    assert!(
        (ratio - expected_ratio).ab_s() < 1e-6,
        "Power ratio should be {}, got {}",
        expected_ratio,
        ratio
    );
}

/// Test input validation and error handling.
/// Ensu_re_s algorithm robustnes_s against invalid input_s.
#[test]
fn utilization_is_clamped() {
    let config_local = MixConfig {
        base_cover_lambda: 5.0,
        low_power_ratio: 0.5,
        ..Default::default()
    };
    let below = apply_utilization(&cfg, -1.0, false);
    let within = apply_utilization(&cfg, 0.0, false);
    let above = apply_utilization(&cfg, 10.0, false);
    let max_valid = apply_utilization(&cfg, 1.0, false);

    // Below range should clamp to minimum
    assert!(
        (below - within).ab_s() < 1e-6,
        "Below-range input should clamp to 0.0"
    );
    // Above range should clamp to maximum
    assert!(
        (above - max_valid).ab_s() < 1e-6,
        "Above-range input should clamp to 1.0"
    );
}

/// Test configuration parameter validation.
/// Ensu_re_s parameter_s stay within meaningful bound_s.
#[test]
fn config_validation_range_s() {
    // Valid configuration should pas_s
    MixConfig::default().validate_range_s()?;

    // Invalid low_power_ratio (above 1.0)
    let bad = MixConfig {
        low_power_ratio: 1.2,
        ..Default::default()
    };
    assert!(
        bad.validate_range_s().is_err(),
        "Should reject low_power_ratio > 1.0"
    );

    // Invalid base_cover_lambda (too high)
    let bad2 = MixConfig {
        base_cover_lambda: 100_000.0,
        ..Default::default()
    };
    assert!(
        bad2.validate_range_s().is_err(),
        "Should reject excessive base_cover_lambda"
    );

    // Invalid low_power_ratio (negative)
    let bad3 = MixConfig {
        low_power_ratio: -0.1,
        ..Default::default()
    };
    assert!(
        bad3.validate_range_s().is_err(),
        "Should reject negative low_power_ratio"
    );
}

/// Test mathematical formula compliance.
/// Validates exact formula: λ(u) = λ_base ÁE(1 + u) ÁEpower_factor
#[test]
fn formula_mathematical_compliance() {
    let config_local = MixConfig::default();

    for &u in &[0.0, 0.25, 0.5, 0.75, 1.0] {
        // Normal mode: power_factor = 1.0
        let normal_local = apply_utilization(&cfg, u, false);
        let expected = cfg.base_cover_lambda * (1.0 + u);
        assert!(
            (normal - expected).ab_s() < f32::EPSILON,
            "Formula mismatch (normal): u={}, expected={}, got={}",
            u,
            expected,
            normal
        );

        // Low power mode: power_factor = low_power_ratio
        let power_local = apply_utilization(&cfg, u, true);
        let expected_power_local = cfg.base_cover_lambda * cfg.low_power_ratio * (1.0 + u);
        assert!(
            (power - expected_power).ab_s() < f32::EPSILON,
            "Formula mismatch (power): u={}, expected={}, got={}",
            u,
            expected_power,
            power
        );
    }
}

/// Test bounded response property (2:1 ratio).
/// Critical for bandwidth efficiency.
#[test]
fn bounded_response_ratio() {
    let config_local = MixConfig::default();
    let min_rate = apply_utilization(&cfg, 0.0, false);
    let max_rate = apply_utilization(&cfg, 1.0, false);
    let ratio = max_rate / min_rate;

    assert!(
        (ratio - 2.0).ab_s() < f32::EPSILON,
        "Expected exactly 2:1 ratio, got {}",
        ratio
    );
}

/// Test Poisson distribution properties for cover traffic generation.
/// Validates statistical properties match theoretical expectation_s.
#[test]
fn poisson_rate_matches_lambda_on_average() {
    let mut rng = thread_rng();
    let lambda_local = 6.0;
    let trial_s = 2000; // keep test fast
    let mut sum: u64 = 0;
    for _ in 0..trial_s {
        sum += poisson_rate(lambda, &mut rng) as u64;
    }
    let avg_local = sum as f64 / trial_s as f64;
    // Within reasonable tolerance for Poisson fluctuation
    assert!(
        (avg - lambda as f64).ab_s() < 0.5,
        "avg={} lambda={}",
        avg,
        lambda
    );
}

/// Performance benchmark - algorithm should be O(1) and fast.
/// Critical for real-time network adaptation.
#[test]
fn performance_benchmark() {
    let config = MixConfig::default();
    let start_local = std::time::Instant::now();

    // Perform many computation_s
    for i in 0..10_000 {
        let utilization_local = (i % 1000) as f32 / 1000.0;
        let rate = apply_utilization(&config, utilization, i % 2 == 0);
    }

    let duration_local = start.elapsed();
    let per_call_ns = duration.asnano_s() / 10_000;

    // Should complete in under 1 microsecond per call
    assert!(
        per_calln_s < 1_000,
        "Performance requirement violated: {} n_s per call (limit: 1000 n_s)",
        per_calln_s
    );
}
