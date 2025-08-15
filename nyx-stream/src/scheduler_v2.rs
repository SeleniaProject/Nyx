#![forbid(unsafe_code)]

//! Weighted Round Robin Path Scheduler for Nyx Protocol v1.0
//!
//! This module implements the Smooth Weighted Round-Robin (SWRR) scheduling algorithm
//! optimized for multipath data plane routing. Weights are calculated as inverse RTT
//! to prioritize faster network paths, implementing the v1.0 specification requirements.
//!
//! ## Algorithm
//!
//! The scheduler uses the Smooth WRR algorithm:
//! 1. For each selection cycle, increment current_weight[i] += original_weight[i]
//! 2. Select path with maximum current_weight
//! 3. Decrement selected path: current_weight[selected] -= total_weight
//!
//! This ensures smooth traffic distribution proportional to path weights over time.
//!
//! ## Weight Calculation
//!
//! ```text
//! Weight = SCALE / RTT_ms
//! where SCALE = 1000 (configurable)
//! ```
//!
//! Lower RTT yields higher weight, favoring faster paths.
//!
//! ## Example
//!
//! ```rust
//! use nyx_stream::WeightedRoundRobinScheduler;
//! use std::time::Duration;
//!
//! let mut scheduler = WeightedRoundRobinScheduler::new();
//!
//! // Add paths with RTT measurements
//! scheduler.update_path(1, Duration::from_millis(10)); // Fast path, high weight
//! scheduler.update_path(2, Duration::from_millis(50)); // Slower path, lower weight
//!
//! // Select paths according to weighted distribution
//! for _ in 0..100 {
//!     if let Some(path_id) = scheduler.select_path() {
//!         println!("Selected PathID: {}", path_id);
//!     }
//! }
//! ```
//!
//! ## Thread Safety
//!
//! The scheduler is NOT thread-safe by design for performance. Wrap in `Mutex`/`RwLock`
//! for concurrent access or use one instance per thread.
//!
//! ## References
//!
//! - Nginx Smooth WRR: https://github.com/nginx/nginx/commit/52327e0627f49dbda1e8db695e63a4b0af4448b1
//! - Nyx Protocol v1.0 Specification §4.2: Multipath Extension

use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, trace, warn};

use nyx_core::types::{is_valid_user_path_id, PathId};

/// Default weight scale factor for RTT to weight conversion
pub const DEFAULT_WEIGHT_SCALE: f64 = 1000.0;

/// Minimum weight value to prevent zero-weight paths
pub const MIN_WEIGHT: u32 = 1;

/// Maximum weight value to prevent overflow
pub const MAX_WEIGHT: u32 = 10_000;

/// Default RTT for new paths (milliseconds)
pub const DEFAULT_RTT_MS: f64 = 100.0;

/// Per-path state for the WRR algorithm
#[derive(Debug, Clone, PartialEq)]
struct PathState {
    /// Original weight assigned to this path
    original_weight: u32,
    /// Current weight counter for WRR selection
    current_weight: i64,
    /// Last measured RTT for this path
    rtt: Duration,
    /// Whether this path is healthy/active
    is_active: bool,
    /// Number of times this path has been selected
    selection_count: u64,
    /// Last update timestamp
    last_updated: std::time::Instant,
}

impl PathState {
    fn new(weight: u32, rtt: Duration) -> Self {
        Self {
            original_weight: weight,
            current_weight: 0,
            rtt,
            is_active: true,
            selection_count: 0,
            last_updated: std::time::Instant::now(),
        }
    }

    /// Update RTT and recalculate weight
    fn update_rtt(&mut self, new_rtt: Duration, weight_scale: f64) {
        self.rtt = new_rtt;
        self.original_weight = calculate_weight_from_rtt(new_rtt, weight_scale);
        self.last_updated = std::time::Instant::now();
    }

    /// Mark path as selected
    fn mark_selected(&mut self) {
        self.selection_count += 1;
        self.last_updated = std::time::Instant::now();
    }
}

/// Calculate weight from RTT using inverse relationship
fn calculate_weight_from_rtt(rtt: Duration, scale: f64) -> u32 {
    let rtt_ms = rtt.as_millis() as f64;
    if rtt_ms <= 0.0 {
        return MAX_WEIGHT;
    }

    let weight = (scale / rtt_ms).round() as u32;
    weight.clamp(MIN_WEIGHT, MAX_WEIGHT)
}

/// Calculate weight from RTT and loss rate using inverse RTT scaled by (1 - loss_rate).
/// Loss rate is clamped to [0.0, 0.99] to avoid zeroing weights completely.
fn calculate_weight_from_quality(rtt: Duration, loss_rate: f64, scale: f64) -> u32 {
    let base = calculate_weight_from_rtt(rtt, scale) as f64;
    let loss = loss_rate.clamp(0.0, 0.99);
    let adjusted = base * (1.0 - loss);
    adjusted.round() as u32
}

/// Weighted Round Robin scheduler for multipath routing
///
/// This scheduler implements Smooth WRR algorithm with weights calculated as
/// inverse RTT to prioritize faster network paths. It maintains fairness
/// while respecting path performance characteristics.
#[derive(Debug)]
pub struct WeightedRoundRobinScheduler {
    /// Per-path state information
    paths: HashMap<PathId, PathState>,
    /// Sum of all active path weights
    total_weight: u64,
    /// Weight scale factor for RTT conversion
    weight_scale: f64,
    /// Last selected path (for debugging/monitoring)
    last_selected: Option<PathId>,
    /// Total number of path selections made
    total_selections: u64,
    /// Creation timestamp
    created_at: std::time::Instant,
}

impl WeightedRoundRobinScheduler {
    /// Create a new WRR scheduler with default configuration
    pub fn new() -> Self {
        Self {
            paths: HashMap::new(),
            total_weight: 0,
            weight_scale: DEFAULT_WEIGHT_SCALE,
            last_selected: None,
            total_selections: 0,
            created_at: std::time::Instant::now(),
        }
    }

    /// Create a new WRR scheduler with custom weight scale
    pub fn with_weight_scale(weight_scale: f64) -> Self {
        Self {
            paths: HashMap::new(),
            total_weight: 0,
            weight_scale,
            last_selected: None,
            total_selections: 0,
            created_at: std::time::Instant::now(),
        }
    }

    /// Add or update a path with RTT measurement
    ///
    /// # Arguments
    ///
    /// * `path_id` - PathID (must be in valid user range 1-239)
    /// * `rtt` - Round-trip time measurement for weight calculation
    ///
    /// # Returns
    ///
    /// `Ok(())` if path was added/updated successfully
    /// `Err(String)` if PathID is invalid
    pub fn update_path(&mut self, path_id: PathId, rtt: Duration) -> Result<(), String> {
        if !is_valid_user_path_id(path_id) {
            return Err(format!("PathID {} is not in valid user range", path_id));
        }

        let new_weight = calculate_weight_from_rtt(rtt, self.weight_scale);

        match self.paths.get_mut(&path_id) {
            Some(state) => {
                // Update existing path
                let old_weight = state.original_weight as u64;
                state.update_rtt(rtt, self.weight_scale);

                // Adjust total weight
                self.total_weight = self.total_weight.saturating_sub(old_weight);
                self.total_weight = self.total_weight.saturating_add(new_weight as u64);

                debug!(
                    path_id = path_id,
                    old_weight = old_weight,
                    new_weight = new_weight,
                    rtt_ms = rtt.as_millis(),
                    total_weight = self.total_weight,
                    "Updated path RTT and weight"
                );
            }
            None => {
                // Add new path
                let state = PathState::new(new_weight, rtt);
                self.paths.insert(path_id, state);
                self.total_weight = self.total_weight.saturating_add(new_weight as u64);

                debug!(
                    path_id = path_id,
                    weight = new_weight,
                    rtt_ms = rtt.as_millis(),
                    total_paths = self.paths.len(),
                    total_weight = self.total_weight,
                    "Added new path to WRR scheduler"
                );
            }
        }

        Ok(())
    }

    /// Add or update a path with RTT and loss rate measurement (enhanced quality model)
    pub fn update_path_with_quality(
        &mut self,
        path_id: PathId,
        rtt: Duration,
        loss_rate: f64,
    ) -> Result<(), String> {
        if !is_valid_user_path_id(path_id) {
            return Err(format!("PathID {} is not in valid user range", path_id));
        }

        let new_weight = calculate_weight_from_quality(rtt, loss_rate, self.weight_scale);

        match self.paths.get_mut(&path_id) {
            Some(state) => {
                let old_weight = state.original_weight as u64;
                state.update_rtt(rtt, self.weight_scale);
                // Overwrite recalculated weight with quality-adjusted value
                state.original_weight = new_weight;
                self.total_weight = self.total_weight.saturating_sub(old_weight);
                self.total_weight = self.total_weight.saturating_add(new_weight as u64);
                debug!(
                    path_id = path_id,
                    old_weight = old_weight,
                    new_weight = new_weight,
                    rtt_ms = rtt.as_millis(),
                    loss_rate = loss_rate,
                    total_weight = self.total_weight,
                    "Updated path RTT+loss and weight"
                );
            }
            None => {
                let mut state = PathState::new(new_weight, rtt);
                // Ensure state weight equals quality-adjusted
                state.original_weight = new_weight;
                self.paths.insert(path_id, state);
                self.total_weight = self.total_weight.saturating_add(new_weight as u64);
                debug!(
                    path_id = path_id,
                    weight = new_weight,
                    rtt_ms = rtt.as_millis(),
                    loss_rate = loss_rate,
                    total_paths = self.paths.len(),
                    total_weight = self.total_weight,
                    "Added new path (quality) to WRR scheduler"
                );
            }
        }
        Ok(())
    }

    /// Remove a path from the scheduler
    ///
    /// # Arguments
    ///
    /// * `path_id` - PathID to remove
    ///
    /// # Returns
    ///
    /// `true` if path was removed, `false` if not found
    pub fn remove_path(&mut self, path_id: PathId) -> bool {
        if let Some(state) = self.paths.remove(&path_id) {
            self.total_weight = self
                .total_weight
                .saturating_sub(state.original_weight as u64);

            debug!(
                path_id = path_id,
                removed_weight = state.original_weight,
                remaining_paths = self.paths.len(),
                total_weight = self.total_weight,
                "Removed path from WRR scheduler"
            );
            true
        } else {
            false
        }
    }

    /// Select next path using Smooth Weighted Round Robin algorithm
    ///
    /// This implements the core SWRR algorithm for fair path selection
    /// based on inverse RTT weighting.
    ///
    /// # Returns
    ///
    /// `Some(PathId)` if a path is available for selection
    /// `None` if no active paths are available
    pub fn select_path(&mut self) -> Option<PathId> {
        if self.paths.is_empty() {
            return None;
        }

        // Step 1: Increment all current weights by their original weights
        for state in self.paths.values_mut() {
            if state.is_active {
                state.current_weight = state
                    .current_weight
                    .saturating_add(state.original_weight as i64);
            }
        }

        // Step 2: Find path with maximum current weight
        let selected_path = self
            .paths
            .iter()
            .filter(|(_, state)| state.is_active)
            .max_by_key(|(_, state)| state.current_weight)
            .map(|(&path_id, _)| path_id);

        // Step 3: Adjust selected path's current weight
        if let Some(path_id) = selected_path {
            if let Some(state) = self.paths.get_mut(&path_id) {
                state.current_weight = state
                    .current_weight
                    .saturating_sub(self.total_weight as i64);
                state.mark_selected();

                self.last_selected = Some(path_id);
                self.total_selections += 1;

                trace!(
                    selected_path = path_id,
                    current_weight = state.current_weight,
                    original_weight = state.original_weight,
                    rtt_ms = state.rtt.as_millis(),
                    total_selections = self.total_selections,
                    "Selected path via WRR algorithm"
                );
            }
        }

        selected_path
    }

    /// Activate or deactivate a path
    ///
    /// Inactive paths are not considered for selection but remain
    /// in the scheduler for quick reactivation.
    pub fn set_path_active(&mut self, path_id: PathId, active: bool) -> bool {
        if let Some(state) = self.paths.get_mut(&path_id) {
            let old_active = state.is_active;
            state.is_active = active;

            debug!(
                path_id = path_id,
                old_active = old_active,
                new_active = active,
                "Changed path active state"
            );

            true
        } else {
            false
        }
    }

    /// Get current scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        let active_paths = self.paths.values().filter(|s| s.is_active).count();
        let inactive_paths = self.paths.len() - active_paths;

        SchedulerStats {
            total_paths: self.paths.len(),
            active_paths,
            inactive_paths,
            total_weight: self.total_weight,
            total_selections: self.total_selections,
            last_selected: self.last_selected,
            uptime: self.created_at.elapsed(),
        }
    }

    /// Get detailed information about all paths
    pub fn path_info(&self) -> Vec<PathInfo> {
        self.paths
            .iter()
            .map(|(&path_id, state)| PathInfo {
                path_id,
                weight: state.original_weight,
                current_weight: state.current_weight,
                rtt: state.rtt,
                is_active: state.is_active,
                selection_count: state.selection_count,
                last_updated: state.last_updated,
            })
            .collect()
    }

    /// Reset all current weights (useful for testing or rebalancing)
    pub fn reset_weights(&mut self) {
        for state in self.paths.values_mut() {
            state.current_weight = 0;
        }

        debug!(
            total_paths = self.paths.len(),
            "Reset all current weights to zero"
        );
    }

    /// Check if scheduler has any active paths
    pub fn has_active_paths(&self) -> bool {
        self.paths.values().any(|s| s.is_active)
    }

    /// Get number of active paths
    pub fn active_path_count(&self) -> usize {
        self.paths.values().filter(|s| s.is_active).count()
    }

    /// Update weight scale factor (affects all future weight calculations)
    pub fn set_weight_scale(&mut self, scale: f64) {
        if scale > 0.0 {
            self.weight_scale = scale;
            debug!(new_scale = scale, "Updated weight scale factor");
        } else {
            warn!(invalid_scale = scale, "Ignored invalid weight scale factor");
        }
    }
}

impl Default for WeightedRoundRobinScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about scheduler performance
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    /// Total number of paths (active + inactive)
    pub total_paths: usize,
    /// Number of active paths
    pub active_paths: usize,
    /// Number of inactive paths  
    pub inactive_paths: usize,
    /// Sum of all active path weights
    pub total_weight: u64,
    /// Total number of path selections made
    pub total_selections: u64,
    /// Last selected path ID
    pub last_selected: Option<PathId>,
    /// Scheduler uptime
    pub uptime: Duration,
}

/// Detailed information about a specific path
#[derive(Debug, Clone)]
pub struct PathInfo {
    /// Path identifier
    pub path_id: PathId,
    /// Original weight for this path
    pub weight: u32,
    /// Current weight counter
    pub current_weight: i64,
    /// Last measured RTT
    pub rtt: Duration,
    /// Whether path is active
    pub is_active: bool,
    /// Number of times selected
    pub selection_count: u64,
    /// Last update timestamp
    pub last_updated: std::time::Instant,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = WeightedRoundRobinScheduler::new();
        assert_eq!(scheduler.paths.len(), 0);
        assert_eq!(scheduler.total_weight, 0);
        assert!(scheduler.last_selected.is_none());
    }

    #[test]
    fn test_path_addition() {
        let mut scheduler = WeightedRoundRobinScheduler::new();

        // Add valid path
        let result = scheduler.update_path(1, Duration::from_millis(50));
        assert!(result.is_ok());
        assert_eq!(scheduler.paths.len(), 1);
        assert!(scheduler.total_weight > 0);

        // Try to add invalid path
        let result = scheduler.update_path(0, Duration::from_millis(50)); // Control path, invalid
        assert!(result.is_err());
        assert_eq!(scheduler.paths.len(), 1); // Should remain unchanged
    }

    #[test]
    fn test_path_removal() {
        let mut scheduler = WeightedRoundRobinScheduler::new();

        scheduler.update_path(1, Duration::from_millis(50)).unwrap();
        scheduler
            .update_path(2, Duration::from_millis(100))
            .unwrap();
        assert_eq!(scheduler.paths.len(), 2);

        // Remove existing path
        assert!(scheduler.remove_path(1));
        assert_eq!(scheduler.paths.len(), 1);

        // Try to remove non-existent path
        assert!(!scheduler.remove_path(99));
        assert_eq!(scheduler.paths.len(), 1);
    }

    #[test]
    fn test_weight_calculation() {
        let weight1 = calculate_weight_from_rtt(Duration::from_millis(10), DEFAULT_WEIGHT_SCALE);
        let weight2 = calculate_weight_from_rtt(Duration::from_millis(100), DEFAULT_WEIGHT_SCALE);

        // Lower RTT should yield higher weight
        assert!(weight1 > weight2);
        assert_eq!(weight1, 100); // 1000/10
        assert_eq!(weight2, 10); // 1000/100
    }

    #[test]
    fn test_smooth_wrr_distribution() {
        let mut scheduler = WeightedRoundRobinScheduler::new();

        // Add paths with different RTTs
        scheduler.update_path(1, Duration::from_millis(10)).unwrap(); // Weight: 100
        scheduler.update_path(2, Duration::from_millis(20)).unwrap(); // Weight: 50
        scheduler.update_path(3, Duration::from_millis(50)).unwrap(); // Weight: 20

        let mut counts = HashMap::new();
        const ITERATIONS: usize = 1700; // Multiple of total weight (170)

        for _ in 0..ITERATIONS {
            if let Some(path_id) = scheduler.select_path() {
                *counts.entry(path_id).or_insert(0) += 1;
            }
        }

        // Verify distribution roughly matches weight ratios
        let total_selections: u32 = counts.values().sum();
        assert_eq!(total_selections, ITERATIONS as u32);

        // Path 1 (weight 100) should get ~59% (100/170)
        let path1_ratio = counts[&1] as f64 / total_selections as f64;
        assert!(
            path1_ratio > 0.55 && path1_ratio < 0.65,
            "Path 1 ratio: {}",
            path1_ratio
        );

        // Path 2 (weight 50) should get ~29% (50/170)
        let path2_ratio = counts[&2] as f64 / total_selections as f64;
        assert!(
            path2_ratio > 0.25 && path2_ratio < 0.35,
            "Path 2 ratio: {}",
            path2_ratio
        );

        // Path 3 (weight 20) should get ~12% (20/170)
        let path3_ratio = counts[&3] as f64 / total_selections as f64;
        assert!(
            path3_ratio > 0.08 && path3_ratio < 0.16,
            "Path 3 ratio: {}",
            path3_ratio
        );
    }

    #[test]
    fn test_no_paths_available() {
        let mut scheduler = WeightedRoundRobinScheduler::new();
        assert!(scheduler.select_path().is_none());
    }

    #[test]
    fn test_path_activation() {
        let mut scheduler = WeightedRoundRobinScheduler::new();
        scheduler.update_path(1, Duration::from_millis(50)).unwrap();
        scheduler.update_path(2, Duration::from_millis(50)).unwrap();

        // Deactivate path 2
        scheduler.set_path_active(2, false);

        // Only path 1 should be selected
        for _ in 0..10 {
            assert_eq!(scheduler.select_path(), Some(1));
        }

        // Reactivate path 2
        scheduler.set_path_active(2, true);

        // Now both paths should be selectable
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
            "Both paths should be selected"
        );
    }

    #[test]
    fn test_stats() {
        let mut scheduler = WeightedRoundRobinScheduler::new();
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
        assert!(stats.last_selected.is_none());
    }

    #[test]
    fn test_weight_scale_adjustment() {
        let mut scheduler = WeightedRoundRobinScheduler::with_weight_scale(2000.0);
        scheduler.update_path(1, Duration::from_millis(10)).unwrap();

        // With scale 2000, weight should be 200 (2000/10)
        let path_info = scheduler.path_info();
        assert_eq!(path_info[0].weight, 200);

        // Change scale and update path
        scheduler.set_weight_scale(500.0);
        scheduler.update_path(1, Duration::from_millis(10)).unwrap();

        // Now weight should be 50 (500/10)
        let path_info = scheduler.path_info();
        assert_eq!(path_info[0].weight, 50);
    }

    #[test]
    fn test_loss_weighting_affects_distribution() {
        let mut scheduler = WeightedRoundRobinScheduler::new();
        // Same RTT, different loss: path1 low loss, path2 higher loss
        scheduler
            .update_path_with_quality(1, Duration::from_millis(50), 0.01)
            .unwrap();
        scheduler
            .update_path_with_quality(2, Duration::from_millis(50), 0.20)
            .unwrap();

        let mut c1 = 0u32;
        let mut c2 = 0u32;
        for _ in 0..1000 {
            match scheduler.select_path() {
                Some(1) => c1 += 1,
                Some(2) => c2 += 1,
                _ => {}
            }
        }
        // Low loss path should be preferred
        assert!(
            c1 > c2,
            "expected path1 selections > path2, got {} vs {}",
            c1,
            c2
        );
    }
}
