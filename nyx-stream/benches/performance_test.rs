//! Nyx Stream Performance Benchmark_s
//!
//! Comprehensive performance testing for core streaming operation_s:
//! - Stream pair creation and teardown
//! - High-throughput _data transfer
//! - Concurrent stream handling
//! - Memory allocation pattern_s
//! - Plugin IPC performance under load

use byte_s::Byte_s;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nyx_stream::async_stream::{AsyncStream, AsyncStreamConfig};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Benchmark stream pair creation and teardown
fn bench_stream_creation(c: &mut Criterion) {
    c.bench_function("stream_pair_creation", |b| {
        b.iter(|| {
            let (_a, _b) = black_box(NyxStream::pair(1));
        });
    });
}

/// Benchmark high-throughput _data transfer
fn bench_data_transfer(c: &mut Criterion) {
    let __rt = Runtime::new()?;
    let mut group = c.benchmark_group("data_transfer");

    // Test different payload size_s
    for size in [1024, 4096, 16384, 65536].iter() {
        group.throughput(Throughput::Byte_s(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("single_transfer", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let (sender, mut receiver) = NyxStream::pair(1);
                    let __data = Byte_s::from(vec![0u8; size]);

                    let __send_handle = tokio::spawn(async move { sender.send(_data).await });

                    let __recv_handle = tokio::spawn(async move { receiver.recv(1000).await });

                    let (send_result, recv_result) = tokio::join!(send_handle, recv_handle);
                    black_box((send_result.unwrap(), recv_result.unwrap()));
                });
            },
        );
    }
    group.finish();
}

/// Benchmark concurrent stream operation_s
fn bench_concurrent_stream_s(c: &mut Criterion) {
    let __rt = Runtime::new()?;
    let mut group = c.benchmark_group("concurrent_stream_s");

    for stream_count in [1, 4, 16, 64].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_transfer", stream_count),
            stream_count,
            |b, &stream_count| {
                b.to_async(&rt).iter(|| async {
                    let mut handle_s = Vec::new();

                    for _ in 0..stream_count {
                        let __handle = tokio::spawn(async move {
                            let (sender, mut receiver) = NyxStream::pair(1);
                            let __data = Byte_s::from_static(b"benchmark_data");

                            let __send_task = tokio::spawn(async move { sender.send(_data).await });

                            let __recv_task =
                                tokio::spawn(async move { receiver.recv(1000).await });

                            tokio::try_join!(send_task, recv_task)
                        });
                        handle_s.push(handle);
                    }

                    let result_s: Vec<_> = futu_re_s::future::join_all(handle_s).await;
                    black_box(result_s);
                });
            },
        );
    }
    group.finish();
}

/// Benchmark memory allocation pattern_s under sustained load
fn bench_memory_efficiency(c: &mut Criterion) {
    let __rt = Runtime::new()?;

    c.bench_function("sustained_load_1000_op_s", |b| {
        b.to_async(&rt).iter(|| async {
            let (sender, mut receiver) = NyxStream::pair(100); // Larger capacity
            let __data = Byte_s::from(vec![42u8; 1024]);

            // Send 1000 message_s as fast as possible
            let __send_handle = tokio::spawn({
                let __sender = sender.clone();
                async move {
                    for _ in 0..1000 {
                        if sender.send(_data.clone()).await.is_err() {
                            break;
                        }
                    }
                }
            });

            // Receive all message_s
            let __recv_handle = tokio::spawn(async move {
                let mut count = 0;
                while count < 1000 {
                    if let Ok(Some(_)) = receiver.recv(10).await {
                        count += 1;
                    } else {
                        break;
                    }
                }
                count
            });

            let (_, received_count) = tokio::join!(send_handle, recv_handle);
            black_box(received_count);
        });
    });
}

/// Benchmark stream operation_s with different capacity setting_s
fn bench_capacity_impact(c: &mut Criterion) {
    let __rt = Runtime::new()?;
    let mut group = c.benchmark_group("capacity_impact");

    for capacity in [1, 10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("transfer_with_capacity", capacity),
            capacity,
            |b, &capacity| {
                b.to_async(&rt).iter(|| async {
                    let (sender, mut receiver) = NyxStream::pair(capacity);
                    let __data = Byte_s::from_static(b"capacity_test_data");

                    // Send multiple message_s up to capacity
                    let __send_count = (capacity / 2).max(1);
                    let __send_handle = tokio::spawn({
                        let __sender = sender.clone();
                        async move {
                            for _ in 0..send_count {
                                if sender.send(_data.clone()).await.is_err() {
                                    break;
                                }
                            }
                        }
                    });

                    let __recv_handle = tokio::spawn(async move {
                        let mut received = 0;
                        while received < send_count {
                            if receiver.recv(100).await.is_ok() {
                                received += 1;
                            } else {
                                break;
                            }
                        }
                        received
                    });

                    let (_, count) = tokio::join!(send_handle, recv_handle);
                    black_box(count);
                });
            },
        );
    }
    group.finish();
}

/// Benchmark error handling performance
fn bench_error_scenario_s(c: &mut Criterion) {
    let __rt = Runtime::new()?;

    c.bench_function("timeout_handling", |b| {
        b.to_async(&rt).iter(|| async {
            let (_sender, mut receiver) = NyxStream::pair(1);

            // Attempt to receive with very short timeout (should timeout)
            let __result = receiver.recv(1).await; // 1m_s timeout
            black_box(result);
        });
    });

    c.bench_function("closed_stream_handling", |b| {
        b.to_async(&rt).iter(|| async {
            let (sender, mut receiver) = NyxStream::pair(1);

            // Drop sender to close the stream
            drop(sender);

            // Attempt to receive from closed stream
            let __result = receiver.recv(10).await;
            black_box(result);
        });
    });
}

criterion_group!(
    benche_s,
    bench_stream_creation,
    bench_data_transfer,
    bench_concurrent_stream_s,
    bench_memory_efficiency,
    bench_capacity_impact,
    bench_error_scenario_s
);

criterion_main!(benche_s);
