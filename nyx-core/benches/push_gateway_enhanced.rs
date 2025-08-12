#![forbid(unsafe_code)]

//! Benchmark tests for enhanced Push Gateway performance validation
//! 
//! These benchmarks measure the performance characteristics of jittered backoff
//! calculations and histogram operations to ensure they meet production requirements.

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;
use std::time::Duration;

use nyx_core::push_gateway::{
    LatencyHistogram, JitteredBackoff, PushGatewayManager, MinimalReconnector, BoxFuture
};

/// Mock reconnector for benchmarking
struct BenchmarkReconnector;

impl MinimalReconnector for BenchmarkReconnector {
    fn reconnect_minimal(&self) -> BoxFuture<'_, Result<(), String>> {
        Box::pin(async move {
            // Simulate minimal processing delay
            tokio::time::sleep(Duration::from_micros(100)).await;
            Ok(())
        })
    }
}

/// Benchmark jittered backoff calculation performance
fn bench_jittered_backoff_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jittered_backoff_calculation");
    group.sample_size(10000);
    
    let backoff = JitteredBackoff::default();
    
    for attempt in [1, 3, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("calculate_delay", attempt),
            attempt,
            |b, &attempt| {
                b.iter(|| {
                    backoff.calculate_delay(attempt)
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark histogram sample recording performance
fn bench_histogram_sample_recording(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_sample_recording");
    group.sample_size(5000);
    
    for sample_count in [100, 1000, 10000].iter() {
        let samples: Vec<u64> = (0..*sample_count)
            .map(|i| (i % 10000) + 10) // Range 10-10009ms
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("record_samples", sample_count),
            &samples,
            |b, samples| {
                b.iter(|| {
                    let mut histogram = LatencyHistogram::default();
                    for &sample in samples {
                        histogram.record_sample(sample);
                    }
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark percentile calculation performance
fn bench_histogram_percentile_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_percentile_calculation");
    group.sample_size(2000);
    
    for sample_count in [1000, 5000, 10000].iter() {
        let mut histogram = LatencyHistogram::default();
        
        // Pre-populate histogram with samples
        for i in 0..*sample_count {
            let latency = (i % 10000) + 10; // Range 10-10009ms
            histogram.record_sample(latency);
        }
        
        group.bench_with_input(
            BenchmarkId::new("calculate_percentiles", sample_count),
            &histogram,
            |b, histogram| {
                b.iter(|| {
                    let _p50 = histogram.calculate_percentile(0.50);
                    let _p95 = histogram.calculate_percentile(0.95);
                    let _p99 = histogram.calculate_percentile(0.99);
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark bucket distribution generation
fn bench_histogram_bucket_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_bucket_distribution");
    group.sample_size(3000);
    
    for sample_count in [1000, 5000, 10000].iter() {
        let mut histogram = LatencyHistogram::default();
        
        // Pre-populate histogram
        for i in 0..*sample_count {
            let latency = (i % 10000) + 10;
            histogram.record_sample(latency);
        }
        
        group.bench_with_input(
            BenchmarkId::new("bucket_distribution", sample_count),
            &histogram,
            |b, histogram| {
                b.iter(|| {
                    histogram.bucket_distribution()
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark complete push gateway statistics collection
fn bench_push_gateway_stats_collection(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("push_gateway_stats_collection");
    group.sample_size(1000);
    
    // Create manager with some historical data
    let reconnector = Arc::new(BenchmarkReconnector);
    let mgr = PushGatewayManager::new(reconnector);
    
    // Generate some sample data
    rt.block_on(async {
        for _ in 0..100 {
            let _ = mgr.push_wake();
            let _ = mgr.resume_low_power_session().await;
        }
    });
    
    group.bench_function("collect_stats", |b| {
        b.iter(|| {
            mgr.stats()
        })
    });
    
    group.finish();
}

/// Memory usage benchmark for histogram operations
fn bench_histogram_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("histogram_memory_usage");
    group.sample_size(500);
    group.measurement_time(Duration::from_secs(30));
    
    for histogram_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("multiple_histograms", histogram_count),
            histogram_count,
            |b, &histogram_count| {
                b.iter(|| {
                    let histograms: Vec<LatencyHistogram> = (0..histogram_count)
                        .map(|_| {
                            let mut hist = LatencyHistogram::default();
                            // Add some samples to each histogram
                            for sample in [50, 150, 500, 1500, 5000].iter() {
                                hist.record_sample(*sample);
                            }
                            hist
                        })
                        .collect();
                    
                    // Perform operations on all histograms
                    let total_samples: u64 = histograms.iter()
                        .map(|h| h.total_samples)
                        .sum();
                    
                    total_samples
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark concurrent access simulation
fn bench_concurrent_push_gateway_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_push_gateway_operations");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(45));
    
    for concurrent_ops in [5, 10, 20].iter() {
        let reconnector = Arc::new(BenchmarkReconnector);
        let mgr = PushGatewayManager::new(reconnector);
        
        group.bench_with_input(
            BenchmarkId::new("concurrent_operations", concurrent_ops),
            concurrent_ops,
            |b, &concurrent_ops| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = Vec::new();
                        
                        for _ in 0..concurrent_ops {
                            let mgr_clone = mgr.clone();
                            handles.push(tokio::spawn(async move {
                                let _ = mgr_clone.push_wake();
                                let stats = mgr_clone.stats();
                                stats.total_wake_events
                            }));
                        }
                        
                        // Wait for all operations to complete
                        for handle in handles {
                            let _ = handle.await;
                        }
                    })
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_jittered_backoff_calculation,
    bench_histogram_sample_recording,
    bench_histogram_percentile_calculation,
    bench_histogram_bucket_distribution,
    bench_push_gateway_stats_collection,
    bench_histogram_memory_usage,
    bench_concurrent_push_gateway_operations
);

criterion_main!(benches);
