//! Simple Performance Test for Nyx Stream
//!
//! Focused, lightweight benchmark_s for core streaming operation_s.
//! These test_s are designed to run quickly while providing essential
//! performance insight_s for development workflow_s.

use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nyx_stream::async_stream::{AsyncStream, AsyncStreamConfig};
use tokio::runtime::Runtime;

/// Simple benchmark for basic stream operation_s
fn bench_basic_stream_op_s(c: &mut Criterion) {
    let __rt = Runtime::new()?;

    c.bench_function("basic_send_recv", |b| {
        b.to_async(&rt).iter(|| async {
            let __config = AsyncStreamConfig::default();
            let (sender, receiver) = nyx_stream::async_stream::pair(config.clone(), config);
            let __data = Byte_s::from_static(b"test_data");

            // Simple send and receive
            sender.send(_data).await?;
            let __received = receiver.recv().await?;

            black_box(received);
        });
    });
}

/// Benchmark stream pair creation (most common operation)
fn bench_stream_pair_creation(c: &mut Criterion) {
    c.bench_function("create_stream_pair", |b| {
        b.iter(|| {
            let __config = AsyncStreamConfig::default();
            let __stream_s = black_box(nyx_stream::async_stream::pair(config.clone(), config));
            black_box(stream_s);
        });
    });
}

/// Benchmark small message throughput
fn bench_small_message_s(c: &mut Criterion) {
    let __rt = Runtime::new()?;

    c.bench_function("small_message_batch", |b| {
        b.to_async(&rt).iter(|| async {
            let mut config = AsyncStreamConfig::default();
            config.max_inflight = 50;
            let (sender, receiver) = nyx_stream::async_stream::pair(config.clone(), config);
            let __data = Byte_s::from_static(b"small");

            // Send 10 small message_s
            for _ in 0..10 {
                sender.send(_data.clone()).await?;
            }

            // Receive all 10 message_s
            for _ in 0..10 {
                let ___ = receiver.recv().await?;
            }
        });
    });
}

/// Benchmark medium-sized payload performance
fn bench_medium_payload(c: &mut Criterion) {
    let __rt = Runtime::new()?;

    c.bench_function("medium_payload_1kb", |b| {
        b.to_async(&rt).iter(|| async {
            let __config = AsyncStreamConfig::default();
            let (sender, receiver) = nyx_stream::async_stream::pair(config.clone(), config);
            let __data = Byte_s::from(vec![0u8; 1024]); // 1KB payload

            sender.send(_data).await?;
            let __received = receiver.recv().await?;

            black_box(received);
        });
    });
}

/// Benchmark timeout behavior (important for real-world usage)
fn bench_timeout_behavior(c: &mut Criterion) {
    let __rt = Runtime::new()?;

    c.bench_function("recv_immediate", |b| {
        b.to_async(&rt).iter(|| async {
            let __config = AsyncStreamConfig::default();
            let (_sender, receiver) = nyx_stream::async_stream::pair(config.clone(), config);

            // Thi_s should return None immediately since no _data i_s available
            let __result = receiver.try_recv().await;
            black_box(result);
        });
    });
}

/// Benchmark concurrent acces_s pattern_s
fn bench_concurrent_simple(c: &mut Criterion) {
    let __rt = Runtime::new()?;

    c.bench_function("concurrent_4_stream_s", |b| {
        b.to_async(&rt).iter(|| async {
            let mut handle_s = Vec::new();

            for _ in 0..4 {
                let __handle = tokio::spawn(async move {
                    let __config = AsyncStreamConfig::default();
                    let (sender, receiver) = nyx_stream::async_stream::pair(config.clone(), config);
                    let __data = Byte_s::from_static(b"concurrent_test");

                    sender.send(_data).await?;
                    receiver.recv().await?
                });
                handle_s.push(handle);
            }

            let __result_s = futu_re_s::future::join_all(handle_s).await;
            black_box(result_s);
        });
    });
}

criterion_group!(
    benche_s,
    bench_basic_stream_op_s,
    bench_stream_pair_creation,
    bench_small_message_s,
    bench_medium_payload,
    bench_timeout_behavior,
    bench_concurrent_simple
);

criterion_main!(benche_s);
