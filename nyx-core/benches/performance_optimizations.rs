use criterion::{black_box, criterion_group, criterion_main, Criterion};

// Performance benchmarks for ultra-optimized components
use nyx_core::performance::RateLimiter;

/// Comprehensive performance benchmarks for core optimizations
fn bench_core_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_optimizations");

    // Rate Limiter Performance Comparison
    group.bench_function("rate_limiter_standard", |b| {
        b.iter(|| {
            let mut rl = RateLimiter::new(1000.0, 1000.0);
            let mut allowed = 0;
            for _ in 0..10000 {
                if rl.allow() {
                    allowed += 1;
                }
            }
            black_box(allowed);
        })
    });

    group.bench_function("rate_limiter_optimized", |b| {
        b.iter(|| {
            let mut rl = RateLimiter::new(1000.0, 1000.0);
            let mut allowed = 0;
            for _ in 0..10000 {
                if rl.allow_optimized() {
                    allowed += 1;
                }
            }
            black_box(allowed);
        })
    });

    group.bench_function("rate_limiter_ultra_fast", |b| {
        b.iter(|| {
            let mut rl = RateLimiter::new(1000.0, 1000.0);
            let mut allowed = 0;
            for _ in 0..10000 {
                if rl.allow_ultra_fast() {
                    allowed += 1;
                }
            }
            black_box(allowed);
        })
    });

    group.finish();
}

/// Memory allocation benchmarks
fn bench_memory_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_optimizations");

    // Vector allocation vs pre-allocation
    group.bench_function("vec_dynamic_allocation", |b| {
        b.iter(|| {
            let mut data = Vec::new();
            for i in 0..1000 {
                data.push(i);
            }
            black_box(data);
        })
    });

    group.bench_function("vec_pre_allocation", |b| {
        b.iter(|| {
            let mut data = Vec::with_capacity(1000);
            for i in 0..1000 {
                data.push(i);
            }
            black_box(data);
        })
    });

    // Buffer reuse vs new allocation
    group.bench_function("buffer_new_each_time", |b| {
        b.iter(|| {
            let mut results = Vec::new();
            for i in 0..100 {
                let mut buffer = Vec::new();
                buffer.extend_from_slice(&[i; 100]);
                results.push(buffer);
            }
            black_box(results);
        })
    });

    group.bench_function("buffer_reuse", |b| {
        b.iter(|| {
            let mut buffer = Vec::with_capacity(100);
            let mut results = Vec::new();
            for i in 0..100 {
                buffer.clear();
                buffer.extend_from_slice(&[i; 100]);
                results.push(buffer.clone());
            }
            black_box(results);
        })
    });

    // Advanced SIMD-friendly operations
    group.bench_function("array_processing_scalar", |b| {
        let data: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        b.iter(|| {
            let mut sum = 0.0;
            for &value in &data {
                sum += value * value;
            }
            black_box(sum);
        })
    });

    group.bench_function("array_processing_vectorized", |b| {
        let data: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        b.iter(|| {
            // Optimized loop that's more friendly to auto-vectorization
            let mut sum = 0.0;
            let chunks = data.chunks_exact(4);
            let remainder = chunks.remainder();
            
            for chunk in chunks {
                sum += chunk[0] * chunk[0];
                sum += chunk[1] * chunk[1];
                sum += chunk[2] * chunk[2];
                sum += chunk[3] * chunk[3];
            }
            
            for &value in remainder {
                sum += value * value;
            }
            
            black_box(sum);
        })
    });

    group.finish();
}

/// Cache-friendly data structure benchmarks
fn bench_cache_optimizations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_optimizations");

    // Array of structures vs structure of arrays
    #[derive(Copy, Clone)]
    struct Point { x: f64, y: f64, z: f64 }
    
    group.bench_function("aos_processing", |b| {
        let points: Vec<Point> = (0..1000)
            .map(|i| Point { x: i as f64, y: (i * 2) as f64, z: (i * 3) as f64 })
            .collect();
        
        b.iter(|| {
            let mut sum = 0.0;
            for point in &points {
                sum += point.x + point.y + point.z;
            }
            black_box(sum);
        })
    });

    group.bench_function("soa_processing", |b| {
        let xs: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let ys: Vec<f64> = (0..1000).map(|i| (i * 2) as f64).collect();
        let zs: Vec<f64> = (0..1000).map(|i| (i * 3) as f64).collect();
        
        b.iter(|| {
            let mut sum = 0.0;
            for i in 0..1000 {
                sum += xs[i] + ys[i] + zs[i];
            }
            black_box(sum);
        })
    });

    // Cache-aligned vs non-aligned structures
    group.bench_function("cache_aligned_access", |b| {
        #[repr(align(64))]
        struct AlignedData {
            values: [u64; 8],
        }
        
        let data: Vec<AlignedData> = (0..100)
            .map(|i| AlignedData { values: [i; 8] })
            .collect();
        
        b.iter(|| {
            let mut sum = 0u64;
            for item in &data {
                for &value in &item.values {
                    sum = sum.wrapping_add(value);
                }
            }
            black_box(sum);
        })
    });

    group.finish();
}

criterion_group!(benches, bench_core_optimizations, bench_memory_optimizations, bench_cache_optimizations);
criterion_main!(benches);
