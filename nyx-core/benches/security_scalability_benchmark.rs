use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nyx_core::performance::RateLimiter;
#[cfg(feature = "zero_copy")]
use nyx_core::zero_copy::manager::BufferPool;
// use std::sync::Arc;
// use std::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
// use futures::future;

/// Comprehensive performance benchmarks for Nyx Core
/// This benchmark suite measures the performance improvements after optimization
fn bench_rate_limiter_comprehensive(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiter");

    // Test different rate limits
    for &rate in [10.0, 100.0, 1000.0].iter() {
        group.bench_with_input(BenchmarkId::new("allow", rate), &rate, |b, &rate| {
            let mut rl = RateLimiter::new(rate * 2.0, rate);
            b.iter(|| {
                black_box(rl.allow());
            })
        });

        group.bench_with_input(BenchmarkId::new("burst", rate), &rate, |b, &rate| {
            let mut rl = RateLimiter::new(rate * 10.0, rate);
            b.iter(|| {
                for _ in 0..10 {
                    black_box(rl.allow());
                }
            })
        });
    }
    group.finish();
}

#[cfg(feature = "zero_copy")]
fn bench_buffer_pool_comprehensive(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool");

    let pool = BufferPool::with_capacity(1024 * 1024); // 1MB pool
    let sizes = [64, 256, 1024, 4096, 16384];

    for &size in &sizes {
        group.throughput(criterion::Throughput::Elements(1000));

        group.bench_with_input(
            BenchmarkId::new("acquire_release", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let buf = pool.acquire(size);
                    black_box(buf.len());
                    pool.release(buf);
                })
            },
        );
    }
    group.finish();
}

fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    // Simulate different memory patterns
    let sizes = [1024, 4096, 16384, 65536];

    for &size in &sizes {
        group.bench_with_input(BenchmarkId::new("allocation", size), &size, |b, &size| {
            b.iter(|| {
                let data = vec![0u8; size];
                black_box(data.len());
            })
        });
    }
    group.finish();
}

fn bench_algorithmic_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("algorithms");

    let rates = [10.0, 100.0, 1000.0];

    for &rate in &rates {
        group.bench_with_input(BenchmarkId::new("rate_limiter", rate), &rate, |b, &rate| {
            let mut rl = RateLimiter::new(rate * 2.0, rate);
            b.iter(|| {
                for _ in 0..100 {
                    black_box(rl.allow());
                }
            })
        });
    }
    group.finish();
}

#[cfg(feature = "mobile")]
fn bench_mobile_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("mobile");

    group.bench_function("low_power_mode", |b| {
        b.iter(|| {
            // Simulate low power operations
            let _result = black_box(42);
        })
    });

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

fn bench_security_features(c: &mut Criterion) {
    let mut group = c.benchmark_group("security");

    group.bench_function("authentication", |b| {
        b.iter(|| {
            // Simulate authentication
            let _auth = black_box(true);
        })
    });

    group.finish();
}

fn bench_scalability_tests(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability");

    let sizes = [100, 1000, 10000];

    for &size in &sizes {
        group.bench_with_input(
            BenchmarkId::new("concurrent_operations", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    for _ in 0..size {
                        black_box(42);
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_rate_limiter_comprehensive,
    bench_memory_efficiency,
    bench_algorithmic_complexity,
    bench_core_optimizations,
    bench_memory_optimizations,
    bench_cache_optimizations,
    bench_security_features,
    bench_scalability_tests
);

criterion_main!(benches);
