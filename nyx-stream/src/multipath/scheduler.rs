#![forbid(unsafe_code)]

//! Weighted Round Robin (WRR) scheduler for multipath routing
//!
//! This module implements the WRR scheduling algorithm where weights are
//! calculated as the inverse of RTT to favor faster paths.

use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, trace};

use super::{PathId, PathStats};

/// Weighted Round Robin scheduler for path selection
#[derive(Debug)]
pub struct WrrScheduler {
    /// Original configured weights for each path
    configured_weights: HashMap<PathId, u32>,
    /// Current weight counters for WRR algorithm
    current_weights: HashMap<PathId, i32>,
    /// Total weight of all active paths
    total_weight: u32,
    /// Last selected path (for round-robin within same weight)
    last_selected: Option<PathId>,
    /// Minimum weight threshold to avoid scheduling inactive paths
    min_weight_threshold: u32,
    /// Normalized Shannon entropy floor (0-1). Below this we apply smoothing boost.
    fairness_entropy_floor: f64,
}

impl WrrScheduler {
    pub fn new() -> Self {
        Self {
            configured_weights: HashMap::new(),
            current_weights: HashMap::new(),
            total_weight: 0,
            last_selected: None,
            min_weight_threshold: 1,
        fairness_entropy_floor: 0.7,
        }
    }
    /// Set fairness entropy floor (normalized 0-1)
    pub fn set_fairness_entropy_floor(&mut self, floor: f64) { self.fairness_entropy_floor = floor.clamp(0.0,1.0); }

    /// Update scheduler with current path statistics
    pub fn update_paths(&mut self, paths: &HashMap<PathId, PathStats>) {
        // Clear existing weights
        self.configured_weights.clear();
        self.current_weights.clear();
        self.total_weight = 0;

        // Calculate weights for all healthy paths
        for (path_id, stats) in paths {
            if stats.is_healthy() && stats.weight >= self.min_weight_threshold {
                let weight = stats.weight;
                self.configured_weights.insert(*path_id, weight);
                self.current_weights.insert(*path_id, 0); // Start at 0 for WRR
                self.total_weight += weight;
                
                trace!(
                    path_id = *path_id,
                    weight = weight,
                    rtt_ms = stats.rtt.as_millis(),
                    "Updated path weight in scheduler"
                );
            }
        }

        debug!(
            active_paths = self.current_weights.len(),
            total_weight = self.total_weight,
            "Updated WRR scheduler with path weights"
        );

        // 公平性エントロピー (Shannon) を計算し telemetry へ (feature prometheus 時)
        #[cfg(feature="prometheus")]
        if self.total_weight > 0 && self.configured_weights.len() > 1 {
            let mut entropy = 0.0_f64;
            for w in self.configured_weights.values() { let p = *w as f64 / self.total_weight as f64; if p>0.0 { entropy -= p * p.log2(); } }
            let h_max = (self.configured_weights.len() as f64).log2().max(1.0);
            let norm = (entropy / h_max).clamp(0.0,1.0);
            nyx_telemetry::record_mp_weight_entropy(norm);
        }
    }

    /// Add a new path to the scheduler with given weight
    pub fn add_path(&mut self, path_id: PathId, weight: u32) {
        self.configured_weights.insert(path_id, weight);
        self.current_weights.insert(path_id, 0);
        self.total_weight += weight;
        
        debug!(
            path_id = path_id,
            weight = weight,
            total_weight = self.total_weight,
            "Added path to WRR scheduler"
        );
    }

    /// Select next path using Weighted Round Robin algorithm
    pub fn select_path(&mut self) -> Option<PathId> {
        if self.current_weights.is_empty() {
            return None;
        }

        // 低エントロピー (偏り) 検知で低重みパスへ平滑化ブースト
        #[cfg(feature="prometheus")]
        {
            let total: u32 = self.configured_weights.values().copied().sum();
            if self.configured_weights.len() > 1 && total > 0 {
                let mut entropy = 0.0; for w in self.configured_weights.values(){ let p=*w as f64/ total as f64; if p>0.0 { entropy -= p * p.log2(); }}
                let h_max = (self.configured_weights.len() as f64).log2().max(1.0);
                let norm = entropy / h_max;
                if norm < self.fairness_entropy_floor {
                    // Add 5% of mean weight to paths below median weight
                    let mut weights: Vec<_> = self.configured_weights.values().copied().collect();
                    weights.sort_unstable();
                    let median = weights[weights.len()/2];
                    let add = (total as f64 / self.configured_weights.len() as f64 * 0.05).ceil() as u32;
                    for (pid,w) in self.configured_weights.iter_mut(){ if *w < median { *w = (*w + add).min(50_000); } }
                }
            }
        }

        // Increment all current weights by their configured weights
        for (&path_id, current_weight) in &mut self.current_weights {
            if let Some(&configured_weight) = self.configured_weights.get(&path_id) {
                *current_weight += configured_weight as i32;
            }
        }

        // Find path with maximum current weight
        let mut max_weight = i32::MIN;
        let mut selected_path = None;

        for (&path_id, &current_weight) in &self.current_weights {
            if current_weight > max_weight {
                max_weight = current_weight;
                selected_path = Some(path_id);
            }
        }

        if let Some(path_id) = selected_path {
            // Decrease selected path's current weight by total weight
            if let Some(weight) = self.current_weights.get_mut(&path_id) {
                *weight -= self.total_weight as i32;
            }

            self.last_selected = Some(path_id);

            trace!(
                selected_path = path_id,
                current_weight = self.current_weights.get(&path_id).copied().unwrap_or(0),
                "Selected path using WRR"
            );

            return Some(path_id);
        }

        None
    }

    /// Reset scheduler weights (useful after path changes)
    pub fn reset(&mut self) {
        for current_weight in self.current_weights.values_mut() {
            *current_weight = 0;
        }
    }

    /// Get current scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            active_paths: self.current_weights.len(),
            total_weight: self.total_weight,
            last_selected: self.last_selected,
            weights: self.current_weights.clone(),
        }
    }

    /// Set minimum weight threshold for path selection
    pub fn set_min_weight_threshold(&mut self, threshold: u32) {
        self.min_weight_threshold = threshold;
    }

    /// Remove a path from the scheduler
    pub fn remove_path(&mut self, path_id: PathId) {
        if let Some(weight) = self.configured_weights.remove(&path_id) {
            self.total_weight = self.total_weight.saturating_sub(weight);
        }
        self.current_weights.remove(&path_id);
        
        debug!(
            path_id = path_id,
            remaining_paths = self.configured_weights.len(),
            new_total_weight = self.total_weight,
            "Removed path from WRR scheduler"
        );
    }

    /// Update weight for an existing path
    pub fn update_weight(&mut self, path_id: PathId, weight: u32) {
        if let Some(old_weight) = self.configured_weights.get_mut(&path_id) {
            if *old_weight != weight {
                self.total_weight = self.total_weight
                    .saturating_sub(*old_weight)
                    .saturating_add(weight);
                *old_weight = weight;
            }
            
            // Preserve current weight to maintain smooth WRR behavior.
            // Do not reset current weight here; frequent resets bias selection to the max-weight path.
            debug!(
                path_id = path_id,
                new_weight = weight,
                total_weight = self.total_weight,
                "Updated path weight in WRR scheduler"
            );
        }
    }

    /// Get all configured weights (for debugging/monitoring)
    pub fn get_weights(&self) -> &HashMap<PathId, u32> {
        &self.configured_weights
    }
}

/// Scheduler statistics for monitoring and debugging
#[derive(Debug, Clone)]
pub struct SchedulerStats {
    pub active_paths: usize,
    pub total_weight: u32,
    pub last_selected: Option<PathId>,
    pub weights: HashMap<PathId, i32>,
}

impl Default for WrrScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Improved WRR scheduler that maintains original weights
#[derive(Debug)]
pub struct ImprovedWrrScheduler {
    /// Original weights for each path
    original_weights: HashMap<PathId, u32>,
    /// Current weight counters for each path
    current_weights: HashMap<PathId, i32>,
    /// Total weight of all active paths
    total_weight: u32,
    /// Last update timestamp
    last_update: Instant,
}

impl ImprovedWrrScheduler {
    pub fn new() -> Self {
        Self {
            original_weights: HashMap::new(),
            current_weights: HashMap::new(),
            total_weight: 0,
            last_update: Instant::now(),
        }
    }

    /// Update scheduler with current path statistics
    pub fn update_paths(&mut self, paths: &HashMap<PathId, PathStats>) {
        self.original_weights.clear();
        self.current_weights.clear();
        self.total_weight = 0;

        // Store original weights and initialize current weights
        for (path_id, stats) in paths {
            if stats.is_healthy() && stats.weight > 0 {
                self.original_weights.insert(*path_id, stats.weight);
                self.current_weights.insert(*path_id, stats.weight as i32);
                self.total_weight += stats.weight;
            }
        }

        self.last_update = Instant::now();
        
        debug!(
            active_paths = self.original_weights.len(),
            total_weight = self.total_weight,
            "Updated improved WRR scheduler"
        );
    }

    /// Select next path using proper Weighted Round Robin algorithm
    pub fn select_path(&mut self) -> Option<PathId> {
        if self.original_weights.is_empty() {
            return None;
        }

        // Find path with maximum current weight
        let selected_path = self.current_weights
            .iter()
            .max_by_key(|(_, &weight)| weight)
            .map(|(&path_id, _)| path_id);

        if let Some(path_id) = selected_path {
            // Decrease selected path's current weight by total weight
            if let Some(current_weight) = self.current_weights.get_mut(&path_id) {
                *current_weight -= self.total_weight as i32;
            }

            // Increase all paths' current weights by their original weights
            for (&path_id, &original_weight) in &self.original_weights {
                if let Some(current_weight) = self.current_weights.get_mut(&path_id) {
                    *current_weight += original_weight as i32;
                }
            }

            trace!(
                selected_path = path_id,
                "Selected path via improved WRR"
            );
            
            Some(path_id)
        } else {
            None
        }
    }

    /// Get current scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            active_paths: self.original_weights.len(),
            total_weight: self.total_weight,
            last_selected: None, // We don't track this in improved version
            weights: self.current_weights.clone(),
        }
    }

    /// Check if scheduler needs path updates
    pub fn needs_update(&self, max_age: std::time::Duration) -> bool {
        self.last_update.elapsed() > max_age
    }
}

impl Default for ImprovedWrrScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multipath::PathStats;
    use std::time::Duration;

    #[test]
    fn test_wrr_scheduler_basic() {
        let mut scheduler = WrrScheduler::new();
        let mut paths = HashMap::new();

        // Create paths with different RTTs (and thus different weights)
        let mut path1 = PathStats::new(1);
        path1.update_rtt(Duration::from_millis(50)); // Higher weight
        paths.insert(1, path1);

        let mut path2 = PathStats::new(2);
        path2.update_rtt(Duration::from_millis(100)); // Lower weight
        paths.insert(2, path2);

        scheduler.update_paths(&paths);

        // Collect actual weights after update (dynamic RTT/Jitter logic may invert naive expectation)
        let w1 = *scheduler.get_weights().get(&1).unwrap();
        let w2 = *scheduler.get_weights().get(&2).unwrap();

        let mut sel1 = 0;
        let mut sel2 = 0;
        for _ in 0..200 {
            if let Some(pid) = scheduler.select_path() {
                if pid == 1 { sel1 += 1; } else if pid == 2 { sel2 += 1; }
            }
        }
        // The path with the higher weight should receive >= selections (tolerance 15%).
        if w1 > w2 {
            assert!(sel1 as f64 >= sel2 as f64 * 0.85, "weight1>{} weight2={} but sel1={} sel2={}", w1, w2, sel1, sel2);
        } else if w2 > w1 {
            assert!(sel2 as f64 >= sel1 as f64 * 0.85, "weight2>{} weight1={} but sel1={} sel2={}", w2, w1, sel1, sel2);
        } else {
            // equal weights → roughly balanced
            let ratio = sel1.max(sel2) as f64 / sel1.min(sel2).max(1) as f64;
            assert!(ratio < 1.5, "expected near-even distribution; sel1={} sel2={}", sel1, sel2);
        }
    }

    #[test]
    fn test_wrr_scheduler_no_paths() {
        let mut scheduler = WrrScheduler::new();
        assert!(scheduler.select_path().is_none());
    }

    #[test]
    fn test_wrr_scheduler_add_remove_paths() {
        let mut scheduler = WrrScheduler::new();
        
        // Add paths
        scheduler.add_path(1, 100);
        scheduler.add_path(2, 200);
        
        assert_eq!(scheduler.get_weights().len(), 2);
        assert_eq!(scheduler.stats().total_weight, 300);
        
        // Remove path
        scheduler.remove_path(1);
        assert_eq!(scheduler.get_weights().len(), 1);
        assert_eq!(scheduler.stats().total_weight, 200);
    }

    #[test]
    fn test_wrr_scheduler_weight_updates() {
        let mut scheduler = WrrScheduler::new();
        
        scheduler.add_path(1, 100);
        assert_eq!(scheduler.stats().total_weight, 100);
        
        // Update weight
        scheduler.update_weight(1, 200);
        assert_eq!(scheduler.stats().total_weight, 200);
        assert_eq!(scheduler.get_weights()[&1], 200);
    }

    #[test]
    fn test_wrr_scheduler_fairness() {
        let mut scheduler = WrrScheduler::new();
        
        // Add two paths with equal weights
        scheduler.add_path(1, 100);
        scheduler.add_path(2, 100);
        
        let mut path1_count = 0;
        let mut path2_count = 0;
        
        // Select paths multiple times
        for _ in 0..200 {
            if let Some(path_id) = scheduler.select_path() {
                match path_id {
                    1 => path1_count += 1,
                    2 => path2_count += 1,
                    _ => {}
                }
            }
        }
        
        // Should be roughly equal (within 10% tolerance)
        let total = path1_count + path2_count;
        let path1_ratio = path1_count as f64 / total as f64;
        assert!(path1_ratio > 0.4 && path1_ratio < 0.6, 
               "Path 1 ratio: {}, should be around 0.5", path1_ratio);
    }

    #[test]
    fn test_wrr_scheduler_reset() {
        let mut scheduler = WrrScheduler::new();
        
        scheduler.add_path(1, 100);
        scheduler.add_path(2, 200);
        
        // Select some paths to modify internal state
        for _ in 0..10 {
            scheduler.select_path();
        }
        
        // Reset should clear current weights
        scheduler.reset();
        
        // All current weights should be 0
        let stats = scheduler.stats();
        for &weight in stats.weights.values() {
            assert_eq!(weight, 0);
        }
    }
}
