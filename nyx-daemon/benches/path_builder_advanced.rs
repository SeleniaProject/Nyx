#![forbid(unsafe_code)]

//! Benchmark tests for advanced path selection and diversity scoring
//!
//! NOTE: This bench is self-contained. It does not depend on `nyx-daemon`
//! internal/private types or async networking. It simulates peer data and
//! benchmarks selection/diversity algorithms deterministically.

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct BenchPeer {
    peer_id: String,
    region: Option<String>,
    capabilities: HashSet<String>,
    latency_ms: Option<f64>,
    reliability_score: f64,
    bandwidth_mbps: Option<f64>,
    location: Option<(f64, f64)>, // lat, lon
}

/// Simple diversity score based on unique regions and capability mix
fn calculate_path_diversity_score(peers: &[BenchPeer]) -> f64 {
    if peers.is_empty() {
        return 0.0;
    }
    let mut regions = HashSet::new();
    let mut caps: HashMap<String, usize> = HashMap::new();
    for p in peers {
        if let Some(r) = &p.region {
            regions.insert(r.clone());
        }
        for c in &p.capabilities {
            *caps.entry(c.clone()).or_insert(0) += 1;
        }
    }
    let region_diversity = regions.len() as f64 / peers.len() as f64;
    let cap_diversity = (caps.len() as f64).min(peers.len() as f64) / peers.len() as f64;
    // Weighted sum: regions more important
    0.7 * region_diversity + 0.3 * cap_diversity
}

/// Optimize in-place by sorting with a composite score
fn optimize_peer_selection(peers: &mut [BenchPeer]) -> Result<(), ()> {
    peers.sort_by(|a, b| {
        let ascore = composite_score(a);
        let bscore = composite_score(b);
        bscore
            .partial_cmp(&ascore)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(())
}

fn composite_score(p: &BenchPeer) -> f64 {
    let latency = p.latency_ms.unwrap_or(300.0);
    let bandwidth = p.bandwidth_mbps.unwrap_or(10.0);
    // Higher is better
    let reliability = p.reliability_score;
    // Convert latency to a decreasing contribution (avoid div-by-zero)
    let latency_component = 1.0 / (1.0 + latency.max(1.0) / 100.0);
    // Normalize bandwidth (cap at 1000 Mbps for scale)
    let bandwidth_component = (bandwidth / 1000.0).min(1.0);
    0.5 * reliability + 0.3 * bandwidth_component + 0.2 * latency_component
}

/// Select a diverse subset of peers prioritizing region diversity and quality
fn select_diverse_path_peers(peers: &[BenchPeer], count: usize) -> Result<Vec<BenchPeer>, ()> {
    if peers.is_empty() || count == 0 {
        return Ok(Vec::new());
    }
    // Start from optimized order (best first)
    let mut sorted = peers.to_vec();
    optimize_peer_selection(&mut sorted).map_err(|_| ())?;
    let mut selected: Vec<BenchPeer> = Vec::with_capacity(count);
    let mut used_regions: HashSet<String> = HashSet::new();
    for p in sorted.into_iter() {
        let region_ok = match &p.region {
            Some(r) => {
                if !used_regions.contains(r) {
                    used_regions.insert(r.clone());
                    true
                } else {
                    // Allow duplicates if we still need more peers
                    selected.len() < count / 2
                }
            }
            None => selected.len() < count / 2,
        };
        if region_ok {
            selected.push(p);
            if selected.len() >= count {
                break;
            }
        }
    }
    if selected.len() < count {
        // Fill remaining slots from any remaining peers (fallback)
        let mut fallback_needed = count - selected.len();
        for p in peers.iter() {
            if !selected.iter().any(|s| s.peer_id == p.peer_id) {
                selected.push(p.clone());
                fallback_needed -= 1;
                if fallback_needed == 0 {
                    break;
                }
            }
        }
    }
    Ok(selected)
}

/// Create a large set of diverse test peers for benchmarking
fn create_benchmark_peers(count: usize) -> Vec<BenchPeer> {
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

        let capabilities_set: HashSet<String> =
            capabilities.iter().map(|s| s.to_string()).collect();

        let peer = BenchPeer {
            peer_id: format!("benchmark-peer-{}", i),
            capabilities: capabilities_set,
            region: Some(region.to_string()),
            location: Some((lat, lon)),
            latency_ms: Some(50.0 + (i as f64 * 10.0) % 400.0), // 50-450ms
            reliability_score: 0.5 + (i as f64 * 0.001) % 0.5,  // 0.5-1.0
            bandwidth_mbps: Some(10.0 + (i as f64 * 2.0) % 990.0), // 10-1000 Mbps
        };

        peers.push(peer);
    }

    peers
}

/// Benchmark path diversity calculation performance
fn bench_path_diversity_calculation(c: &mut Criterion) {
    let mut group = c.benchmark_group("path_diversity_calculation");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(30));
    group.warm_up_time(Duration::from_secs(5));

    for path_length in [3, 5, 7, 10].iter() {
        let test_peers = create_benchmark_peers(*path_length);

        group.throughput(Throughput::Elements(*path_length as u64));
        group.bench_with_input(
            BenchmarkId::new("diversity_score", path_length),
            path_length,
            |b, &_path_length| {
                b.iter(|| black_box(calculate_path_diversity_score(black_box(&test_peers))))
            },
        );
    }

    group.finish();
}

/// Benchmark peer selection optimization performance
fn bench_peer_selection_optimization(c: &mut Criterion) {
    let mut group = c.benchmark_group("peer_selection_optimization");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(60));
    group.warm_up_time(Duration::from_secs(5));

    for candidate_count in [50, 100, 500, 1000].iter() {
        let test_peers = create_benchmark_peers(*candidate_count);

        group.throughput(Throughput::Elements(*candidate_count as u64));
        group.bench_with_input(
            BenchmarkId::new("optimize_selection", candidate_count),
            candidate_count,
            |b, &_candidate_count| {
                b.iter_batched(
                    || black_box(test_peers.clone()),
                    |mut peers| {
                        optimize_peer_selection(black_box(&mut peers)).unwrap();
                    },
                    BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

/// Benchmark diverse path selection performance
fn bench_diverse_path_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("diverse_path_selection");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(45));
    group.warm_up_time(Duration::from_secs(5));

    for (candidates, path_len) in [(100, 3), (500, 5), (1000, 7)].iter() {
        let test_peers = create_benchmark_peers(*candidates);

        group.throughput(Throughput::Elements(*path_len as u64));
        group.bench_with_input(
            BenchmarkId::new(
                "select_diverse_peers",
                format!("{}c_{}p", candidates, path_len),
            ),
            &(*candidates, *path_len),
            |b, &(_candidates, path_len)| {
                b.iter_batched(
                    || black_box(test_peers.clone()),
                    |peers| {
                        let selected =
                            select_diverse_path_peers(black_box(&peers), black_box(path_len))
                                .unwrap();
                        black_box(selected)
                    },
                    BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

/// Benchmark advanced peer scoring performance
fn bench_advanced_peer_scoring(c: &mut Criterion) {
    let mut group = c.benchmark_group("advanced_peer_scoring");
    group.sample_size(200);
    group.warm_up_time(Duration::from_secs(5));

    let test_peers = create_benchmark_peers(1000);
    group.throughput(Throughput::Elements(test_peers.len() as u64));

    group.bench_function("calculate_advanced_peer_score", |b| {
        b.iter(|| {
            for peer in &test_peers {
                let _ = black_box(composite_score(black_box(peer)));
            }
        })
    });

    group.finish();
}

/// Memory usage benchmark for path selection algorithms
fn bench_memory_usage(c: &mut Criterion) {
    // Small/medium cases
    {
        let mut group = c.benchmark_group("memory_usage_small");
        group.sample_size(20);
        group.measurement_time(Duration::from_secs(30));
        group.warm_up_time(Duration::from_secs(5));

        for peer_count in [1000, 5000].iter() {
            group.throughput(Throughput::Elements(*peer_count as u64));
            group.bench_with_input(
                BenchmarkId::new("large_peer_set_processing", peer_count),
                peer_count,
                |b, &peer_count| {
                    b.iter_batched(
                        || black_box(create_benchmark_peers(peer_count)),
                        |test_peers| {
                            let selected =
                                select_diverse_path_peers(black_box(&test_peers), black_box(5))
                                    .unwrap();
                            let diversity_score =
                                black_box(calculate_path_diversity_score(black_box(&selected)));
                            black_box((selected.len(), diversity_score))
                        },
                        BatchSize::LargeInput,
                    )
                },
            );
        }

        group.finish();
    }

    // Large case split out with longer measurement
    {
        let mut group = c.benchmark_group("memory_usage_large");
        group.sample_size(12);
        group.measurement_time(Duration::from_secs(60));
        group.warm_up_time(Duration::from_secs(8));

        let peer_count: usize = 10_000;
        group.throughput(Throughput::Elements(peer_count as u64));
        group.bench_with_input(
            BenchmarkId::new("large_peer_set_processing", &peer_count),
            &peer_count,
            |b, &peer_count| {
                b.iter_batched(
                    || black_box(create_benchmark_peers(peer_count)),
                    |test_peers| {
                        let selected =
                            select_diverse_path_peers(black_box(&test_peers), black_box(5))
                                .unwrap();
                        let diversity_score =
                            black_box(calculate_path_diversity_score(black_box(&selected)));
                        black_box((selected.len(), diversity_score))
                    },
                    BatchSize::LargeInput,
                )
            },
        );

        group.finish();
    }
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
