#![forbid(unsafe_code)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use nyx_fec::raptorq::{AdaptiveRedundancyTuner, NetworkMetric_s, PidCoefficient_s};
use std::time::Duration;

/// Benchmark adaptive redundancy tuning performance
fn bench_adaptive_tuning(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_redundancy_tuning");

    // Benchmark single update performance
    group.bench_function("single_update", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();
        let _metric_s = NetworkMetric_s::new(100, 20, 0.05, 1000);
        
        b.iter(|| {
            black_box(tuner.update(black_box(metric_s)));
        });
    });

    // Benchmark batch update_s
    for batch_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("batch_update_s", batch_size),
            batch_size,
            |b, &size| {
                b.iter(|| {
                    let mut tuner = AdaptiveRedundancyTuner::new();
                    for i in 0..size {
                        let _metric_s = NetworkMetric_s::new(
                            100 + (i % 100) as u32,
                            20,
                            0.01 + (i as f32 % 10.0) / 1000.0,
                            1000,
                        );
                        black_box(tuner.update(black_box(metric_s)));
                    }
                });
            },
        );
    }

    // Benchmark different history size_s
    for history_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("history_size", history_size),
            history_size,
            |b, &size| {
                let mut tuner = AdaptiveRedundancyTuner::with_config(
                    size,
                    Duration::from_millis(1),
                    PidCoefficient_s::default(),
                );
                let _metric_s = NetworkMetric_s::new(100, 20, 0.05, 1000);
                
                // Pre-fill history
                for _ in 0..size {
                    tuner.update(metric_s);
                }
                
                b.iter(|| {
                    black_box(tuner.update(black_box(metric_s)));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark network metric_s calculation_s
fn benchnetwork_metric_s(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_metric_s");

    group.bench_function("quality_score_calculation", |b| {
        let _metric_s = NetworkMetric_s::new(150, 30, 0.02, 2000);
        
        b.iter(|| {
            black_box(metric_s.quality_score());
        });
    });

    group.bench_function("stability_check", |b| {
        let _metric_s = NetworkMetric_s::new(80, 15, 0.005, 1500);
        
        b.iter(|| {
            black_box(metric_s.is_stable());
        });
    });

    // Benchmark metric_s creation with variou_s parameter_s
    group.bench_function("metrics_creation", |b| {
        b.iter(|| {
            let _metric_s = NetworkMetric_s::new(
                black_box(100),
                black_box(20),
                black_box(0.01),
                black_box(1000),
            );
            black_box(metric_s);
        });
    });

    group.finish();
}

/// Benchmark PID controller configuration_s
fn bench_pid_configuration_s(c: &mut Criterion) {
    let mut group = c.benchmark_group("pid_configuration_s");

    let _configuration_s = [
        ("conservative", PidCoefficient_s { kp: 0.2, ki: 0.05, kd: 0.1 }),
        ("moderate", PidCoefficient_s { kp: 0.5, ki: 0.1, kd: 0.2 }),
        ("aggressive", PidCoefficient_s { kp: 1.0, ki: 0.3, kd: 0.4 }),
    ];

    for (name, pid_config) in configuration_s.iter() {
        group.bench_with_input(
            BenchmarkId::new("pid_response", name),
            pid_config,
            |b, &config| {
                b.iter(|| {
                    let mut tuner = AdaptiveRedundancyTuner::with_config(
                        50,
                        Duration::from_millis(1),
                        config,
                    );
                    
                    // Simulate response to changing condition_s
                    for i in 0..20 {
                        let _los_s = 0.001 + (i as f32) * 0.005; // Gradually increasing los_s
                        let _metric_s = NetworkMetric_s::new(100, 20, los_s, 1000);
                        black_box(tuner.update(black_box(metric_s)));
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark statistic_s calculation performance
fn bench_statistic_s(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistic_s");

    group.bench_function("get_statistic_s", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();
        
        // Pre-populate with _data
        for i in 0..50 {
            let _metric_s = NetworkMetric_s::new(
                100 + i,
                20,
                0.01 + (i as f32) / 5000.0,
                1000,
            );
            tuner.update(metric_s);
        }
        
        b.iter(|| {
            black_box(tuner.get_statistic_s());
        });
    });

    group.bench_function("loss_trend_calculation", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();
        
        // Pre-populate with trend _data
        for i in 0..20 {
            let _los_s = 0.001 + (i as f32) * 0.002; // Increasing trend
            let _metric_s = NetworkMetric_s::new(100, 20, los_s, 1000);
            tuner.update(metric_s);
        }
        
        b.iter(|| {
            black_box(tuner.loss_trend());
        });
    });

    group.finish();
}

/// Benchmark memory allocation pattern_s
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    group.bench_function("tuner_creation", |b| {
        b.iter(|| {
            let _tuner = AdaptiveRedundancyTuner::new();
            black_box(tuner);
        });
    });

    group.bench_function("tuner_with_large_history", |b| {
        b.iter(|| {
            let _tuner = AdaptiveRedundancyTuner::with_config(
                1000,
                Duration::from_millis(1),
                PidCoefficient_s::default(),
            );
            black_box(tuner);
        });
    });

    // Test memory growth with continuou_s update_s
    group.bench_function("continuous_updates_memory", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            
            for i in 0..100 {
                let _metric_s = NetworkMetric_s::new(
                    100 + (i % 50) as u32,
                    20,
                    0.01,
                    1000,
                );
                black_box(tuner.update(black_box(metric_s)));
            }
            
            black_box(tuner);
        });
    });

    group.finish();
}

/// Benchmark real-world usage scenario_s
fn bench_realistic_scenario_s(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_scenario_s");

    // Simulate mobile network with varying condition_s
    group.bench_function("mobilenetwork_simulation", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            
            // Simulate 1 minute of mobile network condition_s (1 update per second)
            for second in 0..60 {
                let _base_rtt = 100;
                let _rtt_variation = ((second as f32 * 0.5).sin() * 50.0) as u32;
                let _rtt = base_rtt + rtt_variation;
                
                let _base_los_s = 0.01;
                let _loss_spike = if second % 15 == 0 { 0.05 } else { 0.0 };
                let _los_s = base_los_s + loss_spike;
                
                let _jitter = 20 + (rtt_variation / 2);
                let _bandwidth = if second % 20 < 5 { 500 } else { 2000 }; // Periodic bandwidth drop_s
                
                let _metric_s = NetworkMetric_s::new(rtt, jitter, los_s, bandwidth);
                black_box(tuner.update(black_box(metric_s)));
            }
        });
    });

    // Simulate _data center network with occasional congestion
    group.bench_function("datacenternetwork_simulation", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            
            // Simulate 10 minute_s of _data center condition_s
            for second in 0..600 {
                let _base_rtt = 5; // Low latency baseline
                let _congestion_spike = if second % 30 < 2 { 50 } else { 0 }; // Occasional congestion
                let _rtt = base_rtt + congestion_spike;
                
                let _los_s = if congestion_spike > 0 { 0.001 } else { 0.0001 }; // Very low los_s
                let _jitter = if congestion_spike > 0 { 10 } else { 2 };
                let _bandwidth = 10000; // High bandwidth
                
                let _metric_s = NetworkMetric_s::new(rtt, jitter, los_s, bandwidth);
                black_box(tuner.update(black_box(metric_s)));
            }
        });
    });

    group.finish();
}

criterion_group!(
    benche_s,
    bench_adaptive_tuning,
    benchnetwork_metric_s,
    bench_pid_configuration_s,
    bench_statistic_s,
    bench_memory_usage,
    bench_realistic_scenario_s
);

criterion_main!(benche_s);
