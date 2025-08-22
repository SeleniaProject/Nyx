#![cfg(feature = "raptorq")]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nyx_fec::raptorq::{AdaptiveRedundancyTuner, NetworkMetrics, PidCoefficients};
use std::time::Duration;

fn adaptive_redundancy_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_redundancy");
    group.measurement_time(Duration::from_secs(10));

    // Benchmark single update performance
    group.bench_function("single_update", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();
        let metrics = NetworkMetrics::new(100, 20, 0.05, 1000);

        b.iter(|| {
            black_box(tuner.update(black_box(metrics)));
        });
    });

    // Benchmark batch updates
    for batch_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_updates", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut tuner = AdaptiveRedundancyTuner::new();
                    for i in 0..size {
                        let metrics = NetworkMetrics::new(
                            100 + (i % 100) as u32,
                            20,
                            0.01 + (i as f32 / 1000.0),
                            1000,
                        );
                        black_box(tuner.update(black_box(metrics)));
                    }
                });
            },
        );
    }

    // Benchmark configuration updates
    group.bench_function("config_updates", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();
        let metrics = NetworkMetrics::new(100, 20, 0.05, 1000);

        b.iter(|| {
            for _ in 0..10 {
                tuner.update(metrics);
                black_box(tuner.update(black_box(metrics)));
            }
        });
    });

    // Benchmark statistic calculation
    group.bench_function("statistics_calculation", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();

        // Pre-fill with some data
        for i in 0..100 {
            let metrics = NetworkMetrics::new(100, 20, 0.05, 1000);
            tuner.update(metrics);
        }

        b.iter(|| {
            black_box(tuner.get_statistics());
        });
    });

    group.finish();
}

fn network_metrics_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_metrics");

    // Benchmark metric creation
    group.bench_function("create_metrics", |b| {
        b.iter(|| {
            let metrics = NetworkMetrics::new(150, 30, 0.02, 2000);
            black_box(metrics.quality_score());
        });
    });

    group.bench_function("stability_check", |b| {
        let metrics = NetworkMetrics::new(80, 15, 0.005, 1500);
        b.iter(|| {
            black_box(metrics.is_stable());
        });
    });

    // Benchmark metric operations
    group.bench_function("metric_operations", |b| {
        b.iter(|| {
            let metrics = NetworkMetrics::new(
                black_box(100),
                black_box(20),
                black_box(0.01),
                black_box(1000),
            );
            black_box(metrics);
        });
    });

    group.finish();
}

fn pid_controller_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("pid_controller");

    // Benchmark different PID configurations
    let configurations = [
        (
            "conservative",
            PidCoefficients {
                kp: 0.1,
                ki: 0.01,
                kd: 0.05,
            },
        ),
        (
            "balanced",
            PidCoefficients {
                kp: 0.3,
                ki: 0.05,
                kd: 0.1,
            },
        ),
        (
            "aggressive",
            PidCoefficients {
                kp: 0.8,
                ki: 0.2,
                kd: 0.3,
            },
        ),
        (
            "precision",
            PidCoefficients {
                kp: 0.15,
                ki: 0.02,
                kd: 0.08,
            },
        ),
        (
            "responsive",
            PidCoefficients {
                kp: 0.6,
                ki: 0.15,
                kd: 0.25,
            },
        ),
    ];

    for (name, pid_config) in configurations.iter() {
        group.bench_with_input(
            BenchmarkId::new("pid_adaptation", name),
            pid_config,
            |b, config| {
                b.iter(|| {
                    let mut tuner = AdaptiveRedundancyTuner::with_config(
                        50, // max_history
                        Duration::from_millis(1), // min_adjustment_interval 
                        *config // pid_coefficients
                    );
                    for i in 0..50 {
                        let loss = 0.001 + (i as f32) * 0.005; // Gradually increasing loss
                        let metrics = NetworkMetrics::new(100, 20, loss, 1000);
                        black_box(tuner.update(black_box(metrics)));
                    }
                });
            },
        );
    }

    group.finish();
}

fn stability_convergence_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("stability_convergence");

    // Benchmark convergence under various conditions
    group.bench_function("convergence_stable", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            for i in 0..200 {
                let metrics = NetworkMetrics::new(100 + i, 20, 0.01 + (i as f32) / 5000.0, 1000);
                tuner.update(metrics);
            }
        });
    });

    group.bench_function("convergence_unstable", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            for i in 0..200 {
                let loss = 0.001 + (i as f32) * 0.002; // Increasing trend
                let metrics = NetworkMetrics::new(100, 20, loss, 1000);
                tuner.update(metrics);
            }
        });
    });

    group.finish();
}

fn memory_allocation_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    // Benchmark tuner creation
    group.bench_function("create_tuner", |b| {
        b.iter(|| {
            let tuner = AdaptiveRedundancyTuner::new();
            black_box(tuner);
        });
    });

    group.bench_function("create_tuner_with_config", |b| {
        b.iter(|| {
            let tuner = AdaptiveRedundancyTuner::with_config(
                50, // max_history
                Duration::from_millis(1), // min_adjustment_interval
                PidCoefficients {
                    kp: 0.5,
                    ki: 0.1,
                    kd: 0.2,
                }
            );
            black_box(tuner);
        });
    });

    // Benchmark high-frequency updates
    group.bench_function("high_frequency_updates", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();
        b.iter(|| {
            for i in 0..1000 {
                let metrics = NetworkMetrics::new(100 + (i % 50) as u32, 20, 0.01, 1000);
                black_box(tuner.update(black_box(metrics)));
            }
        });
    });

    group.finish();
}

fn real_world_scenarios_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_scenarios");

    // Simulate varying network conditions
    group.bench_function("varying_conditions", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            for second in 0..120 {
                let base_rtt = 100;
                let rtt_variation = ((second as f32 * 0.5).sin() * 50.0) as u32;
                let rtt = base_rtt + rtt_variation;

                let base_loss = 0.01;
                let loss_spike = if second % 15 == 0 { 0.05 } else { 0.0 };
                let loss = base_loss + loss_spike;

                let jitter = 20 + (rtt_variation / 2);
                let bandwidth = if second % 20 < 5 { 500 } else { 2000 }; // Periodic bandwidth drops

                let metrics = NetworkMetrics::new(rtt, jitter, loss, bandwidth);
                black_box(tuner.update(black_box(metrics)));
            }
        });
    });

    // Low latency scenario
    group.bench_function("low_latency_scenario", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            for second in 0..60 {
                let base_rtt = 5; // Low latency baseline
                let congestion_spike = if second % 30 < 2 { 50 } else { 0 }; // Occasional congestion
                let rtt = base_rtt + congestion_spike;

                let loss = if congestion_spike > 0 { 0.001 } else { 0.0001 }; // Very low loss
                let jitter = if congestion_spike > 0 { 10 } else { 2 };
                let bandwidth = 10000; // High bandwidth

                let metrics = NetworkMetrics::new(rtt, jitter, loss, bandwidth);
                black_box(tuner.update(black_box(metrics)));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    adaptive_redundancy_benchmarks,
    network_metrics_benchmarks,
    pid_controller_benchmarks,
    stability_convergence_benchmarks,
    memory_allocation_benchmarks,
    real_world_scenarios_benchmarks
);
criterion_main!(benches);
