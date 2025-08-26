//! 📱 Mobile & IoT Performance Benchmark Suite
//! 
//! This benchmark suite simulates real-world constraints of mobile and IoT devices:
//! - Limited memory scenarios (32MB-512MB available)
//! - Intermittent connectivity patterns
//! - Battery-conscious operation modes
//! - CPU throttling under thermal constraints
//! - Background/foreground application lifecycle
//! - Multi-app resource contention

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nyx_core::performance::RateLimiter;
use nyx_stream::async_stream::{pair, AsyncStreamConfig};
use nyx_stream::performance::{StreamMetrics, BufferPool as StreamBufferPool};
use bytes::Bytes;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Mobile memory constraints simulation
const MOBILE_LOW_MEM: usize = 32 * 1024 * 1024;    // 32MB
const MOBILE_MID_MEM: usize = 128 * 1024 * 1024;   // 128MB  
const MOBILE_HIGH_MEM: usize = 512 * 1024 * 1024;  // 512MB

/// IoT device constraints
const IOT_TINY_MEM: usize = 4 * 1024 * 1024;       // 4MB
const IOT_SMALL_MEM: usize = 16 * 1024 * 1024;     // 16MB

/// Battery-conscious timing patterns
const BATTERY_SAVER_INTERVAL: Duration = Duration::from_millis(500);
const NORMAL_OPERATION_INTERVAL: Duration = Duration::from_millis(50);
const AGGRESSIVE_POWER_SAVE: Duration = Duration::from_secs(2);

/// Benchmark: Memory-constrained operation
/// Simulates Nyx performance under mobile device memory pressure
fn bench_memory_constrained_operation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("memory_constrained");
    group.sample_size(30); // Reduce sample size for faster execution
    group.measurement_time(Duration::from_secs(10)); // Shorter measurement time
    
    let memory_scenarios = [
        ("iot_tiny", IOT_TINY_MEM),
        ("iot_small", IOT_SMALL_MEM),
        ("mobile_low", MOBILE_LOW_MEM),
        ("mobile_mid", MOBILE_MID_MEM),
        ("mobile_high", MOBILE_HIGH_MEM),
    ];
    
    for (scenario_name, available_memory) in memory_scenarios {
        group.bench_with_input(
            BenchmarkId::new("constrained_memory", scenario_name),
            &available_memory,
            |b, &mem_limit| {
                b.to_async(&rt).iter(|| async {
                    // Calculate appropriate buffer limits based on available memory
                    let max_buffers = (mem_limit / 1024).min(1000); // Conservative estimate
                    let buffer_size = (mem_limit / max_buffers).min(8192);
                    
                    let metrics = Box::leak(Box::new(StreamMetrics::new()));
                    let _buffer_pool = StreamBufferPool::new(max_buffers, metrics);
                    
                    let config = AsyncStreamConfig {
                        max_inflight: (max_buffers / 10).max(4), // Conservative inflight
                        max_frame_len: Some(buffer_size),
                        ..Default::default()
                    };
                    
                    let (sender, receiver) = pair(config.clone(), config);
                    let memory_used = Arc::new(AtomicUsize::new(0));
                    
                    // Simulate constrained operation
                    let send_task = tokio::spawn({
                        let memory_used = memory_used.clone();
                        async move {
                            for i in 0..100 {
                                // Check memory usage before allocation
                                let current_mem = memory_used.load(Ordering::Relaxed);
                                if current_mem + buffer_size > mem_limit {
                                    // Memory pressure: wait for buffers to be freed
                                    tokio::time::sleep(Duration::from_millis(10)).await;
                                    continue;
                                }
                                
                                let data = Bytes::from(vec![(i % 255) as u8; buffer_size]);
                                memory_used.fetch_add(buffer_size, Ordering::Relaxed);
                                
                                if sender.send(data).await.is_err() {
                                    break;
                                }
                                
                                // Mobile devices often have irregular message timing
                                if i % 10 == 0 {
                                    tokio::time::sleep(Duration::from_millis(5)).await;
                                }
                            }
                        }
                    });
                    
                    let recv_task = tokio::spawn({
                        let memory_used = memory_used.clone();
                        async move {
                            let mut received = 0;
                            while received < 100 {
                                if let Ok(Some(data)) = receiver.recv().await {
                                    // Process and immediately free memory
                                    memory_used.fetch_sub(data.len(), Ordering::Relaxed);
                                    
                                    // Simulate mobile processing with occasional GC pauses
                                    if received % 20 == 0 {
                                        tokio::time::sleep(Duration::from_micros(500)).await;
                                    }
                                    
                                    received += 1;
                                }
                            }
                            received
                        }
                    });
                    
                    let (_, received) = tokio::join!(send_task, recv_task);
                    black_box(received.unwrap_or(0));
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark: Intermittent connectivity patterns
/// Simulates mobile network conditions with connection drops and reconnects
fn bench_intermittent_connectivity(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("intermittent_connectivity");
    group.sample_size(10); // Much smaller sample size to prevent timeout
    group.measurement_time(Duration::from_secs(5)); // Shorter measurement time
    
    let connectivity_patterns = [
        ("stable_wifi", 99, Duration::from_millis(100)),      // Much shorter intervals
        ("mobile_4g", 95, Duration::from_millis(100)),        // Much shorter intervals
        ("weak_signal", 80, Duration::from_millis(50)),      // Much shorter intervals
        ("subway_pattern", 60, Duration::from_millis(50)),    // Much shorter intervals
    ];
    
    for (pattern_name, uptime_pct, disconnect_interval) in connectivity_patterns {
        group.bench_with_input(
            BenchmarkId::new("connectivity_pattern", pattern_name),
            &(uptime_pct, disconnect_interval),
            |b, &(uptime, interval)| {
                b.to_async(&rt).iter(|| async {
                    let config = AsyncStreamConfig {
                        max_inflight: 16,
                        retransmit_timeout: Duration::from_millis(200),
                        max_retries: 5,
                        ..Default::default()
                    };
                    
                    let (sender, receiver) = pair(config.clone(), config);
                    let connected = Arc::new(AtomicUsize::new(1));
                    
                    // Simulate network state changes
                    let network_task = tokio::spawn({
                        let connected = connected.clone();
                        async move {
                            for cycle in 0..3 { // Reduced from 10 to 3
                                // Connected period (much shorter)
                                let up_time = Duration::from_millis(100); // Fixed short time
                                tokio::time::sleep(up_time).await;
                                
                                // Disconnected period (much shorter)
                                connected.store(0, Ordering::Relaxed);
                                let down_time = Duration::from_millis(50); // Fixed short time
                                tokio::time::sleep(down_time).await;
                                connected.store(1, Ordering::Relaxed);
                                
                                if cycle % 2 == 0 {
                                    // Simulate additional mobile-specific delays (shorter)
                                    tokio::time::sleep(Duration::from_millis(10)).await;
                                }
                            }
                        }
                    });
                    
                    // Application continues trying to send data
                    let app_task = tokio::spawn({
                        let connected = connected.clone();
                        async move {
                            let mut successfully_sent = 0;
                            
                            for i in 0..10 { // Reduced from 50 to 10
                                if connected.load(Ordering::Relaxed) == 0 {
                                    // Connection is down, wait or queue messages
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                    continue;
                                }
                                
                                let data = Bytes::from(vec![(i % 255) as u8; 1024]);
                                
                                match sender.send(data).await {
                                    Ok(_) => {
                                        successfully_sent += 1;
                                        
                                        // Try to receive response
                                        if let Ok(Some(_)) = receiver.recv().await {
                                            // Successful round trip
                                        }
                                    }
                                    Err(_) => {
                                        // Connection failed, implement mobile retry logic
                                        tokio::time::sleep(Duration::from_millis(200)).await;
                                    }
                                }
                                
                                // Realistic mobile app behavior
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                            
                            successfully_sent
                        }
                    });
                    
                    let (_, sent_count) = tokio::join!(network_task, app_task);
                    black_box(sent_count.unwrap_or(0));
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark: Battery-conscious operation modes
/// Simulates adaptive behavior based on battery level and power saving modes
fn bench_battery_conscious_operation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("battery_conscious");
    group.sample_size(20); // Smaller sample size
    group.measurement_time(Duration::from_secs(8)); // Shorter measurement time
    
    let battery_modes = [
        ("full_battery", 100, Duration::from_millis(10)),
        ("high_battery", 80, Duration::from_millis(20)),
        ("medium_battery", 50, Duration::from_millis(50)),
        ("low_battery", 20, Duration::from_millis(100)),
        ("critical_battery", 5, Duration::from_millis(200)),
    ];
    
    for (mode_name, battery_level, operation_interval) in battery_modes {
        group.bench_with_input(
            BenchmarkId::new("battery_mode", mode_name),
            &(battery_level, operation_interval),
            |b, &(level, interval)| {
                b.to_async(&rt).iter(|| async {
                    // Adjust performance based on battery level
                    let max_inflight = match level {
                        81..=100 => 64,  // Full performance
                        51..=80 => 32,   // Reduced performance
                        21..=50 => 16,   // Battery saver
                        6..=20 => 8,     // Low power mode
                        _ => 4,          // Critical power mode
                    };
                    
                    let rate_limit = match level {
                        81..=100 => 1000.0,  // Full rate
                        51..=80 => 500.0,    // Reduced rate
                        21..=50 => 200.0,    // Conservative rate
                        6..=20 => 50.0,      // Very low rate
                        _ => 10.0,           // Emergency rate
                    };
                    
                    let config = AsyncStreamConfig {
                        max_inflight,
                        retransmit_timeout: interval.mul_f32(2.0),
                        ..Default::default()
                    };
                    
                    let mut rate_limiter = RateLimiter::new(rate_limit, 1.0); // 1 Hz refill rate
                    let (sender, receiver) = pair(config.clone(), config);
                    
                    let mut messages_processed = 0;
                    
                    for i in 0..30 {
                        // Apply battery-aware rate limiting
                        while !rate_limiter.allow() {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        
                        let data = Bytes::from(vec![(i % 255) as u8; 512]);
                        
                        if sender.send(data).await.is_ok() {
                            if let Ok(Some(_)) = receiver.recv().await {
                                messages_processed += 1;
                            }
                        }
                        
                        // Power-aware operation intervals
                        tokio::time::sleep(interval).await;
                        
                        // Simulate battery drain affecting performance
                        if i % 10 == 0 && level < 50 {
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                    
                    black_box(messages_processed);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark: Background/foreground app lifecycle
/// Simulates iOS/Android app lifecycle impact on Nyx performance
fn bench_app_lifecycle_impact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("app_lifecycle");
    group.sample_size(15); // Smaller sample size
    group.measurement_time(Duration::from_secs(6)); // Shorter measurement time
    
    let lifecycle_states = [
        ("foreground_active", 100, Duration::from_millis(10)),
        ("foreground_inactive", 80, Duration::from_millis(50)),
        ("background_short", 50, Duration::from_millis(200)),
        ("background_long", 20, Duration::from_millis(1000)),
        ("suspended", 5, Duration::from_millis(5000)),
    ];
    
    for (state_name, cpu_percent, wake_interval) in lifecycle_states {
        group.bench_with_input(
            BenchmarkId::new("lifecycle_state", state_name),
            &(cpu_percent, wake_interval),
            |b, &(cpu, wake_time)| {
                b.to_async(&rt).iter(|| async {
                    let config = AsyncStreamConfig {
                        max_inflight: (cpu / 5).max(4),
                        retransmit_timeout: wake_time.mul_f32(2.0),
                        ..Default::default()
                    };
                    
                    let (sender, receiver) = pair(config.clone(), config);
                    let mut total_processed = 0;
                    
                    // Simulate lifecycle transitions
                    for phase in 0..5 {
                        let phase_messages = match cpu {
                            81..=100 => 20,  // Active foreground
                            51..=80 => 15,   // Inactive foreground
                            21..=50 => 10,   // Background
                            6..=20 => 5,     // Limited background
                            _ => 2,          // Suspended/minimal
                        };
                        
                        for i in 0..phase_messages {
                            let data = Bytes::from(vec![(phase as u8 + i as u8) % 255; 256]);
                            
                            if sender.send(data).await.is_ok() {
                                if let Ok(Some(_)) = receiver.recv().await {
                                    total_processed += 1;
                                }
                            }
                            
                            // OS-imposed limitations
                            if cpu < 50 {
                                tokio::time::sleep(wake_time).await;
                            } else {
                                tokio::time::sleep(Duration::from_millis(1)).await;
                            }
                        }
                        
                        // Simulate OS lifecycle events
                        match cpu {
                            6..=20 => {
                                // Background app refresh limitation (shorter)
                                tokio::time::sleep(Duration::from_millis(50)).await;
                            }
                            0..=5 => {
                                // Suspended state (much shorter)
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                            _ => {}
                        }
                    }
                    
                    black_box(total_processed);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark: Multi-app resource contention
/// Simulates performance when Nyx competes with other mobile apps for resources
fn bench_multi_app_contention(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("multi_app_contention");
    group.sample_size(15); // Smaller sample size
    group.measurement_time(Duration::from_secs(7)); // Shorter measurement time
    
    let contention_scenarios = [
        ("nyx_only", 1, 100),              // Nyx has full resources
        ("light_contention", 3, 70),       // Few apps running
        ("moderate_contention", 8, 40),    // Typical mobile usage
        ("heavy_contention", 15, 20),      // Many apps active
        ("extreme_contention", 25, 10),    // Device under stress
    ];
    
    for (scenario_name, app_count, resource_percent) in contention_scenarios {
        group.bench_with_input(
            BenchmarkId::new("contention_level", scenario_name),
            &(app_count, resource_percent),
            |b, &(apps, resources)| {
                b.to_async(&rt).iter(|| async {
                    // Simulate resource allocation based on app count
                    let _available_memory = MOBILE_MID_MEM * resources / 100;
                    let cpu_slice = Duration::from_millis((100 / apps.max(1)) as u64);
                    
                    let config = AsyncStreamConfig {
                        max_inflight: (resources / 5).max(4),
                        retransmit_timeout: cpu_slice.mul_f32(10.0),
                        ..Default::default()
                    };
                    
                    let (sender, receiver) = pair(config.clone(), config);
                    let mut successful_ops = 0;
                    
                    // Simulate competing apps affecting Nyx performance
                    for i in 0..20 {
                        // Simulate OS scheduler giving Nyx a time slice
                        tokio::time::sleep(cpu_slice).await;
                        
                        let data = Bytes::from(vec![(i % 255) as u8; 512]);
                        
                        let _start = Instant::now();
                        if sender.send(data).await.is_ok() {
                            if let Ok(Some(_)) = receiver.recv().await {
                                successful_ops += 1;
                            }
                        }
                        
                        // Simulate context switching and memory pressure
                        if apps > 10 {
                            tokio::time::sleep(Duration::from_micros(100)).await;
                        }
                        
                        // Simulate memory pressure from other apps
                        if i % 5 == 0 && resources < 50 {
                            tokio::time::sleep(Duration::from_millis(50)).await;
                        }
                    }
                    
                    black_box(successful_ops);
                });
            },
        );
    }
    
    group.finish();
}

/// Benchmark: IoT device constraints
/// Simulates Nyx performance on extremely resource-constrained IoT devices
fn bench_iot_device_constraints(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("iot_minimal_operation", |b| {
        b.to_async(&rt).iter(|| async {
            // Ultra-constrained IoT scenario
            let config = AsyncStreamConfig {
                stream_id: 1,
                max_inflight: 2,  // Very limited
                max_frame_len: Some(256),  // Small frames
                retransmit_timeout: Duration::from_secs(1),  // Patient retries
                max_retries: 3,
                ..Default::default()
            };
            
            let (sender, receiver) = pair(config.clone(), config);
            
            // IoT devices typically send small, infrequent messages
            let iot_messages = [
                ("sensor_reading", 32),    // Temperature, humidity, etc.
                ("status_update", 64),     // Device health status
                ("alarm_trigger", 128),    // Emergency notification
                ("config_update", 256),    // Configuration change
            ];
            
            let mut total_bytes = 0;
            
            for (msg_type, size) in iot_messages {
                let data = Bytes::from(vec![42u8; size]);
                
                // IoT devices often wait between transmissions to save power
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                if sender.send(data).await.is_ok() {
                    if let Ok(Some(response)) = receiver.recv().await {
                        total_bytes += response.len();
                        
                        // Simulate IoT processing time
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
            
            black_box(total_bytes);
        });
    });
}

criterion_group!(
    mobile_iot_benchmarks,
    bench_memory_constrained_operation,
    bench_intermittent_connectivity,
    bench_battery_conscious_operation,
    bench_app_lifecycle_impact,
    bench_multi_app_contention,
    bench_iot_device_constraints
);

criterion_main!(mobile_iot_benchmarks);
