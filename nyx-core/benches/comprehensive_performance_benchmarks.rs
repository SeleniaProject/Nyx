use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use nyx_core::performance::RateLimiter;
use std::sync::Arc;
use std::thread;
#[cfg(feature = "zero_copy")]
use nyx_core::zero_copy::manager::BufferPool;
#[cfg(feature = "zero_copy")]
use nyx_core::zero_copy::manager::Buffer;

/// Comprehensive performance benchmarks for Nyx Core
/// This benchmark suite measures the performance improvements after optimization
fn bench_rate_limiter_comprehensive(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiter");

    // Test different rate limits
    for &rate in [10.0, 100.0, 1000.0].iter() {
        group.bench_with_input(BenchmarkId::new("allow", rate), &rate, |b, &rate| {
            let mut rl = RateLimiter::new(rate * 2.0, rate);
            b.iter(|| {
                black_box(rl.allow_optimized());
            })
        });

        group.bench_with_input(BenchmarkId::new("burst", rate), &rate, |b, &rate| {
            let mut rl = RateLimiter::new(rate * 10.0, rate);
            b.iter(|| {
                for _ in 0..10 {
                    black_box(rl.allow_optimized());
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

        group.bench_with_input(BenchmarkId::new("acquire_release", size), &size, |b, &size| {
            b.iter(|| {
                let buf = pool.acquire(size);
                black_box(buf.len());
                pool.release(buf);
            })
        });

        group.bench_with_input(BenchmarkId::new("buffer_operations", size), &size, |b, &size| {
            let mut buf = pool.acquire(size);
            b.iter(|| {
                buf.clear();
                buf.extend_from_slice(&vec![0u8; size / 2]);
                black_box(buf.len());
            });
            pool.release(buf);
        });
    }
    group.finish();
}

#[cfg(not(feature = "zero_copy"))]
#[allow(dead_code)]
fn bench_buffer_pool_comprehensive(_c: &mut Criterion) {
    // Skip buffer pool benchmarks when feature is not available
}

#[cfg(feature = "zero_copy")]
fn bench_zero_copy_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("zero_copy");

    let test_data = vec![0u8; 8192];

    group.bench_function("buffer_creation", |b| {
        b.iter(|| {
            let buf: Buffer = test_data.clone().into();
            black_box(buf);
        })
    });

    let buf: Buffer = test_data.into();

    group.bench_function("buffer_clone", |b| {
        b.iter(|| {
            let cloned = buf.clone();
            black_box(cloned);
        })
    });

    group.bench_function("buffer_access", |b| {
        b.iter(|| {
            let slice = buf.as_slice();
            black_box(slice.len());
        })
    });

    group.bench_function("buffer_iteration", |b| {
        b.iter(|| {
            for &byte in buf.as_slice() {
                black_box(byte);
            }
        })
    });

    group.finish();
}

#[cfg(not(feature = "zero_copy"))]
#[allow(dead_code)]
fn bench_zero_copy_operations(_c: &mut Criterion) {
    // Skip zero-copy benchmarks when feature is not available
}

#[cfg(feature = "zero_copy")]
fn bench_concurrent_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent");

    let pool = Arc::new(BufferPool::with_capacity(1024 * 1024));
    let num_threads = num_cpus::get();

    group.bench_function("concurrent_buffer_ops", |b| {
        b.iter(|| {
            let mut handles = vec![];

            for _ in 0..num_threads {
                let pool_clone = Arc::clone(&pool);
                let handle = thread::spawn(move || {
                    for i in 0..100 {
                        let size = (i % 10 + 1) * 128;
                        let mut buf = pool_clone.acquire(size);
                        buf.extend_from_slice(&vec![0u8; size]);
                        pool_clone.release(buf);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    group.finish();
}

#[cfg(not(feature = "zero_copy"))]
#[allow(dead_code)]
fn bench_concurrent_operations(_c: &mut Criterion) {
    // Skip concurrent benchmarks when feature is not available
}

fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    group.bench_function("vec_allocation", |b| {
        b.iter(|| {
            let mut vec = Vec::with_capacity(1024);
            for i in 0..1024 {
                vec.push(i as u8);
            }
            black_box(vec);
        })
    });

    #[cfg(feature = "zero_copy")]
    let pool = BufferPool::with_capacity(1024 * 1024);

    #[cfg(feature = "zero_copy")]
    group.bench_function("pooled_allocation", |b| {
        b.iter(|| {
            let mut buf = pool.acquire(1024);
            for i in 0..1024 {
                buf.push(i as u8);
            }
            pool.release(buf);
        })
    });

    #[cfg(feature = "zero_copy")]
    group.bench_function("buffer_creation_overhead", |b| {
        b.iter(|| {
            let buf: Buffer = vec![0u8; 1024].into();
            black_box(buf);
        })
    });

    group.finish();
}

fn bench_algorithmic_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("algorithms");

    // Test rate limiter under different loads
    let rates = [1.0, 10.0, 100.0, 1000.0];

    for &rate in &rates {
        group.bench_with_input(BenchmarkId::new("rate_limiter_load", rate), &rate, |b, &rate| {
            let mut rl = RateLimiter::new(rate * 100.0, rate);
            let mut allowed = 0;

            b.iter(|| {
                // Simulate burst traffic
                for _ in 0..100 {
                    if rl.allow_optimized() {
                        allowed += 1;
                    }
                }
                black_box(allowed);
            })
        });
    }

    group.finish();
}

// Define benchmark groups based on available features
#[cfg(feature = "zero_copy")]
criterion_group!(
    benches,
    bench_rate_limiter_comprehensive,
    bench_buffer_pool_comprehensive,
    bench_zero_copy_operations,
    bench_concurrent_operations,
    bench_memory_efficiency,
    bench_algorithmic_complexity
);

#[cfg(not(feature = "zero_copy"))]
criterion_group!(
    benches,
    bench_rate_limiter_comprehensive,
    bench_memory_efficiency,
    bench_algorithmic_complexity
);

criterion_main!(benches);
