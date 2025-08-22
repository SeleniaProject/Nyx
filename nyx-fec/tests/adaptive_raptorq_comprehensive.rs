#![forbid(unsafe_code)]

#[cfg(feature = "raptorq")]
use nyx_fec::raptorq::{AdaptiveRedundancyTuner, NetworkMetrics, PidCoefficients};
use std::time::{Duration, Instant};

/// Comprehensive integration tests for adaptive RaptorQ redundancy tuning
#[cfg(all(test, feature = "raptorq"))]
mod integration_tests {
    use super::*;

    #[test]
    fn scenario_stable_low_latency_network() {
        let mut tuner = AdaptiveRedundancyTuner::new();

        // Simulate 10 updates with stable, good network conditions
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(100));
            let metrics = NetworkMetrics::new(30, 5, 0.001, 2000); // 30ms RTT, 5ms jitter, 0.1% loss, 2Mbps
            let redundancy = tuner.update(metrics);

            // Should converge to low redundancy for stable good network
            assert!(
                redundancy.tx < 0.3,
                "TX redundancy too high for stable network: {}",
                redundancy.tx
            );
            assert!(
                redundancy.rx < 0.3,
                "RX redundancy too high for stable network: {}",
                redundancy.rx
            );
        }

        let stats = tuner.get_statistics();
        assert!(
            stats.quality_score > 0.8,
            "Quality score should be high for stable network"
        );
        assert!(stats.loss_trend.abs() < 0.1, "Loss trend should be stable");
    }

    #[test]
    fn scenario_degrading_network_conditions() {
        let mut tuner = AdaptiveRedundancyTuner::new();

        // Start with good conditions
        for _i in 0..5 {
            std::thread::sleep(Duration::from_millis(100));
            let metrics = NetworkMetrics::new(50, 10, 0.001, 1500);
            tuner.update(metrics);
        }
        let initial_redundancy = tuner.current_redundancy();

        // Gradually degrade network conditions
        for i in 0..10 {
            std::thread::sleep(Duration::from_millis(100));
            let rtt = 50 + i * 20; // Increasing RTT
            let jitter = 10 + i * 5; // Increasing jitter
            let loss_rate = 0.001 + i as f32 * 0.005; // Increasing loss rate
            let bandwidth = 1500 - i * 100; // Decreasing bandwidth

            let metrics = NetworkMetrics::new(rtt, jitter, loss_rate, bandwidth);
            tuner.update(metrics);
        }

        let final_redundancy = tuner.current_redundancy();

        // Redundancy should increase as network degrades
        assert!(
            final_redundancy.tx > initial_redundancy.tx,
            "TX redundancy should increase with degrading network"
        );
        assert!(
            final_redundancy.rx > initial_redundancy.rx,
            "RX redundancy should increase with degrading network"
        );

        let stats = tuner.get_statistics();
        assert!(
            stats.quality_score < 0.5,
            "Quality score should be low for degraded network"
        );
    }

    #[test]
    fn scenario_high_loss_burst_recovery() {
        let mut tuner = AdaptiveRedundancyTuner::new();

        // Establish baseline with normal conditions
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(50));
            let metrics = NetworkMetrics::new(40, 8, 0.005, 1200);
            tuner.update(metrics);
        }

        // Sudden burst of high packet loss
        for _ in 0..5 {
            std::thread::sleep(Duration::from_millis(50));
            let metrics = NetworkMetrics::new(40, 8, 0.15, 1200); // 15% loss
            tuner.update(metrics);
        }

        let burst_redundancy = tuner.current_redundancy();
        assert!(
            burst_redundancy.tx > 0.8,
            "TX redundancy should spike during loss burst: {}",
            burst_redundancy.tx
        );

        // Recovery to normal conditions
        for _ in 0..15 {
            std::thread::sleep(Duration::from_millis(50));
            let metrics = NetworkMetrics::new(40, 8, 0.005, 1200);
            tuner.update(metrics);
        }

        let recovery_redundancy = tuner.current_redundancy();
        assert!(
            recovery_redundancy.tx < burst_redundancy.tx,
            "TX redundancy should decrease during recovery"
        );
    }

    #[test]
    fn scenario_oscillating_network_conditions() {
        let mut tuner = AdaptiveRedundancyTuner::new();
        let mut redundancy_values = Vec::new();

        // Oscillate between good and bad conditions
        for _cycle in 0..5 {
            // Good phase
            for _ in 0..5 {
                std::thread::sleep(Duration::from_millis(50));
                let metrics = NetworkMetrics::new(30, 5, 0.001, 2000);
                let redundancy = tuner.update(metrics);
                redundancy_values.push(redundancy.tx);
            }

            // Bad phase
            for _ in 0..5 {
                std::thread::sleep(Duration::from_millis(50));
                let metrics = NetworkMetrics::new(150, 30, 0.08, 500);
                let redundancy = tuner.update(metrics);
                redundancy_values.push(redundancy.tx);
            }
        }

        let stats = tuner.get_statistics();

        // System should adapt but not overreact (removed unavailable fields)
        assert!(
            stats.adjustment_count > 10,
            "Should track multiple adjustments for oscillating conditions"
        );
        assert!(
            stats.quality_score < 0.8,
            "Quality score should reflect challenging conditions"
        );
    }

    #[test]
    fn scenario_bandwidth_constrained_environment() {
        let mut tuner = AdaptiveRedundancyTuner::new();

        // Very low bandwidth scenario
        for _ in 0..20 {
            std::thread::sleep(Duration::from_millis(75));
            let metrics = NetworkMetrics::new(200, 50, 0.02, 256); // Very low bandwidth
            tuner.update(metrics);
        }

        let redundancy = tuner.current_redundancy();
        let stats = tuner.get_statistics();

        // Should balance redundancy with bandwidth constraints
        assert!(
            redundancy.tx < 0.6,
            "TX redundancy should be constrained by low bandwidth: {}",
            redundancy.tx
        );
        assert!(
            stats.quality_score < 0.7,
            "Quality score should reflect bandwidth constraints"
        );
    }

    #[test]
    fn scenario_custom_pid_coefficients() {
        // Test with aggressive PID coefficients
        let aggressive_coeffs = PidCoefficients {
            kp: 1.5,
            ki: 0.3,
            kd: 0.8,
        };
        let mut aggressive_tuner =
            AdaptiveRedundancyTuner::with_config(50, Duration::from_millis(1), aggressive_coeffs);

        // Test with conservative PID coefficients
        let conservative_coeffs = PidCoefficients {
            kp: 0.3,
            ki: 0.05,
            kd: 0.1,
        };
        let mut conservative_tuner =
            AdaptiveRedundancyTuner::with_config(50, Duration::from_millis(1), conservative_coeffs);

        // Apply same network conditions to both
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(50));
            let metrics = NetworkMetrics::new(100, 20, 0.05, 800);

            aggressive_tuner.update(metrics.clone());
            conservative_tuner.update(metrics);
        }

        let aggressive_redundancy = aggressive_tuner.current_redundancy();
        let conservative_redundancy = conservative_tuner.current_redundancy();

        // Aggressive tuner should react more strongly
        assert!(
            aggressive_redundancy.tx > conservative_redundancy.tx,
            "Aggressive tuner should have higher redundancy"
        );

        let aggressive_stats = aggressive_tuner.get_statistics();
        let conservative_stats = conservative_tuner.get_statistics();

        assert!(
            aggressive_stats.adjustment_count == conservative_stats.adjustment_count,
            "Both tuners should have same number of adjustments"
        );
    }

    #[test]
    fn scenario_long_duration_stability() {
        let mut tuner = AdaptiveRedundancyTuner::new();
        let mut redundancy_history = Vec::new();

        // Run for extended period with stable conditions
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(20));
            let metrics = NetworkMetrics::new(60, 12, 0.01, 1000);
            let redundancy = tuner.update(metrics);
            redundancy_history.push(redundancy.tx);
        }

        // Check for convergence and stability
        let last_20: Vec<f32> = redundancy_history.iter().rev().take(20).cloned().collect();
        let variance = calculate_variance(&last_20);

        assert!(
            variance < 0.01,
            "Redundancy should stabilize over long duration: variance = {}",
            variance
        );

        let stats = tuner.get_statistics();
        assert!(
            stats.adjustment_count == 100,
            "Should track all adjustments correctly"
        );
    }
}

// Helper function for variance calculation
fn calculate_variance(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }

    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f32>()
        / values.len() as f32;

    variance
}

#[test]
fn comprehensive_performance_benchmark() {
    let mut tuner = AdaptiveRedundancyTuner::new();
    let start_time = Instant::now();

    // Stress test with rapid updates
    for i in 0..1000 {
        let rtt = 50 + (i % 100) as u32;
        let jitter = 5 + (i % 20) as u32;
        let loss_rate = 0.001 + (i as f32 % 50.0) / 10000.0;
        let bandwidth = 1000 + (i % 500) as u32;

        let metrics = NetworkMetrics::new(rtt, jitter, loss_rate, bandwidth);
        tuner.update(metrics);
    }

    let duration_elapsed = start_time.elapsed();

    // Should complete 1000 updates quickly
    assert!(
        duration_elapsed < Duration::from_millis(100),
        "Performance test took too long: {:?}",
        duration_elapsed
    );

    let stats = tuner.get_statistics();
    assert!(
        stats.adjustment_count <= 1000,
        "Should track updates correctly (limited by adjustment interval)"
    );
}
