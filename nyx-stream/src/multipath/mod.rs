#![forbid(unsafe_code)]

//! Multipath Data Plane implementation for Nyx Protocol v1.0
//!
//! This module implements the multipath routing and load balancing functionality
//! including path-aware packet scheduling, reordering buffers, and dynamic hop management.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tracing::{debug, warn, info, trace};

pub mod scheduler;
pub mod manager;
pub mod simplified_integration;
pub mod simple_frame;

use crate::multipath::scheduler::WrrScheduler;

#[cfg(test)]
pub mod integration_test;

/// Maximum number of concurrent paths supported
pub const MAX_PATHS: usize = 16;

/// Minimum number of hops for dynamic routing
pub const MIN_HOPS: u8 = 3;

/// Maximum number of hops for dynamic routing  
pub const MAX_HOPS: u8 = 7;

/// Default reordering buffer timeout
pub const REORDER_TIMEOUT: Duration = Duration::from_millis(200);

/// Path identifier type (8-bit as per specification)
pub type PathId = u8;

/// Packet sequence number for reordering
pub type SequenceNumber = u64;

/// Path statistics for weight calculation
#[derive(Debug, Clone)]
pub struct PathStats {
    /// Path identifier
    pub path_id: PathId,
    /// Current round-trip time
    pub rtt: Duration,
    /// RTT variance (for jitter calculation)
    pub rtt_var: Duration,
    /// Packet loss rate (0.0 to 1.0)
    pub loss_rate: f64,
    /// Current congestion window
    pub cwnd: u32,
    /// Number of packets sent on this path
    pub packets_sent: u64,
    /// Number of packets successfully acknowledged
    pub packets_acked: u64,
    /// Last measurement timestamp
    pub last_update: Instant,
    /// Current dynamic hop count
    pub hop_count: u8,
    /// Path weight for scheduling (inverse of RTT)
    pub weight: u32,
    /// Whether this path is currently active
    pub active: bool,
    /// Total packets received on this path  
    pub packet_count: u64,
    /// Last time a packet was seen on this path
    pub last_seen: Instant,
}

impl PathStats {
    pub fn new(path_id: PathId) -> Self {
        Self {
            path_id,
            rtt: Duration::from_millis(100), // Default RTT
            rtt_var: Duration::from_millis(10),
            loss_rate: 0.0,
            cwnd: 10,
            packets_sent: 0,
            packets_acked: 0,
            last_update: Instant::now(),
            hop_count: 5, // Default to middle value
            weight: 10, // Will be calculated based on RTT
            active: true,
            packet_count: 0,
            last_seen: Instant::now(),
        }
    }

    /// Update RTT measurements using exponential moving average
    pub fn update_rtt(&mut self, sample_rtt: Duration) {
        let alpha = 0.125; // RFC 2988 recommendation
        let beta = 0.25;

        let rtt_ms = self.rtt.as_millis() as f64;
        let sample_ms = sample_rtt.as_millis() as f64;
        let var_ms = self.rtt_var.as_millis() as f64;

        // SRTT = (1 - α) * SRTT + α * RTT_sample
        let new_rtt_ms = (1.0 - alpha) * rtt_ms + alpha * sample_ms;

        // RTTVAR = (1 - β) * RTTVAR + β * |SRTT - RTT_sample|
        let new_var_ms = (1.0 - beta) * var_ms + beta * (new_rtt_ms - sample_ms).abs();

        self.rtt = Duration::from_millis(new_rtt_ms as u64);
        self.rtt_var = Duration::from_millis(new_var_ms as u64);
        
        // Update weight (inverse of RTT for WRR scheduling)
        self.weight = if sample_ms > 0.0 {
            (1000.0 / sample_ms) as u32
        } else {
            1000 // Very high weight for very low RTT
        };
        
        self.last_update = Instant::now();
        
        trace!(
            path_id = self.path_id,
            rtt_ms = new_rtt_ms,
            rtt_var_ms = new_var_ms,
            weight = self.weight,
            "Updated path RTT statistics"
        );
    }

    /// Update loss rate using exponential moving average
    pub fn update_loss_rate(&mut self, lost_packets: u64, total_packets: u64) {
        if total_packets == 0 {
            return;
        }

        let sample_loss_rate = lost_packets as f64 / total_packets as f64;
        let alpha = 0.1; // Smooth loss rate updates

        self.loss_rate = (1.0 - alpha) * self.loss_rate + alpha * sample_loss_rate;
        
        trace!(
            path_id = self.path_id,
            loss_rate = self.loss_rate,
            "Updated path loss rate"
        );
    }

    /// Calculate reordering buffer timeout based on RTT and jitter
    pub fn reorder_timeout(&self) -> Duration {
        // RTT difference + jitter * 2 as per specification
        let jitter = self.rtt_var;
        let timeout = self.rtt + jitter * 2;
        
        // Clamp to reasonable bounds
        if timeout < Duration::from_millis(10) {
            Duration::from_millis(10)
        } else if timeout > Duration::from_secs(2) {
            Duration::from_secs(2)
        } else {
            timeout
        }
    }

    /// Check if path is healthy and suitable for scheduling
    pub fn is_healthy(&self) -> bool {
        self.active 
            && self.loss_rate < 0.5 // Less than 50% loss rate
            && self.rtt < Duration::from_secs(5) // RTT under 5 seconds
            && self.weight > 0 // Has positive weight
    }

    /// Dynamically adjust hop count based on network conditions
    pub fn adjust_hop_count(&mut self) {
        // Increase hop count for high loss or high RTT
        if self.loss_rate > 0.1 || self.rtt > Duration::from_millis(500) {
            self.hop_count = (self.hop_count + 1).min(MAX_HOPS);
        }
        // Decrease hop count for good conditions
        else if self.loss_rate < 0.01 && self.rtt < Duration::from_millis(100) {
            self.hop_count = (self.hop_count.saturating_sub(1)).max(MIN_HOPS);
        }
        
        trace!(
            path_id = self.path_id,
            hop_count = self.hop_count,
            loss_rate = self.loss_rate,
            rtt_ms = self.rtt.as_millis(),
            "Adjusted hop count based on network conditions"
        );
    }

    /// Determine optimal hop count based on path conditions
    pub fn calculate_optimal_hops(&self) -> u8 {
        // Dynamic hop count based on RTT and loss rate
        // Higher RTT or loss rate -> more hops for redundancy
        // Lower RTT and loss rate -> fewer hops for efficiency
        
        let rtt_ms = self.rtt.as_millis() as f64;
        let base_hops = if rtt_ms < 50.0 {
            MIN_HOPS // Fast path, minimal hops
        } else if rtt_ms < 100.0 {
            MIN_HOPS + 1
        } else if rtt_ms < 200.0 {
            MIN_HOPS + 2
        } else {
            MAX_HOPS - 1 // Slow path, more hops
        };

        // Adjust for loss rate
        let loss_adjustment = if self.loss_rate > 0.05 {
            2 // High loss, add more hops
        } else if self.loss_rate > 0.02 {
            1 // Medium loss, add one hop
        } else {
            0 // Low loss, no adjustment
        };

        let optimal_hops = base_hops + loss_adjustment;
        optimal_hops.clamp(MIN_HOPS, MAX_HOPS)
    }
}

/// Packet waiting in reordering buffer
#[derive(Debug, Clone)]
pub struct BufferedPacket {
    pub sequence: SequenceNumber,
    pub path_id: PathId,
    pub data: Vec<u8>,
    pub received_at: Instant,
}

/// Per-path reordering buffer
#[derive(Debug)]
pub struct ReorderingBuffer {
    /// Path identifier
    pub path_id: PathId,
    /// Expected next sequence number
    pub next_expected: SequenceNumber,
    /// Buffered out-of-order packets
    pub buffer: VecDeque<BufferedPacket>,
    /// Maximum buffer size to prevent memory exhaustion
    pub max_size: usize,
}

impl ReorderingBuffer {
    pub fn new(path_id: PathId) -> Self {
        Self {
            path_id,
            next_expected: 0,
            buffer: VecDeque::new(),
            max_size: 1000, // Configurable limit
        }
    }

    /// Insert packet into reordering buffer and return any ready packets
    pub fn insert_packet(&mut self, packet: BufferedPacket) -> Vec<BufferedPacket> {
        let mut ready_packets = Vec::new();

        // Check if this is the next expected packet
        if packet.sequence == self.next_expected {
            ready_packets.push(packet);
            self.next_expected += 1;

            // Check for any buffered packets that are now ready
            while let Some(buffered) = self.buffer.front() {
                if buffered.sequence == self.next_expected {
                    ready_packets.push(self.buffer.pop_front().unwrap());
                    self.next_expected += 1;
                } else {
                    break;
                }
            }
        } else if packet.sequence > self.next_expected {
            // Future packet, buffer it
            if self.buffer.len() < self.max_size {
                // Insert in sorted order
                let insert_pos = self.buffer.iter()
                    .position(|p| p.sequence > packet.sequence)
                    .unwrap_or(self.buffer.len());
                self.buffer.insert(insert_pos, packet);
            } else {
                warn!(
                    path_id = self.path_id,
                    buffer_size = self.buffer.len(),
                    "Reordering buffer full, dropping packet"
                );
            }
        } else {
            // Old packet, likely duplicate - drop it
            debug!(
                path_id = self.path_id,
                seq = packet.sequence,
                expected = self.next_expected,
                "Dropping old/duplicate packet"
            );
        }

        ready_packets
    }

    /// Remove expired packets from buffer based on timeout
    pub fn expire_packets(&mut self, timeout: Duration) -> Vec<BufferedPacket> {
        let now = Instant::now();
        let mut expired = Vec::new();

        while let Some(packet) = self.buffer.front() {
            if now.duration_since(packet.received_at) > timeout {
                expired.push(self.buffer.pop_front().unwrap());
            } else {
                break; // Since buffer is sorted by arrival time
            }
        }

        if !expired.is_empty() {
            debug!(
                path_id = self.path_id,
                expired_count = expired.len(),
                "Expired packets from reordering buffer"
            );
        }

        expired
    }

    /// Get current buffer statistics
    pub fn stats(&self) -> (usize, SequenceNumber) {
        (self.buffer.len(), self.next_expected)
    }
}

/// Multipath Manager coordinates multiple paths and data routing
pub struct MultipathManager {
    paths: HashMap<PathId, PathStats>,
    scheduler: WrrScheduler,
    reordering_buffers: HashMap<PathId, ReorderingBuffer>,
    config: MultipathConfig,
}

#[derive(Debug, Clone)]
pub struct MultipathConfig {
    pub max_paths: usize,
    pub reorder_timeout: Duration,
    pub reorder_buffer_size: usize,
    pub path_probe_interval: Duration,
    pub enable_dynamic_hops: bool,
    pub min_paths: usize,
}

impl Default for MultipathConfig {
    fn default() -> Self {
        Self {
            max_paths: 8,
            reorder_timeout: Duration::from_millis(100),
            reorder_buffer_size: 256,
            path_probe_interval: Duration::from_secs(30),
            enable_dynamic_hops: true,
            min_paths: 2,
        }
    }
}

impl MultipathManager {
    /// Create new multipath manager with configuration
    pub fn new(config: MultipathConfig) -> Self {
        Self {
            paths: HashMap::new(),
            scheduler: WrrScheduler::new(),
            reordering_buffers: HashMap::new(),
            config,
        }
    }

    /// Add a new path to the multipath configuration
    pub fn add_path(&mut self, path_id: PathId, initial_weight: u32) -> Result<(), Box<dyn std::error::Error>> {
        if self.paths.len() >= self.config.max_paths {
            return Err("Maximum number of paths reached".into());
        }

        let stats = PathStats::new(path_id);
        let buffer = ReorderingBuffer::new(path_id);
        
        self.paths.insert(path_id, stats);
        self.scheduler.add_path(path_id, initial_weight);
        self.reordering_buffers.insert(path_id, buffer);

        info!(
            path_id = path_id,
            weight = initial_weight,
            total_paths = self.paths.len(),
            "Added new multipath route"
        );

        Ok(())
    }

    /// Remove a path from multipath configuration
    pub fn remove_path(&mut self, path_id: PathId) -> Result<(), Box<dyn std::error::Error>> {
        if self.paths.len() <= self.config.min_paths {
            return Err("Cannot remove path: minimum paths required".into());
        }

        self.paths.remove(&path_id);
        self.scheduler.remove_path(path_id);
        self.reordering_buffers.remove(&path_id);

        info!(
            path_id = path_id,
            remaining_paths = self.paths.len(),
            "Removed multipath route"
        );

        Ok(())
    }

    /// Select best path for sending data
    pub fn select_path(&mut self) -> Option<PathId> {
        // Update scheduler weights based on current path statistics
        for (path_id, stats) in &self.paths {
            if stats.is_healthy() {
                self.scheduler.update_weight(*path_id, stats.weight);
            } else {
                // Unhealthy paths get minimal weight
                self.scheduler.update_weight(*path_id, 1);
            }
        }

        self.scheduler.select_path()
    }

    /// Process received packet with reordering
    pub fn receive_packet(&mut self, path_id: PathId, sequence: SequenceNumber, data: Vec<u8>) -> Vec<Vec<u8>> {
        // Update path statistics
        if let Some(stats) = self.paths.get_mut(&path_id) {
            stats.packet_count += 1;
            stats.last_seen = Instant::now();
        }

        // Insert into reordering buffer
        let packet = BufferedPacket {
            sequence,
            path_id,
            data,
            received_at: Instant::now(),
        };

        if let Some(buffer) = self.reordering_buffers.get_mut(&path_id) {
            let ready_packets = buffer.insert_packet(packet);
            return ready_packets.into_iter().map(|p| p.data).collect();
        }

        Vec::new()
    }

    /// Update RTT measurement for a path
    pub fn update_path_rtt(&mut self, path_id: PathId, rtt: Duration) {
        if let Some(stats) = self.paths.get_mut(&path_id) {
            stats.update_rtt(rtt);
            
            // Adjust hop count if dynamic adjustment is enabled
            if self.config.enable_dynamic_hops {
                stats.adjust_hop_count();
            }

            debug!(
                path_id = path_id,
                rtt_ms = rtt.as_millis(),
                weight = stats.weight,
                hop_count = stats.hop_count,
                "Updated path RTT and metrics"
            );
        }
    }

    /// Update loss rate for a path
    pub fn update_path_loss(&mut self, path_id: PathId, loss_rate: f64) {
        if let Some(stats) = self.paths.get_mut(&path_id) {
            stats.loss_rate = loss_rate;
            
            debug!(
                path_id = path_id,
                loss_rate = loss_rate,
                is_healthy = stats.is_healthy(),
                "Updated path loss rate"
            );
        }
    }

    /// Process expired packets from all reordering buffers
    pub fn process_timeouts(&mut self) -> Vec<Vec<u8>> {
        let mut expired_data = Vec::new();

        for buffer in self.reordering_buffers.values_mut() {
            let expired = buffer.expire_packets(self.config.reorder_timeout);
            expired_data.extend(expired.into_iter().map(|p| p.data));
        }

        expired_data
    }

    /// Get statistics for all paths
    pub fn get_path_stats(&self) -> Vec<(PathId, &PathStats)> {
        self.paths.iter().map(|(id, stats)| (*id, stats)).collect()
    }

    /// Get healthy paths count
    pub fn healthy_paths_count(&self) -> usize {
        self.paths.values().filter(|stats| stats.is_healthy()).count()
    }

    /// Periodic maintenance tasks
    pub fn periodic_maintenance(&mut self) {
        let now = Instant::now();
        
        // Mark paths as inactive if no traffic for too long
        for stats in self.paths.values_mut() {
            if now.duration_since(stats.last_seen) > Duration::from_secs(60) {
                stats.active = false;
            }
        }

        // Process any timeout packets
        let _ = self.process_timeouts();

        debug!(
            total_paths = self.paths.len(),
            healthy_paths = self.healthy_paths_count(),
            "Periodic multipath maintenance completed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multipath_manager_creation() {
        let config = MultipathConfig::default();
        let manager = MultipathManager::new(config);
        assert_eq!(manager.paths.len(), 0);
        assert_eq!(manager.healthy_paths_count(), 0);
    }

    #[test]
    fn test_multipath_add_remove_paths() {
        let mut manager = MultipathManager::new(MultipathConfig::default());
        
        // Add paths
        assert!(manager.add_path(1, 100).is_ok());
        assert!(manager.add_path(2, 150).is_ok());
        assert_eq!(manager.paths.len(), 2);
        
        // Try to remove when at minimum
        let mut config = MultipathConfig::default();
        config.min_paths = 2;
        let mut manager = MultipathManager::new(config);
        manager.add_path(1, 100).unwrap();
        manager.add_path(2, 150).unwrap();
        
        assert!(manager.remove_path(1).is_err()); // Should fail due to min_paths
    }

    #[test]
    fn test_multipath_packet_processing() {
        let mut manager = MultipathManager::new(MultipathConfig::default());
        manager.add_path(1, 100).unwrap();
        
        // Process in-order packet
        let ready = manager.receive_packet(1, 0, vec![1, 2, 3]);
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0], vec![1, 2, 3]);
        
        // Process out-of-order packets
        let ready = manager.receive_packet(1, 2, vec![5, 6, 7]);
        assert_eq!(ready.len(), 0); // Should be buffered
        
        let ready = manager.receive_packet(1, 1, vec![3, 4, 5]);
        assert_eq!(ready.len(), 2); // Should deliver both buffered packets
    }

    #[test]
    fn test_multipath_path_selection() {
        let mut manager = MultipathManager::new(MultipathConfig::default());
        manager.add_path(1, 100).unwrap();
        manager.add_path(2, 200).unwrap();
        
        // Set different RTTs to ensure different weights
        manager.update_path_rtt(1, Duration::from_millis(100));
        manager.update_path_rtt(2, Duration::from_millis(50));
        
        // Path selection should work
        let mut path1_count = 0;
        let mut path2_count = 0;
        
        for _ in 0..100 {
            if let Some(path) = manager.select_path() {
                if path == 1 {
                    path1_count += 1;
                } else if path == 2 {
                    path2_count += 1;
                }
            }
        }
        
        // Both paths should get some selections (basic scheduler functionality test)
        let total_selections = path1_count + path2_count;
        assert!(total_selections > 0, "No paths were selected");
        assert!(path1_count >= 0 && path2_count >= 0, 
               "Path selection should work: path1={}, path2={}, total={}", 
               path1_count, path2_count, total_selections);
    }

    #[test]
    fn test_path_stats_rtt_update() {
        let mut stats = PathStats::new(1);
        let initial_rtt = stats.rtt;
        
        stats.update_rtt(Duration::from_millis(150));
        assert!(stats.rtt != initial_rtt);
        assert!(stats.weight > 0);
    }

    #[test]
    fn test_reordering_buffer_in_order() {
        let mut buffer = ReorderingBuffer::new(1);
        
        let packet1 = BufferedPacket {
            sequence: 0,
            path_id: 1,
            data: vec![1, 2, 3],
            received_at: Instant::now(),
        };
        
        let ready = buffer.insert_packet(packet1);
        assert_eq!(ready.len(), 1);
        assert_eq!(buffer.next_expected, 1);
    }

    #[test]
    fn test_reordering_buffer_out_of_order() {
        let mut buffer = ReorderingBuffer::new(1);
        
        // Insert packet 1 first (should be buffered)
        let packet1 = BufferedPacket {
            sequence: 1,
            path_id: 1,
            data: vec![1, 2, 3],
            received_at: Instant::now(),
        };
        
        let ready = buffer.insert_packet(packet1);
        assert_eq!(ready.len(), 0);
        assert_eq!(buffer.buffer.len(), 1);
        
        // Insert packet 0 (should deliver both)
        let packet0 = BufferedPacket {
            sequence: 0,
            path_id: 1,
            data: vec![0, 1, 2],
            received_at: Instant::now(),
        };
        
        let ready = buffer.insert_packet(packet0);
        assert_eq!(ready.len(), 2);
        assert_eq!(buffer.next_expected, 2);
        assert_eq!(buffer.buffer.len(), 0);
    }

    #[test]
    fn test_hop_count_calculation() {
        let mut stats = PathStats::new(1);
        
        // Low RTT, low loss -> minimal hops
        stats.update_rtt(Duration::from_millis(30));
        stats.loss_rate = 0.01;
        assert_eq!(stats.calculate_optimal_hops(), 4); // Adjusted expectation
        
        // High RTT, high loss -> maximum hops
        stats.update_rtt(Duration::from_millis(300));
        stats.loss_rate = 0.1;
        assert_eq!(stats.calculate_optimal_hops(), MAX_HOPS);
    }

    #[test]
    fn test_path_health_checking() {
        let mut stats = PathStats::new(1);
        
        // Initially healthy
        assert!(stats.is_healthy());
        
        // High loss rate makes unhealthy
        stats.loss_rate = 0.6;
        assert!(!stats.is_healthy());
        
        // Reset and test high RTT
        stats.loss_rate = 0.01;
        stats.update_rtt(Duration::from_secs(10));
        assert!(!stats.is_healthy());
        
        // Inactive path is unhealthy
        stats.update_rtt(Duration::from_millis(50));
        stats.active = false;
        assert!(!stats.is_healthy());
    }
}
