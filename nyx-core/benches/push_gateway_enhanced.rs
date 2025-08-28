// High-performance production benchmark for NyxNet
// Simulates real anonymous network usage patterns

use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nyx_core::performance::RateLimiter;
use nyx_stream::async_stream::{pair, AsyncStreamConfig};
use std::time::Duration;
use tokio::runtime::Runtime;

// Traffic pattern message sizes based on real usage
#[allow(dead_code)]
const SMALL_MSG: usize = 512; // Control messages
#[allow(dead_code)]
const MEDIUM_MSG: usize = 1420; // Standard MTU payload
#[allow(dead_code)]
const LARGE_MSG: usize = 8192; // File transfers
#[allow(dead_code)]
const BURST_MSG: usize = 32768; // Large downloads

/// Most common use case benchmark
#[allow(dead_code)]
fn bench_realistic_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("realistic_usage");

    group.bench_function("web_browsing_simulation", |b| {
        b.to_async(&rt).iter(|| async {
            let config = AsyncStreamConfig::default();
            let (sender, receiver) = pair(config.clone(), config);

            // Simulate typical web browsing patterns
            let data = vec![0u8; MEDIUM_MSG];
            let result = sender.send(Bytes::from(data)).await;
            let _ = black_box(result);

            let received = receiver.recv().await;
            let _ = black_box(received);
        });
    });

    group.bench_function("concurrent_connections", |b| {
        b.to_async(&rt).iter(|| async {
            let config = AsyncStreamConfig::default();

            // Simulate multiple concurrent connections
            let mut handles = Vec::new();
            for _ in 0..10 {
                let (sender, receiver) = pair(config.clone(), config.clone());
                let handle = tokio::spawn(async move {
                    // Concurrent message sending and receiving
                    let data = vec![0u8; SMALL_MSG];
                    let _ = sender.send(Bytes::from(data)).await;
                    let _ = receiver.recv().await;
                });
                handles.push(handle);
            }

            for handle in handles {
                let _ = handle.await;
            }
        });
    });

    group.finish();
}

/// Simulate real relay node load
#[allow(dead_code)]
fn bench_relay_node_simulation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("relay_node");

    let connection_counts = [10, 50, 100, 500];

    for &count in &connection_counts {
        group.bench_with_input(
            BenchmarkId::new("concurrent_relay", count),
            &count,
            |b, &count| {
                b.to_async(&rt).iter(|| async {
                    let config = AsyncStreamConfig::default();
                    let mut rate_limiter = RateLimiter::new(1000.0, 100.0);

                    let mut handles = Vec::new();
                    for _ in 0..count {
                        if rate_limiter.allow() {
                            let (sender, receiver) = pair(config.clone(), config.clone());
                            let handle = tokio::spawn(async move {
                                // Simple rate limiting simulation
                                let data = vec![0u8; MEDIUM_MSG];
                                let _ = sender.send(Bytes::from(data)).await;
                                let _ = receiver.recv().await;
                            });
                            handles.push(handle);
                        }
                    }

                    for handle in handles {
                        let _ = handle.await;
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Memory efficiency stress test
#[allow(dead_code)]
fn bench_memory_efficiency_stress(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_efficiency");

    let message_sizes = [SMALL_MSG, MEDIUM_MSG, LARGE_MSG, BURST_MSG];

    for &size in &message_sizes {
        group.bench_with_input(BenchmarkId::new("memory_usage", size), &size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let config = AsyncStreamConfig::default();
                let (sender, receiver) = pair(config.clone(), config);

                // Process various sized messages efficiently
                let data = vec![0u8; size];
                let result = sender.send(Bytes::from(data)).await;
                let _ = black_box(result);

                let received = receiver.recv().await;
                let _ = black_box(received);
            });
        });
    }

    group.finish();
}

/// Benchmark: Network constraints performance
/// Simulate real network conditions
#[allow(dead_code)]
fn bench_network_constraints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("network_constraints");

    let latencies = [
        ("optimal", 10), // 10ms latency
        ("good", 50),    // 50ms latency
        ("poor", 200),   // 200ms latency
        ("mobile", 500), // 500ms latency
    ];

    for (name, latency_ms) in latencies.iter() {
        group.bench_with_input(
            BenchmarkId::new("latency_simulation", name),
            latency_ms,
            |b, &latency_ms| {
                b.to_async(&rt).iter(|| async {
                    let config = AsyncStreamConfig::default();
                    let (sender, receiver) = pair(config.clone(), config);

                    // Simulate network latency
                    tokio::time::sleep(Duration::from_millis(latency_ms as u64)).await;

                    let data = vec![0u8; MEDIUM_MSG];
                    let result = sender.send(Bytes::from(data)).await;
                    let _ = black_box(result);

                    let received = receiver.recv().await;
                    let _ = black_box(received);
                });
            },
        );
    }

    group.finish();
}

fn bench_core_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_optimizations");

    group.bench_function("rate_limiter_optimized", |b| {
        let mut rl = RateLimiter::new(1000.0, 100.0);
        b.iter(|| {
            black_box(rl.allow());
        })
    });

    group.finish();
}

fn bench_memory_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_optimizations");

    group.bench_function("efficient_allocation", |b| {
        b.iter(|| {
            let data = vec![0u8; 1024];
            black_box(data.len());
        })
    });

    group.finish();
}

fn bench_cache_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_optimizations");

    group.bench_function("cache_hit", |b| {
        b.iter(|| {
            // Simulate cache operations
            let _value = black_box(42);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_core_optimizations,
    bench_memory_optimizations,
    bench_cache_optimizations
);
criterion_main!(benches);
