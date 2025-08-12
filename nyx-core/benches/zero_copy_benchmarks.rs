#![forbid(unsafe_code)]

//! Performance benchmarks for zero-copy optimization system.
//!
//! This module provides comprehensive benchmarking of the zero-copy optimization
//! system to measure performance improvements and validate optimization effectiveness.

use nyx_core::zero_copy::{
    ZeroCopyManager, ZeroCopyManagerConfig, CriticalPath, CriticalPathConfig,
    Stage, AllocationTracker, BufferPool,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Benchmark allocation tracking overhead
fn bench_allocation_tracking(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("allocation_tracking", |b| {
        let tracker = Arc::new(AllocationTracker::new(10000));
        
        b.iter(|| {
            rt.block_on(async {
                let event = nyx_core::zero_copy::AllocationEvent {
                    stage: black_box(Stage::Crypto),
                    operation: black_box(nyx_core::zero_copy::OperationType::Allocate),
                    size: black_box(1024),
                    timestamp: std::time::Instant::now(),
                    context: Some("benchmark".to_string()),
                };
                tracker.record_allocation(event).await;
            });
        });
    });
}

/// Benchmark buffer pool performance
fn bench_buffer_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool");
    
    for size in [256, 1280, 4096, 16384].iter() {
        group.bench_with_input(BenchmarkId::new("get_buffer", size), size, |b, &size| {
            let mut pool = BufferPool::new(100, 1000);
            
            b.iter(|| {
                let buffer = pool.get_buffer(black_box(size));
                pool.return_buffer(buffer);
            });
        });
    }
    
    group.finish();
}

/// Benchmark zero-copy vs traditional processing
fn bench_zero_copy_vs_traditional(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("processing_comparison");
    
    // Traditional processing (with copies)
    group.bench_function("traditional_crypto_processing", |b| {
        b.iter(|| {
            let input = black_box(vec![0u8; 4096]);
            let mut output = Vec::with_capacity(input.len() + 16);
            output.extend_from_slice(&input); // Copy
            output.extend_from_slice(&[0u8; 16]); // Mock AEAD tag
            black_box(output);
        });
    });
    
    // Zero-copy processing
    group.bench_function("zero_copy_crypto_processing", |b| {
        let config = CriticalPathConfig::default();
        let path = CriticalPath::new("benchmark".to_string(), config);
        
        b.iter(|| {
            rt.block_on(async {
                let input = black_box(vec![0u8; 4096]);
                let context_id = "bench_context";
                let result = path.process_crypto_stage(context_id, &input).await;
                black_box(result);
            });
        });
    });
    
    group.finish();
}

/// Benchmark complete pipeline processing
fn bench_complete_pipeline(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("complete_pipeline");
    
    for packet_size in [1024, 4096, 8192, 16384].iter() {
        group.bench_with_input(BenchmarkId::new("process_packet", packet_size), packet_size, |b, &size| {
            let config = CriticalPathConfig {
                enable_zero_copy: true,
                enable_buffer_pooling: true,
                ..Default::default()
            };
            let path = CriticalPath::new("pipeline_bench".to_string(), config);
            
            b.iter(|| {
                rt.block_on(async {
                    let packet = black_box(vec![0u8; size]);
                    let result = path.process_packet(&packet).await;
                    black_box(result);
                });
            });
        });
    }
    
    group.finish();
}

/// Benchmark concurrent processing
fn bench_concurrent_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("concurrent_processing");
    
    for concurrency in [1, 4, 8, 16].iter() {
        group.bench_with_input(BenchmarkId::new("concurrent_packets", concurrency), concurrency, |b, &conc| {
            let config = CriticalPathConfig::default();
            
            b.iter(|| {
                rt.block_on(async {
                    let mut tasks = Vec::new();
                    
                    for i in 0..conc {
                        let path = CriticalPath::new(format!("concurrent_{}", i), config.clone());
                        let packet = vec![0u8; 4096];
                        
                        let task = async move {
                            path.process_packet(&packet).await
                        };
                        tasks.push(task);
                    }
                    
                    let results = futures::future::join_all(tasks).await;
                    black_box(results);
                });
            });
        });
    }
    
    group.finish();
}

/// Benchmark memory allocation patterns
fn bench_memory_allocation_patterns(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("memory_allocation_patterns", |b| {
        let config = ZeroCopyManagerConfig::default();
        let manager = Arc::new(ZeroCopyManager::new(config));
        
        b.iter(|| {
            rt.block_on(async {
                // Simulate typical allocation pattern
                let path = manager.create_critical_path("mem_bench".to_string()).await.unwrap();
                
                // Process several packets of different sizes
                for size in [512, 1280, 2048, 4096] {
                    let packet = vec![0u8; size];
                    let _result = path.process_packet(&packet).await;
                }
                
                manager.remove_critical_path("mem_bench").await.unwrap();
            });
        });
    });
}

/// Benchmark metrics collection overhead
fn bench_metrics_collection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("metrics_collection", |b| {
        let config = CriticalPathConfig::default();
        let path = CriticalPath::new("metrics_bench".to_string(), config);
        
        b.iter(|| {
            rt.block_on(async {
                // Generate some activity
                let packet = vec![0u8; 1024];
                let _result = path.process_packet(&packet).await;
                
                // Collect metrics
                let metrics = path.get_metrics().await;
                black_box(metrics);
            });
        });
    });
}

/// Benchmark buffer pool efficiency under different load patterns
fn bench_buffer_pool_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_pool_efficiency");
    
    // Test different pool sizes and access patterns
    for pool_size in [10, 100, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("pool_size", pool_size), pool_size, |b, &size| {
            b.iter(|| {
                let mut pool = BufferPool::new(size / 10, size);
                
                // Simulate realistic access pattern
                let mut buffers = Vec::new();
                
                // Acquire buffers
                for _ in 0..50 {
                    buffers.push(pool.get_buffer(1280));
                }
                
                // Return some buffers
                for _ in 0..25 {
                    if let Some(buffer) = buffers.pop() {
                        pool.return_buffer(buffer);
                    }
                }
                
                // Acquire more buffers (should reuse)
                for _ in 0..25 {
                    buffers.push(pool.get_buffer(1280));
                }
                
                // Clean up
                for buffer in buffers {
                    pool.return_buffer(buffer);
                }
                
                let stats = pool.stats();
                black_box(stats);
            });
        });
    }
    
    group.finish();
}

/// Benchmark zero-copy optimization effectiveness
fn bench_optimization_effectiveness(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("optimization_effectiveness");
    
    // Compare zero-copy enabled vs disabled
    for zero_copy_enabled in [false, true].iter() {
        let label = if *zero_copy_enabled { "enabled" } else { "disabled" };
        
        group.bench_with_input(BenchmarkId::new("zero_copy", label), zero_copy_enabled, |b, &enabled| {
            let config = CriticalPathConfig {
                enable_zero_copy: enabled,
                enable_buffer_pooling: enabled,
                ..Default::default()
            };
            let path = CriticalPath::new(format!("opt_{}", enabled), config);
            
            b.iter(|| {
                rt.block_on(async {
                    let packet = black_box(vec![0u8; 8192]);
                    let result = path.process_packet(&packet).await;
                    black_box(result);
                });
            });
        });
    }
    
    group.finish();
}

/// Benchmark stage-specific processing
fn bench_stage_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("stage_processing");
    
    let config = CriticalPathConfig::default();
    let path = CriticalPath::new("stage_bench".to_string(), config);
    let test_data = vec![0u8; 4096];
    
    group.bench_function("crypto_stage", |b| {
        b.iter(|| {
            rt.block_on(async {
                let context_id = "crypto_bench";
                let _context = path.start_processing(context_id.to_string()).await.unwrap();
                let result = path.process_crypto_stage(context_id, &test_data).await;
                path.finish_processing(context_id).await.unwrap();
                black_box(result);
            });
        });
    });
    
    group.bench_function("fec_stage", |b| {
        b.iter(|| {
            rt.block_on(async {
                let context_id = "fec_bench";
                let _context = path.start_processing(context_id.to_string()).await.unwrap();
                let crypto_output = path.process_crypto_stage(context_id, &test_data).await.unwrap();
                let result = path.process_fec_stage(context_id, &crypto_output).await;
                path.finish_processing(context_id).await.unwrap();
                black_box(result);
            });
        });
    });
    
    group.finish();
}

criterion_group!(
    zero_copy_benchmarks,
    bench_allocation_tracking,
    bench_buffer_pool,
    bench_zero_copy_vs_traditional,
    bench_complete_pipeline,
    bench_concurrent_processing,
    bench_memory_allocation_patterns,
    bench_metrics_collection,
    bench_buffer_pool_efficiency,
    bench_optimization_effectiveness,
    bench_stage_processing
);

criterion_main!(zero_copy_benchmarks);
