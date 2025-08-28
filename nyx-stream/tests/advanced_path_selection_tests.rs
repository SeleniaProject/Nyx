//! Tests for Advanced Path Selection Algorithms

#![forbid(unsafe_code)]

use nyx_stream::multipath::scheduler::{PathId, PathMetric};
use nyx_stream::{AdvancedPathSelectionConfig, AdvancedPathSelector, PathSelectionAlgorithm};
use std::time::Duration;

/// Test basic functionality of advanced path selector
#[test]
fn test_advanced_path_selector_basic() {
    let config = AdvancedPathSelectionConfig::default();
    let selector = AdvancedPathSelector::new(config);
    let paths = vec![
        (
            PathId(1),
            PathMetric {
                rtt: Duration::from_millis(50),
                loss: 0.01,
                weight: 1,
            },
        ),
        (
            PathId(2),
            PathMetric {
                rtt: Duration::from_millis(100),
                loss: 0.05,
                weight: 2,
            },
        ),
    ];

    // Initialize paths
    assert!(selector.initialize_paths(&paths).is_ok());

    // Select next path
    let selected = selector.select_next_path();
    assert!(selected.is_ok());

    // Get statistics
    let stats = selector.get_path_statistics();
    assert!(stats.is_ok());
    assert_eq!(stats.unwrap().len(), 2);
}

/// Test round-robin algorithm
#[test]
fn test_round_robin_algorithm() {
    let config = AdvancedPathSelectionConfig {
        algorithm: PathSelectionAlgorithm::RoundRobin,
        ..Default::default()
    };

    let selector = AdvancedPathSelector::new(config);
    let paths = vec![
        (
            PathId(1),
            PathMetric {
                rtt: Duration::from_millis(50),
                loss: 0.01,
                weight: 1,
            },
        ),
        (
            PathId(2),
            PathMetric {
                rtt: Duration::from_millis(50),
                loss: 0.01,
                weight: 1,
            },
        ),
    ];

    selector.initialize_paths(&paths).unwrap();

    // Round-robin should alternate between paths
    let mut selections = Vec::new();
    for _ in 0..6 {
        let selected = selector.select_next_path().unwrap();
        selections.push(selected);
    }

    // Should see both paths selected
    assert!(selections.contains(&PathId(1)));
    assert!(selections.contains(&PathId(2)));
}

/// Test path failure and recovery
#[test]
fn test_path_failure_recovery() {
    let config = AdvancedPathSelectionConfig {
        algorithm: PathSelectionAlgorithm::RoundRobin,
        ..Default::default()
    };

    let selector = AdvancedPathSelector::new(config);
    let paths = vec![
        (
            PathId(1),
            PathMetric {
                rtt: Duration::from_millis(50),
                loss: 0.01,
                weight: 1,
            },
        ),
        (
            PathId(2),
            PathMetric {
                rtt: Duration::from_millis(50),
                loss: 0.01,
                weight: 1,
            },
        ),
    ];

    selector.initialize_paths(&paths).unwrap();

    // Mark Path 1 as failed
    selector.mark_path_failed(PathId(1)).unwrap();

    // All selections should use Path 2
    for _ in 0..5 {
        let selected = selector.select_next_path().unwrap();
        assert_eq!(selected, PathId(2));
    }

    // Recover Path 1
    selector.mark_path_recovered(PathId(1)).unwrap();

    // Now both paths should be available
    let stats = selector.get_path_statistics().unwrap();
    assert!(stats[&PathId(1)].is_available);
    assert!(stats[&PathId(2)].is_available);
}

/// Test RTT and loss observation
#[test]
fn test_rtt_loss_observation() {
    let config = AdvancedPathSelectionConfig::default();
    let selector = AdvancedPathSelector::new(config);
    let paths = vec![(
        PathId(1),
        PathMetric {
            rtt: Duration::from_millis(50),
            loss: 0.0,
            weight: 1,
        },
    )];

    selector.initialize_paths(&paths).unwrap();

    // Observe RTT
    let new_rtt = Duration::from_millis(30);
    selector.observe_rtt(PathId(1), new_rtt).unwrap();

    // Observe packet loss and success
    for _ in 0..90 {
        selector.observe_success(PathId(1)).unwrap();
    }
    for _ in 0..10 {
        selector.observe_loss(PathId(1)).unwrap();
    }

    // Check statistics
    let stats = selector.get_path_statistics().unwrap();
    let path_stats = &stats[&PathId(1)];

    assert_eq!(path_stats.loss_stats.packets_sent, 100);
    assert_eq!(path_stats.loss_stats.packets_lost, 10);
    assert!((path_stats.loss_stats.current_loss_rate - 0.1).abs() < 0.001);
}

/// Test load balancing operations
#[test]
fn test_load_balancing() {
    let config = AdvancedPathSelectionConfig::default();
    let selector = AdvancedPathSelector::new(config);
    let paths = vec![
        (
            PathId(1),
            PathMetric {
                rtt: Duration::from_millis(50),
                loss: 0.01,
                weight: 1,
            },
        ),
        (
            PathId(2),
            PathMetric {
                rtt: Duration::from_millis(100),
                loss: 0.05,
                weight: 2,
            },
        ),
    ];

    selector.initialize_paths(&paths).unwrap();

    // Trigger load balancing
    let result = selector.rebalance_load();
    assert!(result.is_ok());

    // Check metrics
    let metrics = selector.get_selection_metrics().unwrap();
    assert!(metrics.load_balance_operations > 0);
}

/// Test all algorithms work without errors
#[test]
fn test_all_algorithms() {
    let algorithms = vec![
        PathSelectionAlgorithm::RoundRobin,
        PathSelectionAlgorithm::WeightedRoundRobin,
        PathSelectionAlgorithm::LatencyBased,
        PathSelectionAlgorithm::LossAware,
        PathSelectionAlgorithm::BandwidthBased,
        PathSelectionAlgorithm::Hybrid {
            latency_weight: 0.4,
            loss_weight: 0.3,
            bandwidth_weight: 0.3,
        },
        PathSelectionAlgorithm::Adaptive,
    ];

    let paths = vec![
        (
            PathId(1),
            PathMetric {
                rtt: Duration::from_millis(20),
                loss: 0.001,
                weight: 10,
            },
        ),
        (
            PathId(2),
            PathMetric {
                rtt: Duration::from_millis(80),
                loss: 0.02,
                weight: 5,
            },
        ),
        (
            PathId(3),
            PathMetric {
                rtt: Duration::from_millis(200),
                loss: 0.005,
                weight: 3,
            },
        ),
    ];

    for algorithm in algorithms {
        let config = AdvancedPathSelectionConfig {
            algorithm,
            ..Default::default()
        };

        let selector = AdvancedPathSelector::new(config);
        selector.initialize_paths(&paths).unwrap();

        // Each algorithm should work without errors
        for _ in 0..10 {
            let result = selector.select_next_path();
            assert!(
                result.is_ok(),
                "Algorithm {algorithm:?} should work correctly"
            );
        }
    }
}
