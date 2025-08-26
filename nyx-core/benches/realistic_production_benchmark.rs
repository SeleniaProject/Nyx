//! 🚀 実運用NyxNet高性能ベンチマーク
//! 
//! 実際の匿名ネットワーク使用パターンをシミュレート:
//! - Webブラウジング、ストリーミング、ファイル転送
//! - 複数ユーザー同時接続
//! - ネットワーク負荷・制約条件
//! - メモリ効率とパフォーマンス

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use nyx_core::performance::RateLimiter;
use nyx_transport::{UdpEndpoint, TransportManager, TransportRequirements};
use nyx_stream::async_stream::{pair, AsyncStreamConfig};
use nyx_stream::performance::StreamMetrics;
use bytes::Bytes;
use std::time::Duration;
use tokio::runtime::Runtime;
use std::sync::Arc;
use futures::future;

// 実際のトラフィックパターンに基づくメッセージサイズ
const SMALL_MSG: usize = 512;     // 制御メッセージ
const MEDIUM_MSG: usize = 1420;   // 標準MTUペイロード
const LARGE_MSG: usize = 8192;    // ファイル転送チャンク
const BURST_MSG: usize = 32768;   // 大容量ダウンロード

/// ベンチマーク: Webブラウジングシナリオ
/// 最も一般的な使用ケース
fn bench_web_browsing_scenarios(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("🌐_web_browsing");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(6));
    
    let scenarios = [
        ("text_page", SMALL_MSG, 10),
        ("image_page", MEDIUM_MSG, 20),  // 50->20に削減
        ("video_stream", LARGE_MSG, 30), // 100->30に削減
        ("file_download", BURST_MSG, 50), // 200->50に削減
    ];
    
    for (name, size, count) in scenarios {
        group.throughput(Throughput::Bytes((size * count) as u64));
        group.bench_with_input(BenchmarkId::new("scenario", name), &(size, count), |b, &(msg_size, msg_count)| {
            b.to_async(&rt).iter(|| async {
                let config = AsyncStreamConfig {
                    max_inflight: 64,
                    retransmit_timeout: Duration::from_millis(50),
                    ..Default::default()
                };
                
                let (sender, receiver) = pair(config.clone(), config);
                
                // 並行でメッセージ送受信
                let send_task = tokio::spawn(async move {
                    for i in 0..msg_count {
                        let data = Bytes::from(vec![42u8; msg_size]);
                        if sender.send(data).await.is_err() {
                            break;
                        }
                        
                        // リアルなユーザー操作間隔
                        if i % 10 == 0 {
                            tokio::time::sleep(Duration::from_micros(50)).await;
                        }
                    }
                });
                
                let recv_task = tokio::spawn(async move {
                    let mut received = 0;
                    while received < msg_count {
                        if let Ok(Some(_)) = receiver.recv().await {
                            received += 1;
                        }
                    }
                    received
                });
                
                let (_, received) = tokio::join!(send_task, recv_task);
                black_box(received.unwrap_or(0));
            });
        });
    }
    
    group.finish();
}

/// ベンチマーク: 同時接続ユーザー負荷
/// リレーノードの実際の負荷をシミュレート
fn bench_concurrent_users(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("👥_concurrent_users");
    group.sample_size(40);
    group.measurement_time(Duration::from_secs(12));
    
    let user_counts = [1, 5, 10, 25, 50];
    
    for user_count in user_counts {
        group.bench_with_input(BenchmarkId::new("users", user_count), &user_count, |b, &users| {
            b.to_async(&rt).iter(|| async {
                let mut tasks = Vec::new();
                
                for user_id in 0..users {
                    let task = tokio::spawn(async move {
                        let config = AsyncStreamConfig {
                            stream_id: user_id as u32,
                            max_inflight: 16,
                            ..Default::default()
                        };
                        
                        let (send, recv) = pair(config.clone(), config);
                        let mut processed = 0;
                        
                        for i in 0..20 {
                            // レート制限を簡単にシミュレート
                            if i % 5 == 0 {
                                tokio::time::sleep(Duration::from_micros(100)).await;
                            }
                            
                            let data = Bytes::from(vec![(user_id as u8 + i as u8) % 255; MEDIUM_MSG]);
                            
                            if send.send(data).await.is_ok() {
                                if let Ok(Some(_)) = recv.recv().await {
                                    processed += 1;
                                }
                            }
                        }
                        
                        processed
                    });
                    
                    tasks.push(task);
                }
                
                let results = future::join_all(tasks).await;
                let total: i32 = results.into_iter().map(|r| r.unwrap_or(0)).sum();
                black_box(total);
            });
        });
    }
    
    group.finish();
}

/// ベンチマーク: メモリ効率テスト
/// 長時間運用での安定性
fn bench_memory_efficiency(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("memory_sustained_load", |b| {
        b.to_async(&rt).iter(|| async {
            let metrics = Arc::new(StreamMetrics::new());
            
            let config = AsyncStreamConfig {
                max_inflight: 128,
                ..Default::default()
            };
            
            let (sender, receiver) = pair(config.clone(), config);
            
            // 複数サイズのメッセージを効率的に処理
            for size in [SMALL_MSG, MEDIUM_MSG, LARGE_MSG] {
                for i in 0..50 {
                    let data = Bytes::from(vec![(i % 255) as u8; size]);
                    
                    if sender.send(data).await.is_ok() {
                        if let Ok(Some(_)) = receiver.recv().await {
                            // メモリ使用量を安定に保つ
                        }
                    }
                }
            }
            
            let stats = metrics.frames_sent.load(std::sync::atomic::Ordering::Relaxed);
            black_box(stats);
        });
    });
}

/// ベンチマーク: ネットワーク制約下でのパフォーマンス
/// 実際のネットワーク条件をシミュレート
fn bench_network_conditions(c: &mut Criterion) {
    let mut group = c.benchmark_group("🌐_network_conditions");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(8));
    
    let conditions = [
        ("optimal", 10),    // 10ms遅延
        ("good", 50),       // 50ms遅延
        ("poor", 200),      // 200ms遅延
        ("mobile", 500),    // 500ms遅延
    ];
    
    for (condition, latency_ms) in conditions {
        group.bench_with_input(BenchmarkId::new("condition", condition), &latency_ms, |b, &latency| {
            b.iter(|| {
                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    let transport_manager = TransportManager::new();
                    let requirements = TransportRequirements {
                        requires_reliability: true,
                        prefers_low_latency: latency < 100,
                        max_latency: Some(Duration::from_millis(latency as u64)),
                        ..Default::default()
                    };
                    
                    let _selected = transport_manager.select_transport(&requirements);
                    
                    // UDPエンドポイントでのストレステスト
                    let mut endpoint1 = UdpEndpoint::bind_loopback().unwrap();
                    let mut endpoint2 = UdpEndpoint::bind_loopback().unwrap();
                    let addr2 = endpoint2.local_addr().unwrap();
                    
                    for i in 0..50 {
                        let data = vec![(i % 255) as u8; MEDIUM_MSG];
                        
                        // 遅延をシミュレート
                        if latency > 50 {
                            std::thread::sleep(Duration::from_micros(latency as u64 * 5));
                        }
                        
                        let _ = endpoint1.send_to_buffered(&data, addr2);
                        
                        if i % 10 == 0 {
                            let mut buf = vec![0u8; MEDIUM_MSG + 100];
                            let _ = endpoint2.recv_from(&mut buf);
                        }
                    }
                    
                    let stats = endpoint1.get_stats();
                    black_box(stats);
                });
            });
        });
    }
    
    group.finish();
}

/// ベンチマーク: エンドツーエンド完全フロー
/// 3ホップ匿名ネットワークの完全シミュレーション
fn bench_end_to_end_flow(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("🔄_complete_anonymity_flow", |b| {
        b.to_async(&rt).iter(|| async {
            let config = AsyncStreamConfig {
                max_inflight: 32,
                retransmit_timeout: Duration::from_millis(100),
                ..Default::default()
            };
            
            // 3ホップパス: クライアント -> ガード -> ミドル -> エグジット
            let (client_send, guard_recv) = pair(config.clone(), config.clone());
            let (guard_send, middle_recv) = pair(config.clone(), config.clone());
            let (middle_send, exit_recv) = pair(config.clone(), config);
            
            let web_request = Bytes::from(b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n".to_vec());
            
            // クライアントがリクエスト送信
            let client_task = tokio::spawn(async move {
                client_send.send(web_request).await.ok()
            });
            
            // ガードリレー
            let guard_task = tokio::spawn(async move {
                if let Ok(Some(data)) = guard_recv.recv().await {
                    tokio::time::sleep(Duration::from_micros(50)).await;
                    guard_send.send(data).await.ok();
                }
            });
            
            // ミドルリレー
            let middle_task = tokio::spawn(async move {
                if let Ok(Some(data)) = middle_recv.recv().await {
                    tokio::time::sleep(Duration::from_micros(50)).await;
                    middle_send.send(data).await.ok();
                }
            });
            
            // エグジットリレー
            let exit_task = tokio::spawn(async move {
                if let Ok(Some(request)) = exit_recv.recv().await {
                    black_box(request.len());
                    return true;
                }
                false
            });
            
            let (client_result, _, _, exit_result) = tokio::join!(
                client_task, guard_task, middle_task, exit_task
            );
            
            black_box((client_result.unwrap_or(None), exit_result.unwrap_or(false)));
        });
    });
}

criterion_group!(
    production_benchmarks,
    bench_web_browsing_scenarios,
    bench_concurrent_users,
    bench_memory_efficiency,
    bench_network_conditions,
    bench_end_to_end_flow
);

criterion_main!(production_benchmarks);
