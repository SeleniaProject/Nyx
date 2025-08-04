#![forbid(unsafe_code)]

//! Multipath manager integrating all multipath functionality
//!
//! This module provides the main coordinator for multipath data plane operations,
//! managing path discovery, scheduling, reordering, and dynamic hop count adjustment.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, info, warn, trace};

use super::{
    PathId, SequenceNumber, PathStats, BufferedPacket, ReorderingBuffer,
    REORDER_TIMEOUT,
};
use super::scheduler::{ImprovedWrrScheduler, SchedulerStats};
use nyx_core::config::MultipathConfig;

/// Multipath packet with metadata
#[derive(Debug, Clone)]
pub struct MultipathPacket {
    pub path_id: PathId,
    pub sequence: SequenceNumber,
    pub data: Vec<u8>,
    pub sent_at: Instant,
    pub hop_count: u8,
}

/// Events emitted by the multipath manager
#[derive(Debug, Clone)]
pub enum MultipathEvent {
    /// New path discovered and activated
    PathActivated { path_id: PathId, hop_count: u8 },
    /// Path deactivated due to poor performance
    PathDeactivated { path_id: PathId, reason: String },
    /// Path statistics updated
    PathStatsUpdated { path_id: PathId, stats: PathStats },
    /// Packet reordered and delivered
    PacketReordered { path_id: PathId, sequence: SequenceNumber, delay: Duration },
    /// Packet expired from reordering buffer
    PacketExpired { path_id: PathId, sequence: SequenceNumber },
    /// Hop count adjusted for path
    HopCountAdjusted { path_id: PathId, old_hops: u8, new_hops: u8 },
}

/// Statistics for the multipath manager
#[derive(Debug, Clone)]
pub struct MultipathStats {
    pub active_paths: usize,
    pub total_packets_sent: u64,
    pub total_packets_received: u64,
    pub total_packets_reordered: u64,
    pub total_packets_expired: u64,
    pub scheduler_stats: SchedulerStats,
    pub path_stats: HashMap<PathId, PathStats>,
    pub reordering_buffer_sizes: HashMap<PathId, usize>,
}

/// Main multipath data plane manager
pub struct MultipathManager {
    /// Configuration parameters
    config: MultipathConfig,
    /// Per-path statistics
    path_stats: Arc<RwLock<HashMap<PathId, PathStats>>>,
    /// Weighted round-robin scheduler
    scheduler: Arc<Mutex<ImprovedWrrScheduler>>,
    /// Per-path reordering buffers
    reordering_buffers: Arc<RwLock<HashMap<PathId, ReorderingBuffer>>>,
    /// Event channel sender
    event_tx: mpsc::UnboundedSender<MultipathEvent>,
    /// Event channel receiver
    event_rx: Arc<Mutex<mpsc::UnboundedReceiver<MultipathEvent>>>,
    /// Global packet counter for sequence numbers
    sequence_counter: Arc<Mutex<SequenceNumber>>,
    /// Total statistics
    total_stats: Arc<Mutex<MultipathStats>>,
    /// Manager start time
    start_time: Instant,
}

impl MultipathManager {
    pub fn new(config: MultipathConfig) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        Self {
            config,
            path_stats: Arc::new(RwLock::new(HashMap::new())),
            scheduler: Arc::new(Mutex::new(ImprovedWrrScheduler::new())),
            reordering_buffers: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(Mutex::new(event_rx)),
            sequence_counter: Arc::new(Mutex::new(0)),
            total_stats: Arc::new(Mutex::new(MultipathStats {
                active_paths: 0,
                total_packets_sent: 0,
                total_packets_received: 0,
                total_packets_reordered: 0,
                total_packets_expired: 0,
                scheduler_stats: SchedulerStats {
                    active_paths: 0,
                    total_weight: 0,
                    last_selected: None,
                    weights: HashMap::new(),
                },
                path_stats: HashMap::new(),
                reordering_buffer_sizes: HashMap::new(),
            })),
            start_time: Instant::now(),
        }
    }

    /// Start the multipath manager background tasks
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting multipath manager");

        // Start health check task
        self.start_health_check_task().await;

        // Start reordering buffer cleanup task
        self.start_reordering_cleanup_task().await;

        // Start hop count adjustment task if enabled
        if self.config.dynamic_hop_count() {
            self.start_hop_adjustment_task().await;
        }

        // Start statistics update task
        self.start_stats_update_task().await;

        Ok(())
    }

    /// Add a new path to the multipath system
    pub async fn add_path(&self, path_id: PathId) -> Result<(), String> {
        let mut path_stats = self.path_stats.write().await;
        let mut reordering_buffers = self.reordering_buffers.write().await;

        if path_stats.len() >= self.config.max_paths {
            return Err(format!("Maximum number of paths ({}) reached", self.config.max_paths));
        }

        if path_stats.contains_key(&path_id) {
            return Err(format!("Path {} already exists", path_id));
        }

        // Create new path statistics
        let stats = PathStats::new(path_id);
        let hop_count = stats.hop_count;
        
        path_stats.insert(path_id, stats);
        reordering_buffers.insert(path_id, ReorderingBuffer::new(path_id));

        // Update scheduler
        {
            let mut scheduler = self.scheduler.lock().unwrap();
            scheduler.update_paths(&path_stats);
        }

        // Emit event
        let _ = self.event_tx.send(MultipathEvent::PathActivated { path_id, hop_count });

        info!(path_id = path_id, hop_count = hop_count, "Added new multipath");

        Ok(())
    }

    /// Remove a path from the multipath system
    pub async fn remove_path(&self, path_id: PathId, reason: String) -> Result<(), String> {
        let mut path_stats = self.path_stats.write().await;
        let mut reordering_buffers = self.reordering_buffers.write().await;

        if !path_stats.contains_key(&path_id) {
            return Err(format!("Path {} does not exist", path_id));
        }

        path_stats.remove(&path_id);
        reordering_buffers.remove(&path_id);

        // Update scheduler
        {
            let mut scheduler = self.scheduler.lock().unwrap();
            scheduler.update_paths(&path_stats);
        }

        // Emit event
        let _ = self.event_tx.send(MultipathEvent::PathDeactivated { path_id, reason: reason.clone() });

        info!(path_id = path_id, reason = reason, "Removed multipath");

        Ok(())
    }

    /// Send packet using multipath scheduling
    pub async fn send_packet(&self, data: Vec<u8>) -> Result<MultipathPacket, String> {
        // Select path using scheduler
        let path_id = {
            let mut scheduler = self.scheduler.lock().unwrap();
            scheduler.select_path()
        }.ok_or("No active paths available")?;

        // Get next sequence number
        let sequence = {
            let mut counter = self.sequence_counter.lock().unwrap();
            let seq = *counter;
            *counter += 1;
            seq
        };

        // Get hop count for selected path
        let hop_count = {
            let path_stats = self.path_stats.read().await;
            path_stats.get(&path_id)
                .map(|stats| stats.hop_count)
                .unwrap_or(5) // Default hop count
        };

        let packet = MultipathPacket {
            path_id,
            sequence,
            data,
            sent_at: Instant::now(),
            hop_count,
        };

        // Update statistics
        {
            let mut stats = self.total_stats.lock().unwrap();
            stats.total_packets_sent += 1;
        }

        trace!(
            path_id = path_id,
            sequence = sequence,
            hop_count = hop_count,
            "Sent packet via multipath"
        );

        Ok(packet)
    }

    /// Receive packet and handle reordering
    pub async fn receive_packet(&self, packet: MultipathPacket) -> Result<Vec<MultipathPacket>, String> {
        let path_id = packet.path_id;
        let mut reordering_buffers = self.reordering_buffers.write().await;

        let buffer = reordering_buffers
            .get_mut(&path_id)
            .ok_or_else(|| format!("No reordering buffer for path {}", path_id))?;

        let buffered_packet = BufferedPacket {
            sequence: packet.sequence,
            path_id: packet.path_id,
            data: packet.data,
            received_at: Instant::now(),
        };

        let ready_packets = buffer.insert_packet(buffered_packet);
        let mut result_packets = Vec::new();

        for ready in ready_packets {
            let multipath_packet = MultipathPacket {
                path_id: ready.path_id,
                sequence: ready.sequence,
                data: ready.data,
                sent_at: packet.sent_at, // Preserve original send time
                hop_count: packet.hop_count,
            };

            // Check if packet was reordered
            if ready.received_at.duration_since(packet.sent_at) > Duration::from_millis(10) {
                let delay = ready.received_at.duration_since(packet.sent_at);
                let _ = self.event_tx.send(MultipathEvent::PacketReordered {
                    path_id,
                    sequence: ready.sequence,
                    delay,
                });

                let mut stats = self.total_stats.lock().unwrap();
                stats.total_packets_reordered += 1;
            }

            result_packets.push(multipath_packet);
        }

        // Update statistics
        {
            let mut stats = self.total_stats.lock().unwrap();
            stats.total_packets_received += 1;
        }

        trace!(
            path_id = path_id,
            sequence = packet.sequence,
            ready_count = result_packets.len(),
            "Received packet via multipath"
        );

        Ok(result_packets)
    }

    /// Update statistics for a specific path
    pub async fn update_path_stats(&self, path_id: PathId, stats: PathStats) -> Result<(), String> {
        let mut path_stats = self.path_stats.write().await;
        
        if !path_stats.contains_key(&path_id) {
            return Err(format!("Path {} not found", path_id));
        }
        
        path_stats.insert(path_id, stats);
        
        // Update scheduler with new statistics
        {
            let mut scheduler = self.scheduler.lock().unwrap();
            scheduler.update_paths(&path_stats);
        }
        
        debug!(path_id = path_id, "Updated path statistics");
        Ok(())
    }

    /// Schedule a packet to be sent on the best available path
    pub async fn schedule_packet(&self, mut packet: MultipathPacket) -> Result<PathId, String> {
        let path_id = {
            let mut scheduler = self.scheduler.lock().unwrap();
            scheduler.select_path().ok_or("No available paths for packet scheduling")?
        };
        
        packet.path_id = path_id;
        
        // Update send statistics
        {
            let mut stats = self.total_stats.lock().unwrap();
            stats.total_packets_sent += 1;
        }
        
        trace!(
            path_id = path_id,
            sequence = packet.sequence,
            "Scheduled packet for transmission"
        );
        
        Ok(path_id)
    }

    /// Update RTT measurement for a path
    pub async fn update_path_rtt(&self, path_id: PathId, rtt: Duration) -> Result<(), String> {
        let mut path_stats = self.path_stats.write().await;
        
        let stats = path_stats
            .get_mut(&path_id)
            .ok_or_else(|| format!("Path {} not found", path_id))?;

        stats.update_rtt(rtt);

        let stats_clone = stats.clone();

        // Update scheduler with new weights
        {
            let mut scheduler = self.scheduler.lock().unwrap();
            scheduler.update_paths(&path_stats);
        }

        // Emit event
        let _ = self.event_tx.send(MultipathEvent::PathStatsUpdated {
            path_id,
            stats: stats_clone,
        });

        Ok(())
    }

    /// Get current multipath statistics
    pub async fn get_stats(&self) -> MultipathStats {
        let path_stats = self.path_stats.read().await;
        let reordering_buffers = self.reordering_buffers.read().await;
        let scheduler_stats = {
            let scheduler = self.scheduler.lock().unwrap();
            scheduler.stats()
        };

        let mut buffer_sizes = HashMap::new();
        for (path_id, buffer) in reordering_buffers.iter() {
            let (size, _) = buffer.stats();
            buffer_sizes.insert(*path_id, size);
        }

        let mut stats = self.total_stats.lock().unwrap();
        stats.active_paths = path_stats.len();
        stats.scheduler_stats = scheduler_stats;
        stats.path_stats = path_stats.clone();
        stats.reordering_buffer_sizes = buffer_sizes;

        stats.clone()
    }

    /// Subscribe to multipath events
    pub fn subscribe_events(&self) -> mpsc::UnboundedReceiver<MultipathEvent> {
        let (_tx, rx) = mpsc::unbounded_channel();
        // In a real implementation, we'd maintain a list of subscribers
        // For simplicity, returning a new receiver
        rx
    }

    // Background task implementations
    async fn start_health_check_task(&self) {
        let path_stats = Arc::clone(&self.path_stats);
        let scheduler = Arc::clone(&self.scheduler);
        let event_tx = self.event_tx.clone();
        let interval_duration = self.config.health_check_interval();

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;

                let mut stats = path_stats.write().await;
                let mut paths_to_remove = Vec::new();

                // Check health of all paths
                for (path_id, path_stat) in stats.iter() {
                    if !path_stat.is_healthy() {
                        paths_to_remove.push(*path_id);
                    }
                }

                // Remove unhealthy paths
                for path_id in paths_to_remove {
                    stats.remove(&path_id);
                    let _ = event_tx.send(MultipathEvent::PathDeactivated {
                        path_id,
                        reason: "Path health check failed".to_string(),
                    });
                    warn!(path_id = path_id, "Removed unhealthy path");
                }

                // Update scheduler
                {
                    let mut scheduler = scheduler.lock().unwrap();
                    scheduler.update_paths(&stats);
                }
            }
        });
    }

    async fn start_reordering_cleanup_task(&self) {
        let reordering_buffers = Arc::clone(&self.reordering_buffers);
        let path_stats = Arc::clone(&self.path_stats);
        let event_tx = self.event_tx.clone();
        let total_stats = Arc::clone(&self.total_stats);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100));
            
            loop {
                interval.tick().await;

                let mut buffers = reordering_buffers.write().await;
                let stats = path_stats.read().await;

                for (path_id, buffer) in buffers.iter_mut() {
                    let timeout = if let Some(path_stat) = stats.get(path_id) {
                        path_stat.reorder_timeout()
                    } else {
                        REORDER_TIMEOUT
                    };

                    let expired = buffer.expire_packets(timeout);
                    
                    if !expired.is_empty() {
                        // Update statistics
                        {
                            let mut total = total_stats.lock().unwrap();
                            total.total_packets_expired += expired.len() as u64;
                        }

                        // Emit events for expired packets
                        for packet in expired {
                            let _ = event_tx.send(MultipathEvent::PacketExpired {
                                path_id: *path_id,
                                sequence: packet.sequence,
                            });
                        }
                    }
                }
            }
        });
    }

    async fn start_hop_adjustment_task(&self) {
        let path_stats = Arc::clone(&self.path_stats);
        let event_tx = self.event_tx.clone();
        let interval_duration = self.config.hop_adjustment_interval();

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;

                let mut stats = path_stats.write().await;

                for (path_id, path_stat) in stats.iter_mut() {
                    let old_hops = path_stat.hop_count;
                    let optimal_hops = path_stat.calculate_optimal_hops();

                    if old_hops != optimal_hops {
                        path_stat.hop_count = optimal_hops;
                        
                        let _ = event_tx.send(MultipathEvent::HopCountAdjusted {
                            path_id: *path_id,
                            old_hops,
                            new_hops: optimal_hops,
                        });

                        debug!(
                            path_id = *path_id,
                            old_hops = old_hops,
                            new_hops = optimal_hops,
                            rtt_ms = path_stat.rtt.as_millis(),
                            loss_rate = path_stat.loss_rate,
                            "Adjusted hop count"
                        );
                    }
                }
            }
        });
    }

    async fn start_stats_update_task(&self) {
        let scheduler = Arc::clone(&self.scheduler);
        let path_stats = Arc::clone(&self.path_stats);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(1));
            
            loop {
                interval.tick().await;

                // Update scheduler if needed
                let needs_update = {
                    let scheduler_guard = scheduler.lock().unwrap();
                    scheduler_guard.needs_update(Duration::from_secs(5))
                };

                if needs_update {
                    let stats = path_stats.read().await;
                    let mut scheduler = scheduler.lock().unwrap();
                    scheduler.update_paths(&stats);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_multipath_manager_creation() {
        let config = MultipathConfig::default();
        let manager = MultipathManager::new(config);
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_paths, 0);
    }

    #[tokio::test]
    async fn test_add_remove_path() {
        let config = MultipathConfig::default();
        let manager = MultipathManager::new(config);

        // Add path
        manager.add_path(1).await.expect("Failed to add path");
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_paths, 1);

        // Remove path
        manager.remove_path(1, "Test removal".to_string()).await.expect("Failed to remove path");
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_paths, 0);
    }

    #[tokio::test]
    async fn test_packet_send_receive() {
        let config = MultipathConfig::default();
        let manager = MultipathManager::new(config);

        // Add path
        manager.add_path(1).await.expect("Failed to add path");

        // Send packet
        let data = vec![1, 2, 3, 4, 5];
        let sent_packet = manager.send_packet(data.clone()).await.expect("Failed to send packet");
        assert_eq!(sent_packet.path_id, 1);
        assert_eq!(sent_packet.data, data);

        // Receive packet (same packet for testing)
        let received_packets = manager.receive_packet(sent_packet).await.expect("Failed to receive packet");
        assert_eq!(received_packets.len(), 1);
        assert_eq!(received_packets[0].data, data);
    }
}
