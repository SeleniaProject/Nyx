#![forbid(unsafe_code)]

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use nyx_fec::raptorq::{AdaptiveRedundancyTuner, NetworkMetrics, PidCoefficients};
use std::time::Duration;

/// Benchmark adaptive redundancy tuning performance
fn bench_adaptive_tuning(c: &mut Criterion) {
    let mut group = c.benchmark_group("adaptive_redundancy_tuning");

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
                            0.01 + (i as f32 % 10.0) / 1000.0,
                            1000,
                        );
                        black_box(tuner.update(black_box(metrics)));
                    }
                });
            },
        );
    }

    // Benchmark different history sizes
    for history_size in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("history_size", history_size),
            history_size,
            |b, &size| {
                let mut tuner = AdaptiveRedundancyTuner::with_config(
                    size,
                    Duration::from_millis(1),
                    PidCoefficients::default(),
                );
                let metrics = NetworkMetrics::new(100, 20, 0.05, 1000);
                
                // Pre-fill history
                for _ in 0..size {
                    tuner.update(metrics);
                }
                
                b.iter(|| {
                    black_box(tuner.update(black_box(metrics)));
                });
            },
        );
    }

    group.finish();
}

/// Benchmark network metrics calculations
fn bench_network_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_metrics");

    group.bench_function("quality_score_calculation", |b| {
        let metrics = NetworkMetrics::new(150, 30, 0.02, 2000);
        
        b.iter(|| {
            black_box(metrics.quality_score());
        });
    });

    group.bench_function("stability_check", |b| {
        let metrics = NetworkMetrics::new(80, 15, 0.005, 1500);
        
        b.iter(|| {
            black_box(metrics.is_stable());
        });
    });

    // Benchmark metrics creation with various parameters
    group.bench_function("metrics_creation", |b| {
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

/// Benchmark PID controller configurations
fn bench_pid_configurations(c: &mut Criterion) {
    let mut group = c.benchmark_group("pid_configurations");

    let configurations = [
        ("conservative", PidCoefficients { kp: 0.2, ki: 0.05, kd: 0.1 }),
        ("moderate", PidCoefficients { kp: 0.5, ki: 0.1, kd: 0.2 }),
        ("aggressive", PidCoefficients { kp: 1.0, ki: 0.3, kd: 0.4 }),
    ];

    for (name, pid_config) in configurations.iter() {
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
                    
                    // Simulate response to changing conditions
                    for i in 0..20 {
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

/// Benchmark statistics calculation performance
fn bench_statistics(c: &mut Criterion) {
    let mut group = c.benchmark_group("statistics");

    group.bench_function("get_statistics", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();
        
        // Pre-populate with data
        for i in 0..50 {
            let metrics = NetworkMetrics::new(
                100 + i,
                20,
                0.01 + (i as f32) / 5000.0,
                1000,
            );
            tuner.update(metrics);
        }
        
        b.iter(|| {
            black_box(tuner.get_statistics());
        });
    });

    group.bench_function("loss_trend_calculation", |b| {
        let mut tuner = AdaptiveRedundancyTuner::new();
        
        // Pre-populate with trend data
        for i in 0..20 {
            let loss = 0.001 + (i as f32) * 0.002; // Increasing trend
            let metrics = NetworkMetrics::new(100, 20, loss, 1000);
            tuner.update(metrics);
        }
        
        b.iter(|| {
            black_box(tuner.loss_trend());
        });
    });

    group.finish();
}

/// Benchmark memory allocation patterns
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    group.bench_function("tuner_creation", |b| {
        b.iter(|| {
            let tuner = AdaptiveRedundancyTuner::new();
            black_box(tuner);
        });
    });

    group.bench_function("tuner_with_large_history", |b| {
        b.iter(|| {
            let tuner = AdaptiveRedundancyTuner::with_config(
                1000,
                Duration::from_millis(1),
                PidCoefficients::default(),
            );
            black_box(tuner);
        });
    });

    // Test memory growth with continuous updates
    group.bench_function("continuous_updates_memory", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            
            for i in 0..100 {
                let metrics = NetworkMetrics::new(
                    100 + (i % 50) as u32,
                    20,
                    0.01,
                    1000,
                );
                black_box(tuner.update(black_box(metrics)));
            }
            
            black_box(tuner);
        });
    });

    group.finish();
}

/// Benchmark real-world usage scenarios
fn bench_realistic_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_scenarios");

    // Simulate mobile network with varying conditions
    group.bench_function("mobile_network_simulation", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            
            // Simulate 1 minute of mobile network conditions (1 update per second)
            for second in 0..60 {
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

    // Simulate data center network with occasional congestion
    group.bench_function("datacenter_network_simulation", |b| {
        b.iter(|| {
            let mut tuner = AdaptiveRedundancyTuner::new();
            
            // Simulate 10 minutes of data center conditions
            for second in 0..600 {
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
    bench_adaptive_tuning,
    bench_network_metrics,
    bench_pid_configurations,
    bench_statistics,
    bench_memory_usage,
    bench_realistic_scenarios
);

criterion_main!(benches);
