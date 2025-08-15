#![forbid(unsafe_code)]

//! Comprehensive integration tests for Weighted Round Robin Scheduler v2
//!
//! This test suite validates the complete WRR scheduler implementation
//! including PathID header integration, RTT-based weight calculation,
//! and multipath manager coordination.

use nyx_core::types::PathId;
use nyx_stream::scheduler_v2::{PathInfo, SchedulerStats, WeightedRoundRobinScheduler};
use std::collections::HashMap;
use std::time::Duration;

/// @spec 2. Multipath Data Plane
#[test]
fn test_scheduler_creation_and_basic_operations() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Initial state
    assert_eq!(scheduler.active_path_count(), 0);
    assert!(!scheduler.has_active_paths());
    assert!(scheduler.select_path().is_none());

    let stats = scheduler.stats();
    assert_eq!(stats.total_paths, 0);
    assert_eq!(stats.active_paths, 0);
    assert_eq!(stats.total_selections, 0);
    assert!(stats.last_selected.is_none());
}

/// @spec 2. Multipath Data Plane
#[test]
fn test_path_management() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Add valid paths
    assert!(scheduler.update_path(1, Duration::from_millis(50)).is_ok());
    assert!(scheduler.update_path(2, Duration::from_millis(100)).is_ok());
    assert_eq!(scheduler.active_path_count(), 2);

    // Try invalid path
    assert!(scheduler.update_path(0, Duration::from_millis(50)).is_err()); // Control path
    assert!(scheduler
        .update_path(250, Duration::from_millis(50))
        .is_err()); // System path
    assert_eq!(scheduler.active_path_count(), 2); // Should remain unchanged

    // Remove paths
    assert!(scheduler.remove_path(1));
    assert_eq!(scheduler.active_path_count(), 1);
    assert!(!scheduler.remove_path(99)); // Non-existent path

    // Clear all paths
    scheduler.remove_path(2);
    assert_eq!(scheduler.active_path_count(), 0);
    assert!(!scheduler.has_active_paths());
}

/// @spec 2. Multipath Data Plane
#[test]
fn test_weight_calculation_from_rtt() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Add paths with different RTTs
    scheduler.update_path(1, Duration::from_millis(10)).unwrap(); // Weight: 100
    scheduler.update_path(2, Duration::from_millis(20)).unwrap(); // Weight: 50
    scheduler
        .update_path(3, Duration::from_millis(100))
        .unwrap(); // Weight: 10

    let path_info = scheduler.path_info();
    let path1 = path_info.iter().find(|p| p.path_id == 1).unwrap();
    let path2 = path_info.iter().find(|p| p.path_id == 2).unwrap();
    let path3 = path_info.iter().find(|p| p.path_id == 3).unwrap();

    // Lower RTT should yield higher weight
    assert!(path1.weight > path2.weight);
    assert!(path2.weight > path3.weight);

    // Verify specific weights (1000/RTT_ms)
    assert_eq!(path1.weight, 100); // 1000/10
    assert_eq!(path2.weight, 50); // 1000/20
    assert_eq!(path3.weight, 10); // 1000/100
}

/// @spec 2. Multipath Data Plane
#[test]
fn test_smooth_wrr_distribution() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Add paths with weights 3:2:1 ratio
    scheduler.update_path(1, Duration::from_millis(10)).unwrap(); // Weight: 100
    scheduler.update_path(2, Duration::from_millis(20)).unwrap(); // Weight: 50
    scheduler
        .update_path(3, Duration::from_millis(100))
        .unwrap(); // Weight: 10

    let mut counts = HashMap::new();
    const ITERATIONS: usize = 1600; // Multiple of total weight (160)

    for _ in 0..ITERATIONS {
        if let Some(path_id) = scheduler.select_path() {
            *counts.entry(path_id).or_insert(0) += 1;
        }
    }

    let total_selections: u32 = counts.values().sum();
    assert_eq!(total_selections, ITERATIONS as u32);

    // Verify distribution matches weight ratios
    // Total weight: 100+50+10 = 160
    let path1_ratio = counts[&1] as f64 / total_selections as f64;
    let path2_ratio = counts[&2] as f64 / total_selections as f64;
    let path3_ratio = counts[&3] as f64 / total_selections as f64;

    // Expected ratios: 100/160=62.5%, 50/160=31.25%, 10/160=6.25%
    assert!(
        path1_ratio > 0.60 && path1_ratio < 0.65,
        "Path 1 ratio: {}",
        path1_ratio
    );
    assert!(
        path2_ratio > 0.29 && path2_ratio < 0.34,
        "Path 2 ratio: {}",
        path2_ratio
    );
    assert!(
        path3_ratio > 0.04 && path3_ratio < 0.08,
        "Path 3 ratio: {}",
        path3_ratio
    );

    // Verify sum is approximately 1.0
    assert!((path1_ratio + path2_ratio + path3_ratio - 1.0).abs() < 0.01);
}

/// @spec 2. Multipath Data Plane
#[test]
fn test_path_activation_deactivation() {
    let mut scheduler = WeightedRoundRobinScheduler::new();
    scheduler.update_path(1, Duration::from_millis(50)).unwrap();
    scheduler.update_path(2, Duration::from_millis(50)).unwrap();

    // Both paths active initially
    let mut path1_selected = false;
    let mut path2_selected = false;

    for _ in 0..20 {
        match scheduler.select_path() {
            Some(1) => path1_selected = true,
            Some(2) => path2_selected = true,
            _ => {}
        }
    }

    assert!(
        path1_selected && path2_selected,
        "Both paths should be selected initially"
    );

    // Deactivate path 2
    scheduler.set_path_active(2, false);

    // Only path 1 should be selected now
    for _ in 0..10 {
        assert_eq!(scheduler.select_path(), Some(1));
    }

    // Reactivate path 2
    scheduler.set_path_active(2, true);

    // Both paths should be selectable again
    path1_selected = false;
    path2_selected = false;

    for _ in 0..20 {
        match scheduler.select_path() {
            Some(1) => path1_selected = true,
            Some(2) => path2_selected = true,
            _ => {}
        }
    }

    assert!(
        path1_selected && path2_selected,
        "Both paths should be selected after reactivation"
    );
}

#[test]
fn test_rtt_updates() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Initial RTT
    scheduler
        .update_path(1, Duration::from_millis(100))
        .unwrap();
    let initial_weight = scheduler.path_info()[0].weight;
    assert_eq!(initial_weight, 10); // 1000/100

    // Update with better RTT
    scheduler.update_path(1, Duration::from_millis(50)).unwrap();
    let updated_weight = scheduler.path_info()[0].weight;
    assert_eq!(updated_weight, 20); // 1000/50

    // Weight should have increased with lower RTT
    assert!(updated_weight > initial_weight);

    // Update with worse RTT
    scheduler
        .update_path(1, Duration::from_millis(200))
        .unwrap();
    let final_weight = scheduler.path_info()[0].weight;
    assert_eq!(final_weight, 5); // 1000/200

    assert!(final_weight < updated_weight);
}

#[test]
fn test_scheduler_statistics() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Add paths
    scheduler.update_path(1, Duration::from_millis(50)).unwrap();
    scheduler
        .update_path(2, Duration::from_millis(100))
        .unwrap();
    scheduler.set_path_active(2, false);

    let stats = scheduler.stats();
    assert_eq!(stats.total_paths, 2);
    assert_eq!(stats.active_paths, 1);
    assert_eq!(stats.inactive_paths, 1);
    assert!(stats.total_weight > 0);
    assert_eq!(stats.total_selections, 0);

    // Make some selections
    for _ in 0..5 {
        scheduler.select_path();
    }

    let stats_after = scheduler.stats();
    assert_eq!(stats_after.total_selections, 5);
    assert_eq!(stats_after.last_selected, Some(1));
}

#[test]
fn test_path_info_details() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    let rtt1 = Duration::from_millis(25);
    let rtt2 = Duration::from_millis(75);

    scheduler.update_path(1, rtt1).unwrap();
    scheduler.update_path(2, rtt2).unwrap();

    let path_info = scheduler.path_info();
    assert_eq!(path_info.len(), 2);

    let path1_info = path_info.iter().find(|p| p.path_id == 1).unwrap();
    let path2_info = path_info.iter().find(|p| p.path_id == 2).unwrap();

    assert_eq!(path1_info.path_id, 1);
    assert_eq!(path1_info.rtt, rtt1);
    assert_eq!(path1_info.weight, 40); // 1000/25
    assert!(path1_info.is_active);
    assert_eq!(path1_info.selection_count, 0);

    assert_eq!(path2_info.path_id, 2);
    assert_eq!(path2_info.rtt, rtt2);
    assert_eq!(path2_info.weight, 13); // 1000/75 ≈ 13
    assert!(path2_info.is_active);
}

#[test]
fn test_weight_scale_adjustment() {
    let mut scheduler = WeightedRoundRobinScheduler::with_weight_scale(2000.0);

    scheduler.update_path(1, Duration::from_millis(10)).unwrap();
    let weight_high_scale = scheduler.path_info()[0].weight;
    assert_eq!(weight_high_scale, 200); // 2000/10

    // Change scale
    scheduler.set_weight_scale(500.0);
    scheduler.update_path(1, Duration::from_millis(10)).unwrap();
    let weight_low_scale = scheduler.path_info()[0].weight;
    assert_eq!(weight_low_scale, 50); // 500/10

    assert!(weight_high_scale > weight_low_scale);
}

#[test]
fn test_extreme_rtt_values() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Very low RTT
    scheduler.update_path(1, Duration::from_millis(1)).unwrap();
    let high_weight = scheduler.path_info()[0].weight;
    assert_eq!(high_weight, 1000); // 1000/1

    // Very high RTT
    scheduler
        .update_path(2, Duration::from_millis(5000))
        .unwrap();
    let low_weight = scheduler
        .path_info()
        .iter()
        .find(|p| p.path_id == 2)
        .unwrap()
        .weight;
    assert!(low_weight >= 1); // Should be clamped to minimum

    // Zero RTT (edge case)
    scheduler.update_path(3, Duration::from_millis(0)).unwrap();
    let zero_rtt_weight = scheduler
        .path_info()
        .iter()
        .find(|p| p.path_id == 3)
        .unwrap()
        .weight;
    assert_eq!(zero_rtt_weight, 10000); // Should get maximum weight
}

#[test]
fn test_weight_reset() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    scheduler.update_path(1, Duration::from_millis(50)).unwrap();
    scheduler
        .update_path(2, Duration::from_millis(100))
        .unwrap();

    // Make selections to change current weights
    for _ in 0..5 {
        scheduler.select_path();
    }

    // Current weights should be non-zero
    let info_before = scheduler.path_info();
    let has_non_zero_current = info_before.iter().any(|p| p.current_weight != 0);
    assert!(has_non_zero_current, "Should have non-zero current weights");

    // Reset weights
    scheduler.reset_weights();

    // All current weights should be zero
    let info_after = scheduler.path_info();
    for path in info_after {
        assert_eq!(
            path.current_weight, 0,
            "Current weight should be 0 after reset"
        );
    }
}

#[test]
fn test_concurrent_path_operations() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Simulate rapid path additions and removals
    for i in 1..=10 {
        scheduler
            .update_path(i, Duration::from_millis(i as u64 * 10))
            .unwrap();
    }

    assert_eq!(scheduler.active_path_count(), 10);

    // Remove every other path
    for i in (2..=10).step_by(2) {
        scheduler.remove_path(i);
    }

    assert_eq!(scheduler.active_path_count(), 5);

    // Remaining paths should still be selectable
    let mut selected_paths = std::collections::HashSet::new();
    for _ in 0..50 {
        if let Some(path_id) = scheduler.select_path() {
            selected_paths.insert(path_id);
        }
    }

    // Should have selected from remaining paths (1,3,5,7,9)
    let expected_paths: std::collections::HashSet<PathId> =
        [1, 3, 5, 7, 9].iter().copied().collect();
    assert_eq!(selected_paths, expected_paths);
}

#[test]
fn test_performance_characteristics() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Add many paths
    const NUM_PATHS: usize = 100;
    for i in 1..=NUM_PATHS {
        scheduler
            .update_path(i as PathId, Duration::from_millis(i as u64))
            .unwrap();
    }

    // Time path selection operations
    let start = std::time::Instant::now();
    const NUM_SELECTIONS: usize = 10000;
    for _ in 0..NUM_SELECTIONS {
        scheduler.select_path();
    }
    let duration = start.elapsed();

    // Should complete reasonably quickly (< 200ms for 10k selections)
    assert!(
        duration.as_millis() < 200,
        "Selection took too long: {:?}",
        duration
    );

    // Verify all selections were successful
    let stats = scheduler.stats();
    assert_eq!(stats.total_selections, NUM_SELECTIONS as u64);
}

#[test]
fn test_fairness_with_equal_weights() {
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Add paths with equal RTT (and thus equal weights)
    const RTT_MS: u64 = 50;
    const NUM_PATHS: u8 = 5;

    for i in 1..=NUM_PATHS {
        scheduler
            .update_path(i, Duration::from_millis(RTT_MS))
            .unwrap();
    }

    let mut counts = HashMap::new();
    const ITERATIONS: usize = 1000;

    for _ in 0..ITERATIONS {
        if let Some(path_id) = scheduler.select_path() {
            *counts.entry(path_id).or_insert(0) += 1;
        }
    }

    // With equal weights, distribution should be roughly equal
    let expected_per_path = ITERATIONS / NUM_PATHS as usize;
    let tolerance = expected_per_path / 5; // 20% tolerance

    for i in 1..=NUM_PATHS {
        let count = counts.get(&i).unwrap_or(&0);
        let diff = (*count as i32 - expected_per_path as i32).abs();
        assert!(
            diff <= tolerance as i32,
            "Path {} got {} selections, expected ~{} (tolerance: {})",
            i,
            count,
            expected_per_path,
            tolerance
        );
    }
}

#[test]
fn test_scheduler_lifecycle() {
    // Test complete scheduler lifecycle
    let mut scheduler = WeightedRoundRobinScheduler::new();

    // Phase 1: Empty state
    assert!(!scheduler.has_active_paths());
    assert!(scheduler.select_path().is_none());

    // Phase 2: Add paths
    scheduler.update_path(1, Duration::from_millis(50)).unwrap();
    scheduler
        .update_path(2, Duration::from_millis(100))
        .unwrap();
    assert!(scheduler.has_active_paths());
    assert_eq!(scheduler.active_path_count(), 2);

    // Phase 3: Use scheduler
    let mut selections = Vec::new();
    for _ in 0..10 {
        if let Some(path_id) = scheduler.select_path() {
            selections.push(path_id);
        }
    }
    assert_eq!(selections.len(), 10);

    // Phase 4: Update RTTs
    scheduler.update_path(1, Duration::from_millis(25)).unwrap(); // Improve path 1
    scheduler
        .update_path(2, Duration::from_millis(200))
        .unwrap(); // Worsen path 2

    // Path 1 should now be selected more frequently
    let mut path1_count = 0;
    for _ in 0..100 {
        if scheduler.select_path() == Some(1) {
            path1_count += 1;
        }
    }
    assert!(
        path1_count > 60,
        "Path 1 should be selected more often with better RTT"
    );

    // Phase 5: Remove paths
    scheduler.remove_path(1);
    assert_eq!(scheduler.active_path_count(), 1);

    // Phase 6: Clean up
    scheduler.remove_path(2);
    assert!(!scheduler.has_active_paths());
    assert!(scheduler.select_path().is_none());
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn test_thread_safety_simulation() {
        // Since WeightedRoundRobinScheduler is not thread-safe by design,
        // this test verifies that wrapping it in a Mutex works correctly
        // for multi-threaded access patterns.

        let scheduler = Arc::new(Mutex::new(WeightedRoundRobinScheduler::new()));

        // Setup phase: Add paths
        {
            let mut sched = scheduler.lock().unwrap();
            for i in 1..=5 {
                sched
                    .update_path(i, Duration::from_millis(i as u64 * 20))
                    .unwrap();
            }
        }

        let mut handles = vec![];
        let selections = Arc::new(Mutex::new(Vec::new()));

        // Simulate concurrent path selections
        for _ in 0..4 {
            let scheduler_clone = Arc::clone(&scheduler);
            let selections_clone = Arc::clone(&selections);

            let handle = thread::spawn(move || {
                for _ in 0..25 {
                    let path_id = {
                        let mut sched = scheduler_clone.lock().unwrap();
                        sched.select_path()
                    };

                    if let Some(id) = path_id {
                        let mut sels = selections_clone.lock().unwrap();
                        sels.push(id);
                    }

                    // Small delay to increase interleaving
                    thread::sleep(Duration::from_micros(10));
                }
            });

            handles.push(handle);
        }

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify results
        let final_selections = selections.lock().unwrap();
        assert_eq!(final_selections.len(), 100); // 4 threads × 25 selections

        // All selections should be from valid paths
        for &path_id in final_selections.iter() {
            assert!(path_id >= 1 && path_id <= 5);
        }

        // Should have used multiple paths
        let unique_paths: std::collections::HashSet<_> = final_selections.iter().collect();
        assert!(unique_paths.len() > 1, "Should have used multiple paths");
    }
}
