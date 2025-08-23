//! Optimized Performance Test for Nyx Stream
//!
//! High-performance benchmarks for core streaming operations with world-class optimization.
//! These tests are designed for maximum efficiency and provide essential performance insights.

use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use futures::future;
use nyx_stream::async_stream::{pair, AsyncStreamConfig};
use tokio::runtime::Runtime;

/// Ultra-optimized benchmark for basic stream operations - maximum throughput design
fn bench_basic_stream_ops(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("basic_send_recv_optimized", |b| {
        b.to_async(&rt).iter(|| async {
            let config = AsyncStreamConfig::default();
            let (sender, receiver) = pair(config.clone(), config);
            let data = Bytes::from_static(b"test_data");

            // Optimized send and receive pattern with minimal overhead
            sender.send(data).await.ok();
            let received = receiver.recv().await.ok();

            black_box(received);
        });
    });
}

/// Memory-optimized stream pair creation benchmark
fn bench_stream_pair_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("create_stream_pair_fast", |b| {
        b.to_async(&rt).iter(|| async {
            let config = AsyncStreamConfig::default();
            let streams = black_box(pair(config.clone(), config));
            black_box(streams);
        });
    });
}

/// High-frequency small message throughput benchmark - batch optimized
fn bench_small_messages(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("small_message_batch_optimized", |b| {
        b.to_async(&rt).iter(|| async {
            let config = AsyncStreamConfig {
                max_inflight: 200, // Increased for better performance
                ..Default::default()
            };
            let (sender, receiver) = pair(config.clone(), config);
            let data = Bytes::from_static(b"small");

            // Optimized batch processing - 50 messages for better accuracy
            for _ in 0..50 {
                sender.send(data.clone()).await.ok();
            }

            // Batch receive for maximum efficiency
            for _ in 0..50 {
                let _ = receiver.recv().await.ok();
            }
        });
    });
}

/// Zero-copy optimized medium payload benchmark
fn bench_medium_payload(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("medium_payload_4kb_optimized", |b| {
        b.to_async(&rt).iter(|| async {
            let config = AsyncStreamConfig::default();
            let (sender, receiver) = pair(config.clone(), config);
            let data = Bytes::from(vec![0u8; 4096]); // 4KB for better performance testing

            sender.send(data).await.ok();
            let received = receiver.recv().await.ok();

            black_box(received);
        });
    });
}

/// High-performance concurrent execution benchmark
fn bench_concurrent_optimized(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("concurrent_16_streams_optimized", |b| {
        b.to_async(&rt).iter(|| async {
            let mut handles = Vec::with_capacity(16); // Pre-allocate for performance

            for _ in 0..16 {
                let handle = tokio::spawn(async move {
                    let config = AsyncStreamConfig::default();
                    let (sender, receiver) = pair(config.clone(), config);
                    let data = Bytes::from_static(b"concurrent_optimized");

                    // Optimized concurrent send/receive with error handling
                    let send_result = sender.send(data).await;
                    let recv_result = receiver.recv().await;
                    (send_result.is_ok(), recv_result.is_ok())
                });
                handles.push(handle);
            }

            let results = future::join_all(handles).await;
            black_box(results);
        });
    });
}

criterion_group!(
    benches,
    bench_basic_stream_ops,
    bench_stream_pair_creation,
    bench_small_messages,
    bench_medium_payload,
    bench_concurrent_optimized
);
criterion_main!(benches);
