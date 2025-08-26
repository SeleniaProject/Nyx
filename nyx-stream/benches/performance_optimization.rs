use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nyx_stream::performance::{StreamMetrics, BufferPool, PerfTimer};
use bytes::BytesMut;
use std::time::Duration;

/// Benchmark buffer pool performance with different allocation patterns
fn bench_buffer_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool");
    
    // Test different buffer sizes
    let sizes = [64, 1024, 8192, 32768];
    
    for size in sizes {
        group.bench_with_input(
            BenchmarkId::new("pool_allocation", size),
            &size,
            |b, &size| {
                let metrics = Box::leak(Box::new(StreamMetrics::new()));
                let mut pool = BufferPool::new(100, metrics);
                
                b.iter(|| {
                    let buf = pool.get_buffer(black_box(size));
                    pool.return_buffer(buf);
                });
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("direct_allocation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let _buf = BytesMut::with_capacity(black_box(size));
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark metrics collection overhead
fn bench_metrics_collection(c: &mut Criterion) {
    let metrics = Box::leak(Box::new(StreamMetrics::new()));
    
    c.bench_function("metrics_send_record", |b| {
        b.iter(|| {
            metrics.record_send(black_box(1024), black_box(Duration::from_micros(100)));
        });
    });
    
    c.bench_function("metrics_recv_record", |b| {
        b.iter(|| {
            metrics.record_recv(black_box(1024), black_box(Duration::from_micros(100)));
        });
    });
    
    c.bench_function("metrics_buffer_operations", |b| {
        b.iter(|| {
            metrics.record_buffer_allocation(black_box(1024));
            metrics.record_buffer_pool_hit();
            metrics.record_buffer_pool_miss();
        });
    });
    
    c.bench_function("metrics_throughput_stats", |b| {
        b.iter(|| {
            let _stats = metrics.get_throughput_stats();
        });
    });
}

/// Benchmark performance timer overhead
fn bench_perf_timer(c: &mut Criterion) {
    c.bench_function("perf_timer_overhead", |b| {
        b.iter(|| {
            let timer = PerfTimer::start();
            let _elapsed = timer.elapsed();
        });
    });
    
    c.bench_function("perf_timer_measurement", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();
            
            for _ in 0..iters {
                let timer = PerfTimer::start();
                // Simulate some work
                std::hint::black_box(42 * 2);
                let _elapsed = timer.elapsed();
            }
            
            start.elapsed()
        });
    });
}

/// Benchmark buffer pool under sequential load
fn bench_buffer_pool_sequential(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool_sequential");
    
    let allocation_counts = [10, 100, 1000, 10000];
    
    for count in allocation_counts {
        group.bench_with_input(
            BenchmarkId::new("sequential_allocations", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let metrics = Box::leak(Box::new(StreamMetrics::new()));
                    let mut pool = BufferPool::new(count / 2, metrics);
                    
                    let mut buffers = Vec::new();
                    
                    // Allocate phase
                    for _ in 0..count {
                        buffers.push(pool.get_buffer(1024));
                    }
                    
                    // Return phase
                    for buf in buffers {
                        pool.return_buffer(buf);
                    }
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark memory allocation patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");
    
    group.bench_function("repeated_small_allocations", |b| {
        let metrics = Box::leak(Box::new(StreamMetrics::new()));
        let mut pool = BufferPool::new(1000, metrics);
        
        b.iter(|| {
            for _ in 0..100 {
                let buf = pool.get_buffer(black_box(64));
                pool.return_buffer(buf);
            }
        });
    });
    
    group.bench_function("mixed_size_allocations", |b| {
        let metrics = Box::leak(Box::new(StreamMetrics::new()));
        let mut pool = BufferPool::new(1000, metrics);
        let sizes = [64, 512, 1024, 4096, 8192];
        
        b.iter(|| {
            for &size in &sizes {
                let buf = pool.get_buffer(black_box(size));
                pool.return_buffer(buf);
            }
        });
    });
    
    group.bench_function("large_allocation_burst", |b| {
        let metrics = Box::leak(Box::new(StreamMetrics::new()));
        let mut pool = BufferPool::new(100, metrics);
        
        b.iter(|| {
            let mut buffers = Vec::new();
            
            // Allocate burst
            for _ in 0..50 {
                buffers.push(pool.get_buffer(black_box(32768)));
            }
            
            // Return burst
            for buf in buffers {
                pool.return_buffer(buf);
            }
        });
    });
    
    group.finish();
}

/// Benchmark throughput calculation performance
fn bench_throughput_calculation(c: &mut Criterion) {
    let metrics = Box::leak(Box::new(StreamMetrics::new()));
    
    // Populate metrics with realistic data
    for i in 0..10000 {
        metrics.record_send(1024, Duration::from_micros(100 + i % 50));
        metrics.record_recv(1024, Duration::from_micros(95 + i % 45));
        metrics.record_buffer_pool_hit();
        if i % 10 == 0 {
            metrics.record_buffer_pool_miss();
        }
    }
    
    c.bench_function("throughput_stats_calculation", |b| {
        b.iter(|| {
            let _stats = metrics.get_throughput_stats();
        });
    });
}

criterion_group!(
    benches,
    bench_buffer_pool,
    bench_metrics_collection,
    bench_perf_timer,
    bench_buffer_pool_sequential,
    bench_memory_patterns,
    bench_throughput_calculation
);
criterion_main!(benches);
