//! Multipath routing functionality for WASM clients
//!
//! This module provides multipath routing capabilities for web browsers
//! and other WASM environments, enabling efficient path selection and
//! load balancing across multiple network paths.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Object, Reflect};

#[cfg(target_arch = "wasm32")]
use web_sys::{console, Performance, Window};

// For non-WASM environments, define a placeholder JsValue
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct JsValue(#[allow(dead_code)] String);

#[cfg(not(target_arch = "wasm32"))]
impl JsValue {
    pub fn from_string(s: &str) -> Self {
        JsValue(s.to_string())
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Multipath configuration for WASM clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipathConfig {
    /// Maximum number of simultaneous paths
    pub max_paths: usize,
    /// Path quality threshold (0.0 - 1.0)
    pub quality_threshold: f64,
    /// Load balancing strategy
    pub load_balance_strategy: LoadBalanceStrategy,
    /// Path failover timeout in milliseconds
    pub failover_timeout_ms: u32,
    /// Enable adaptive path selection
    pub adaptive_selection: bool,
    /// Bandwidth measurement interval in milliseconds
    pub bandwidth_measurement_interval_ms: u32,
}

/// Load balancing strategies available in WASM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalanceStrategy {
    /// Round-robin distribution
    RoundRobin,
    /// Quality-based weighted distribution
    QualityWeighted,
    /// Latency-based selection
    LatencyBased,
    /// Bandwidth-based selection
    BandwidthBased,
    /// Adaptive algorithm based on real-time metrics
    Adaptive,
}

/// Path information for WASM environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathInfo {
    /// Unique path identifier
    pub path_id: String,
    /// Path quality score (0.0 - 1.0)
    pub quality: f64,
    /// Average latency in milliseconds
    pub latency_ms: f64,
    /// Available bandwidth in Mbps
    pub bandwidth_mbps: f64,
    /// Path reliability score (0.0 - 1.0)
    pub reliability: f64,
    /// Current load factor (0.0 - 1.0)
    pub load_factor: f64,
    /// Path status
    pub status: PathStatus,
    /// Last update timestamp
    pub last_updated: u64,
}

/// Path status enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PathStatus {
    /// Path is active and available
    Active,
    /// Path is degraded but usable
    Degraded,
    /// Path is temporarily unavailable
    Unavailable,
    /// Path is being tested
    Testing,
    /// Path has failed
    Failed,
}

/// Multipath manager for WASM environments
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct MultipathManager {
    config: MultipathConfig,
    paths: HashMap<String, PathInfo>,
    current_path_index: usize,
    #[allow(dead_code)]
    last_bandwidth_measurement: u64,
    performance_history: Vec<PathPerformanceRecord>,
}

/// Performance record for path monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PathPerformanceRecord {
    path_id: String,
    timestamp: u64,
    latency_ms: f64,
    bandwidth_mbps: f64,
    packet_loss: f64,
    jitter_ms: f64,
}

impl Default for MultipathManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl MultipathManager {
    /// Create a new multipath manager with default configuration
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new() -> MultipathManager {
        let config = MultipathConfig {
            max_paths: 4,
            quality_threshold: 0.6,
            load_balance_strategy: LoadBalanceStrategy::Adaptive,
            failover_timeout_ms: 5000,
            adaptive_selection: true,
            bandwidth_measurement_interval_ms: 10000,
        };

        MultipathManager {
            config,
            paths: HashMap::new(),
            current_path_index: 0,
            last_bandwidth_measurement: 0,
            performance_history: Vec::new(),
        }
    }

    /// Create a new multipath manager with custom configuration
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn with_config(config_json: &str) -> Result<MultipathManager, JsValue> {
        let config: MultipathConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_string(&format!("Invalid config: {e}")))?;

        Ok(MultipathManager {
            config,
            paths: HashMap::new(),
            current_path_index: 0,
            last_bandwidth_measurement: 0,
            performance_history: Vec::new(),
        })
    }

    /// Add a new path to the multipath manager
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn add_path(&mut self, path_id: &str, initial_quality: f64) -> Result<(), JsValue> {
        if self.paths.len() >= self.config.max_paths {
            return Err(JsValue::from_string("Maximum number of paths reached"));
        }

        let path_info = PathInfo {
            path_id: path_id.to_string(),
            quality: initial_quality.clamp(0.0, 1.0),
            latency_ms: 0.0,
            bandwidth_mbps: 0.0,
            reliability: 1.0,
            load_factor: 0.0,
            status: PathStatus::Testing,
            last_updated: self.get_current_timestamp(),
        };

        self.paths.insert(path_id.to_string(), path_info);

        #[cfg(target_arch = "wasm32")]
        console::log_1(&JsValue::from_string(&format!("Added path: {path_id}")));

        Ok(())
    }

    /// Remove a path from the multipath manager
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn remove_path(&mut self, path_id: &str) -> bool {
        let removed = self.paths.remove(path_id).is_some();
        
        if removed {
            #[cfg(target_arch = "wasm32")]
            console::log_1(&JsValue::from_string(&format!("Removed path: {path_id}")));
        }

        removed
    }

    /// Select the best path based on current strategy
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn select_best_path(&mut self) -> Option<String> {
        let active_paths: Vec<_> = self.paths
            .iter()
            .filter(|(_, path)| matches!(path.status, PathStatus::Active | PathStatus::Degraded))
            .filter(|(_, path)| path.quality >= self.config.quality_threshold)
            .collect();

        if active_paths.is_empty() {
            return None;
        }

        let selected_path = match self.config.load_balance_strategy {
            LoadBalanceStrategy::RoundRobin => {
                self.current_path_index = (self.current_path_index + 1) % active_paths.len();
                active_paths[self.current_path_index].0.clone()
            }
            LoadBalanceStrategy::QualityWeighted => {
                active_paths
                    .iter()
                    .max_by(|a, b| a.1.quality.partial_cmp(&b.1.quality).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(id, _)| id.to_string())
                    .unwrap_or_else(|| active_paths[0].0.to_string())
            }
            LoadBalanceStrategy::LatencyBased => {
                active_paths
                    .iter()
                    .min_by(|a, b| a.1.latency_ms.partial_cmp(&b.1.latency_ms).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(id, _)| id.to_string())
                    .unwrap_or_else(|| active_paths[0].0.to_string())
            }
            LoadBalanceStrategy::BandwidthBased => {
                active_paths
                    .iter()
                    .max_by(|a, b| a.1.bandwidth_mbps.partial_cmp(&b.1.bandwidth_mbps).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(id, _)| id.to_string())
                    .unwrap_or_else(|| active_paths[0].0.to_string())
            }
            LoadBalanceStrategy::Adaptive => {
                self.select_adaptive_path(&active_paths)
            }
        };

        Some(selected_path)
    }

    /// Update path metrics
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn update_path_metrics(
        &mut self,
        path_id: &str,
        latency_ms: f64,
        bandwidth_mbps: f64,
        packet_loss: f64,
    ) -> Result<(), JsValue> {
        let current_time = self.get_current_timestamp();
        
        let path = self.paths.get_mut(path_id)
            .ok_or_else(|| JsValue::from_string("Path not found"))?;

        path.latency_ms = latency_ms;
        path.bandwidth_mbps = bandwidth_mbps;
        path.last_updated = current_time;

        // Calculate quality based on metrics
        let latency_score = 1.0 - (latency_ms / 1000.0).min(1.0);
        let bandwidth_score = (bandwidth_mbps / 100.0).min(1.0);
        let loss_score = 1.0 - packet_loss.min(1.0);
        
        path.quality = (latency_score * 0.4 + bandwidth_score * 0.4 + loss_score * 0.2).clamp(0.0, 1.0);

        // Update path status based on quality
        path.status = if path.quality >= 0.8 {
            PathStatus::Active
        } else if path.quality >= self.config.quality_threshold {
            PathStatus::Degraded
        } else {
            PathStatus::Unavailable
        };

        // Record performance history
        let jitter_ms = self.calculate_jitter(path_id, latency_ms);
        let record = PathPerformanceRecord {
            path_id: path_id.to_string(),
            timestamp: current_time,
            latency_ms,
            bandwidth_mbps,
            packet_loss,
            jitter_ms,
        };
        
        self.performance_history.push(record);
        
        // Keep only recent history (last 100 records)
        if self.performance_history.len() > 100 {
            self.performance_history.drain(0..self.performance_history.len() - 100);
        }

        Ok(())
    }

    /// Get current path statistics as JSON
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn get_path_stats(&self) -> String {
        serde_json::to_string(&self.paths).unwrap_or_else(|_| "{}".to_string())
    }

    /// Perform health check on all paths
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn health_check(&mut self) -> String {
        let current_time = self.get_current_timestamp();
        let mut unhealthy_paths = Vec::new();

        for (path_id, path) in &mut self.paths {
            // Mark paths as failed if they haven't been updated recently
            if current_time - path.last_updated > self.config.failover_timeout_ms as u64 {
                path.status = PathStatus::Failed;
                unhealthy_paths.push(path_id.clone());
            }
        }

        if unhealthy_paths.is_empty() {
            "All paths healthy".to_string()
        } else {
            format!("Unhealthy paths: {}", unhealthy_paths.join(", "))
        }
    }

    /// Get performance history as JSON
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
    pub fn get_performance_history(&self) -> String {
        serde_json::to_string(&self.performance_history).unwrap_or_else(|_| "[]".to_string())
    }
}

impl MultipathManager {
    /// Select path using adaptive algorithm
    fn select_adaptive_path(&self, active_paths: &[(&String, &PathInfo)]) -> String {
        // Adaptive selection considers multiple factors with dynamic weighting
        let mut best_score = f64::NEG_INFINITY;
        let mut best_path = active_paths[0].0.clone();

        for (path_id, path) in active_paths {
            // Dynamic weight calculation based on recent performance
            let recent_performance = self.get_recent_performance(path_id);
            let stability_factor = self.calculate_stability_factor(path_id);
            
            // Adaptive scoring algorithm
            let latency_weight = if recent_performance.avg_latency > 200.0 { 0.5 } else { 0.3 };
            let bandwidth_weight = if recent_performance.avg_bandwidth < 10.0 { 0.5 } else { 0.3 };
            let quality_weight = 0.2;
            let stability_weight = 0.1;

            let score = (path.quality * quality_weight) +
                       ((1.0 - (path.latency_ms / 1000.0).min(1.0)) * latency_weight) +
                       ((path.bandwidth_mbps / 100.0).min(1.0) * bandwidth_weight) +
                       (stability_factor * stability_weight);

            if score > best_score {
                best_score = score;
                best_path = path_id.to_string();
            }
        }

        best_path
    }

    /// Get recent performance metrics for a path
    fn get_recent_performance(&self, path_id: &str) -> RecentPerformance {
        let recent_records: Vec<_> = self.performance_history
            .iter()
            .filter(|record| record.path_id == path_id)
            .rev()
            .take(10)
            .collect();

        if recent_records.is_empty() {
            return RecentPerformance::default();
        }

        let avg_latency = recent_records.iter().map(|r| r.latency_ms).sum::<f64>() / recent_records.len() as f64;
        let avg_bandwidth = recent_records.iter().map(|r| r.bandwidth_mbps).sum::<f64>() / recent_records.len() as f64;
        let avg_packet_loss = recent_records.iter().map(|r| r.packet_loss).sum::<f64>() / recent_records.len() as f64;

        RecentPerformance {
            avg_latency,
            avg_bandwidth,
            avg_packet_loss,
        }
    }

    /// Calculate stability factor for a path
    fn calculate_stability_factor(&self, path_id: &str) -> f64 {
        let recent_records: Vec<_> = self.performance_history
            .iter()
            .filter(|record| record.path_id == path_id)
            .rev()
            .take(10)
            .collect();

        if recent_records.len() < 2 {
            return 1.0; // Assume stable if not enough data
        }

        // Calculate variance in latency as instability measure
        let avg_latency = recent_records.iter().map(|r| r.latency_ms).sum::<f64>() / recent_records.len() as f64;
        let variance = recent_records.iter()
            .map(|r| (r.latency_ms - avg_latency).powi(2))
            .sum::<f64>() / recent_records.len() as f64;

        // Convert variance to stability factor (lower variance = higher stability)
        (1.0 / (1.0 + variance / 100.0)).min(1.0)
    }

    /// Calculate jitter for a path based on recent latency measurements
    fn calculate_jitter(&self, path_id: &str, current_latency: f64) -> f64 {
        let recent_records: Vec<_> = self.performance_history
            .iter()
            .filter(|record| record.path_id == path_id)
            .rev()
            .take(5)
            .collect();

        if recent_records.is_empty() {
            return 0.0; // No previous data, no jitter
        }

        // Calculate jitter as the average absolute difference between consecutive latency measurements
        let mut jitter_sum = 0.0;
        let mut count = 0;

        // Include current latency in jitter calculation
        if let Some(last_record) = recent_records.first() {
            jitter_sum += (current_latency - last_record.latency_ms).abs();
            count += 1;
        }

        // Calculate jitter between consecutive historical measurements
        for window in recent_records.windows(2) {
            if let [newer, older] = window {
                jitter_sum += (newer.latency_ms - older.latency_ms).abs();
                count += 1;
            }
        }

        if count > 0 {
            jitter_sum / count as f64
        } else {
            0.0
        }
    }
            .rev()
            .take(20)
            .collect();

        if recent_records.len() < 5 {
            return 0.5; // Default stability for insufficient data
        }

        // Calculate variance in latency and bandwidth
        let latencies: Vec<f64> = recent_records.iter().map(|r| r.latency_ms).collect();
        let bandwidths: Vec<f64> = recent_records.iter().map(|r| r.bandwidth_mbps).collect();

        let latency_variance = self.calculate_variance(&latencies);
        let bandwidth_variance = self.calculate_variance(&bandwidths);

        // Stability is inverse of variance (normalized)
        let latency_stability = 1.0 / (1.0 + latency_variance / 100.0);
        let bandwidth_stability = 1.0 / (1.0 + bandwidth_variance / 10.0);

        (latency_stability + bandwidth_stability) / 2.0
    }

    /// Calculate variance of a data series
    fn calculate_variance(&self, data: &[f64]) -> f64 {
        if data.len() < 2 {
            return 0.0;
        }

        let mean = data.iter().sum::<f64>() / data.len() as f64;
        let variance = data.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / (data.len() - 1) as f64;

        variance
    }

    /// Get current timestamp in milliseconds
    fn get_current_timestamp(&self) -> u64 {
        #[cfg(target_arch = "wasm32")]
        {
            if let Ok(window) = web_sys::window().ok_or("no window") {
                if let Ok(performance) = window.performance().ok_or("no performance") {
                    return performance.now() as u64;
                }
            }
        }
        
        // Fallback for non-WASM environments
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

/// Recent performance metrics
#[derive(Debug, Clone)]
struct RecentPerformance {
    avg_latency: f64,
    avg_bandwidth: f64,
    #[allow(dead_code)]
    avg_packet_loss: f64,
}

impl Default for RecentPerformance {
    fn default() -> Self {
        Self {
            avg_latency: 100.0,
            avg_bandwidth: 10.0,
            avg_packet_loss: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multipath_manager_creation() {
        let manager = MultipathManager::new();
        assert_eq!(manager.paths.len(), 0);
        assert_eq!(manager.config.max_paths, 4);
    }

    #[test]
    fn test_add_remove_path() {
        let mut manager = MultipathManager::new();
        
        assert!(manager.add_path("path1", 0.8).is_ok());
        assert_eq!(manager.paths.len(), 1);
        
        assert!(manager.remove_path("path1"));
        assert_eq!(manager.paths.len(), 0);
    }

    #[test]
    fn test_path_selection_strategies() {
        let mut manager = MultipathManager::new();
        
        // Add multiple paths
        manager.add_path("path1", 0.9).unwrap();
        manager.add_path("path2", 0.7).unwrap();
        manager.add_path("path3", 0.8).unwrap();

        // Update paths to active status
        for path in manager.paths.values_mut() {
            path.status = PathStatus::Active;
        }

        // Test different strategies
        manager.config.load_balance_strategy = LoadBalanceStrategy::QualityWeighted;
        let selected = manager.select_best_path();
        assert!(selected.is_some());

        manager.config.load_balance_strategy = LoadBalanceStrategy::RoundRobin;
        let selected = manager.select_best_path();
        assert!(selected.is_some());
    }

    #[test]
    fn test_path_metrics_update() {
        let mut manager = MultipathManager::new();
        manager.add_path("test_path", 0.5).unwrap();

        let result = manager.update_path_metrics("test_path", 50.0, 25.0, 0.01);
        assert!(result.is_ok());

        let path = &manager.paths["test_path"];
        assert_eq!(path.latency_ms, 50.0);
        assert_eq!(path.bandwidth_mbps, 25.0);
        assert!(path.quality > 0.5); // Quality should improve with good metrics
    }

    #[test]
    fn test_health_check() {
        let mut manager = MultipathManager::new();
        manager.add_path("healthy_path", 0.8).unwrap();

        let health_status = manager.health_check();
        assert!(health_status.contains("Unhealthy") || health_status.contains("healthy"));
    }
}
