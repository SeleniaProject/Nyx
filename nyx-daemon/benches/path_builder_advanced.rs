#![forbid(unsafe_code)]

//! Benchmark tests for advanced path builder performance validation
//!
//! These tests measure the performance characteristics of the advanced
//! path building algorithms to ensure they meet production requirements.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use geo::Point;
use multiaddr::Multiaddr;
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

use nyx_daemon::path_builder::{CachedPeerInfo, DhtPeerDiscovery, DiscoveryCriteria};
use nyx_daemon::pure_rust_dht_tcp::{DhtConfig, PureRustDht};

/// Create a large set of diverse test peers for benchmarking
fn create_benchmark_peers(count: usize) -> Vec<CachedPeerInfo> {
    let mut peers = Vec::with_capacity(count);
    let regions = vec![
        "us-east-1",
        "us-west-1",
        "eu-west-1",
        "eu-central-1",
        "asia-east-1",
        "asia-south-1",
    ];
    let capabilities_sets = vec![
        vec!["onion-routing", "exit-node"],
        vec!["onion-routing", "relay"],
        vec!["directory", "relay"],
        vec!["onion-routing", "high-bandwidth"],
        vec!["exit-node", "directory"],
    ];

    for i in 0..count {
        let region = &regions[i % regions.len()];
        let capabilities = &capabilities_sets[i % capabilities_sets.len()];

        // Generate diverse locations within regions
        let (lat, lon) = match region {
            &"us-east-1" => (
                40.0 + (i as f64 * 0.1) % 5.0,
                -75.0 + (i as f64 * 0.1) % 5.0,
            ),
            &"us-west-1" => (
                37.0 + (i as f64 * 0.1) % 5.0,
                -122.0 + (i as f64 * 0.1) % 5.0,
            ),
            &"eu-west-1" => (51.0 + (i as f64 * 0.1) % 5.0, -1.0 + (i as f64 * 0.1) % 5.0),
            &"eu-central-1" => (52.0 + (i as f64 * 0.1) % 5.0, 13.0 + (i as f64 * 0.1) % 5.0),
            &"asia-east-1" => (
                35.0 + (i as f64 * 0.1) % 5.0,
                139.0 + (i as f64 * 0.1) % 5.0,
            ),
            _ => (0.0, 0.0),
        };

        let multiaddr: Multiaddr = format!(
            "/ip4/10.{}.{}.{}/tcp/{}",
            (i / 256) % 256,
            i % 256,
            (i * 3) % 256,
            8080 + (i % 1000)
        )
        .parse()
        .expect("Valid multiaddr");

        let capabilities_set: HashSet<String> =
            capabilities.iter().map(|s| s.to_string()).collect();

        let peer = CachedPeerInfo {
            peer_id: format!("benchmark-peer-{}", i),
            addresses: vec![multiaddr],
            capabilities: capabilities_set,
            region: Some(region.to_string()),
            location: Some(Point::new(lat, lon)),
            latency_ms: Some(50.0 + (i as f64 * 10.0) % 400.0), // 50-450ms
            reliability_score: 0.5 + (i as f64 * 0.001) % 0.5,  // 0.5-1.0
            bandwidth_mbps: Some(10.0 + (i as f64 * 2.0) % 990.0), // 10-1000 Mbps
            last_seen: std::time::Instant::now(),
            response_time_ms: Some(30.0 + (i as f64 * 5.0) % 200.0),
            last_active_rtt: None,
            last_active_bandwidth: None,
        };

        peers.push(peer);
    }

    peers
}

/// Benchmark path diversity calculation performance
fn bench_path_diversity_calculation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("path_diversity_calculation");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(30));

    for path_length in [3, 5, 7, 10].iter() {
        let test_peers = create_benchmark_peers(*path_length);

        group.bench_with_input(
            BenchmarkId::new("diversity_score", path_length),
            path_length,
            |b, &_path_length| {
                b.iter(|| {
                    rt.block_on(async {
                        let dht_config = DhtConfig::default();
                        let dht = Arc::new(RwLock::new(Some(PureRustDht::new(dht_config))));
                        let peer_discovery =
                            DhtPeerDiscovery::new(dht).await.expect("DHT creation");

                        peer_discovery
                            .calculate_path_diversity_score(&test_peers)
                            .await
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark peer selection optimization performance  
fn bench_peer_selection_optimization(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("peer_selection_optimization");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(60));

    for candidate_count in [50, 100, 500, 1000].iter() {
        let test_peers = create_benchmark_peers(*candidate_count);

        group.bench_with_input(
            BenchmarkId::new("optimize_selection", candidate_count),
            candidate_count,
            |b, &_candidate_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let dht_config = DhtConfig::default();
                        let dht = Arc::new(RwLock::new(Some(PureRustDht::new(dht_config))));
                        let peer_discovery =
                            DhtPeerDiscovery::new(dht).await.expect("DHT creation");

                        let mut peers = test_peers.clone();
                        peer_discovery
                            .optimize_peer_selection(&mut peers)
                            .await
                            .unwrap()
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark diverse path selection performance
fn bench_diverse_path_selection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("diverse_path_selection");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(45));

    for (candidates, path_len) in [(100, 3), (500, 5), (1000, 7)].iter() {
        let test_peers = create_benchmark_peers(*candidates);

        group.bench_with_input(
            BenchmarkId::new(
                "select_diverse_peers",
                format!("{}c_{}p", candidates, path_len),
            ),
            &(*candidates, *path_len),
            |b, &(candidates, path_len)| {
                b.iter(|| {
                    rt.block_on(async {
                        let dht_config = DhtConfig::default();
                        let dht = Arc::new(RwLock::new(Some(PureRustDht::new(dht_config))));
                        let peer_discovery =
                            DhtPeerDiscovery::new(dht).await.expect("DHT creation");

                        peer_discovery
                            .select_diverse_path_peers(&test_peers, path_len)
                            .await
                            .unwrap()
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark advanced peer scoring performance
fn bench_advanced_peer_scoring(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("advanced_peer_scoring");
    group.sample_size(200);

    let test_peers = create_benchmark_peers(1000);
    let dht_config = DhtConfig::default();
    let dht = Arc::new(RwLock::new(Some(PureRustDht::new(dht_config))));
    let peer_discovery =
        rt.block_on(async { DhtPeerDiscovery::new(dht).await.expect("DHT creation") });

    group.bench_function("calculate_advanced_peer_score", |b| {
        b.iter(|| {
            for peer in &test_peers {
                peer_discovery.calculate_advanced_peer_score(peer);
            }
        })
    });

    group.finish();
}

/// Memory usage benchmark for path selection algorithms
fn bench_memory_usage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("memory_usage");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    // Test with increasing peer counts to measure memory scaling
    for peer_count in [1000, 5000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("large_peer_set_processing", peer_count),
            peer_count,
            |b, &peer_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let test_peers = create_benchmark_peers(peer_count);
                        let dht_config = DhtConfig::default();
                        let dht = Arc::new(RwLock::new(Some(PureRustDht::new(dht_config))));
                        let peer_discovery =
                            DhtPeerDiscovery::new(dht).await.expect("DHT creation");

                        // Process the large peer set
                        let selected = peer_discovery
                            .select_diverse_path_peers(&test_peers, 5)
                            .await
                            .unwrap();
                        let diversity_score = peer_discovery
                            .calculate_path_diversity_score(&selected)
                            .await;

                        (selected.len(), diversity_score)
                    })
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_path_diversity_calculation,
    bench_peer_selection_optimization,
    bench_diverse_path_selection,
    bench_advanced_peer_scoring,
    bench_memory_usage
);

criterion_main!(benches);
