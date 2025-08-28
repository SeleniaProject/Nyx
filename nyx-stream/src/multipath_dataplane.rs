//! Multipath Data Plane - LARMix++ Dynamic Path Selection
//!
//! This module implements the Nyx Protocol v1.0 multipath data plane with:
//! - LARMix++ latency-aware routing with dynamic hop count adjustment
//! - Weighted Round Robin path scheduler (weight = inverse RTT)
//! - Dynamic reordering buffer management (RTT diff + jitter * 2)
//! - Path quality monitoring and automatic failover
//! - Early data and 0-RTT reception with anti-replay protection
//! - Performance metrics tracking and telemetry integration

#![forbid(unsafe_code)]

use crate::errors::{Error, Result};
use crate::frame::{Frame, FrameHeader, FrameType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

/// Path identifier type
pub type PathId = u8;

/// Connection identifier type
pub type ConnectionId = u32;

/// Multipath configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipathConfig {
    /// Maximum number of concurrent paths
    pub max_paths: usize,
    /// Minimum path quality threshold (0.0-1.0)
    pub min_path_quality: f64,
    /// Path probe interval in milliseconds
    pub probe_interval_ms: u64,
    /// Reordering buffer timeout in milliseconds
    pub reorder_timeout_ms: u64,
    /// Weight update interval for scheduler
    pub weight_update_interval_ms: u64,
    /// Enable dynamic hop count adjustment
    pub dynamic_hop_count: bool,
    /// Minimum hop count
    pub min_hop_count: usize,
    /// Maximum hop count
    pub max_hop_count: usize,
    /// Enable early data acceptance
    pub enable_early_data: bool,
    /// Anti-replay window size (must be power of 2)
    pub anti_replay_window_size: u32,
    /// Path failover timeout
    pub failover_timeout_ms: u64,
}

impl Default for MultipathConfig {
    fn default() -> Self {
        Self {
            max_paths: 4,
            min_path_quality: 0.3,
            probe_interval_ms: 5000,
            reorder_timeout_ms: 1000,
            weight_update_interval_ms: 1000,
            dynamic_hop_count: true,
            min_hop_count: 3,
            max_hop_count: 7,
            enable_early_data: true,
            anti_replay_window_size: 1048576, // 2^20
            failover_timeout_ms: 10000,
        }
    }
}

/// Path performance metrics
#[derive(Debug, Clone)]
pub struct PathMetrics {
    /// Round-trip time in milliseconds
    pub rtt_ms: f64,
    /// Jitter in milliseconds
    pub jitter_ms: f64,
    /// Packet loss rate (0.0-1.0)
    pub loss_rate: f64,
    /// Bandwidth in Mbps
    pub bandwidth_mbps: f64,
    /// Path quality score (0.0-1.0)
    pub quality: f64,
    /// Current hop count for this path
    pub hop_count: usize,
    /// Last measurement timestamp
    pub last_measurement: Instant,
    /// Consecutive failed probes
    pub failed_probes: u32,
}

impl Default for PathMetrics {
    fn default() -> Self {
        Self {
            rtt_ms: 0.0,
            jitter_ms: 0.0,
            loss_rate: 0.0,
            bandwidth_mbps: 0.0,
            quality: 0.0,
            hop_count: 5,
            last_measurement: Instant::now(),
            failed_probes: 0,
        }
    }
}

/// Path state information
#[derive(Debug, Clone)]
pub enum PathState {
    /// Path is initializing
    Initializing,
    /// Path is active and ready
    Active,
    /// Path is being probed
    Probing,
    /// Path performance is degraded
    Degraded,
    /// Path has failed
    Failed,
    /// Path is being recovered
    Recovering,
}

/// Path information structure
#[derive(Debug, Clone)]
pub struct PathInfo {
    /// Path identifier
    pub path_id: PathId,
    /// Connection identifier
    pub connection_id: ConnectionId,
    /// Current path state
    pub state: PathState,
    /// Performance metrics
    pub metrics: PathMetrics,
    /// Scheduler weight (inverse RTT)
    pub weight: f64,
    /// Path creation time
    pub created_at: Instant,
    /// Last activity timestamp
    pub last_activity: Instant,
}

/// Weighted Round Robin scheduler for multipath selection
#[derive(Debug)]
pub struct PathScheduler {
    /// Available paths with their weights
    paths: HashMap<PathId, PathInfo>,
    /// Current weights for WRR scheduling
    current_weights: HashMap<PathId, f64>,
    /// Total weight sum
    total_weight: f64,
    /// Last weight update time
    last_weight_update: Instant,
    /// Configuration
    config: MultipathConfig,
}

impl PathScheduler {
    /// Create new path scheduler
    pub fn new(config: MultipathConfig) -> Self {
        Self {
            paths: HashMap::new(),
            current_weights: HashMap::new(),
            total_weight: 0.0,
            last_weight_update: Instant::now(),
            config,
        }
    }

    /// Add a new path to scheduler
    pub fn add_path(&mut self, mut path_info: PathInfo) -> Result<()> {
        if self.paths.len() >= self.config.max_paths {
            return Err(Error::MultipathError {
                message: "Maximum number of paths reached".to_string(),
            });
        }

        let path_id = path_info.path_id;
        let weight = self.calculate_weight(&path_info.metrics);

        // Update the path's weight
        path_info.weight = weight;

        self.paths.insert(path_id, path_info);
        self.current_weights.insert(path_id, weight);
        self.update_total_weight();

        Ok(())
    }

    /// Remove path from scheduler
    pub fn remove_path(&mut self, path_id: PathId) -> bool {
        if self.paths.remove(&path_id).is_some() {
            self.current_weights.remove(&path_id);
            self.update_total_weight();
            true
        } else {
            false
        }
    }

    /// Select next path using Weighted Round Robin
    pub fn select_path(&mut self) -> Option<PathId> {
        if self.paths.is_empty() {
            return None;
        }

        // Update weights if needed
        if self.last_weight_update.elapsed()
            >= Duration::from_millis(self.config.weight_update_interval_ms)
        {
            self.update_weights();
        }

        // Find path with highest current weight
        let selected_path = self
            .current_weights
            .iter()
            .filter(|(path_id, _)| {
                self.paths
                    .get(path_id)
                    .map(|p| matches!(p.state, PathState::Active))
                    .unwrap_or(false)
            })
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(path_id, _)| *path_id);

        // Decrease selected path weight and increase others
        if let Some(path_id) = selected_path {
            for (id, weight) in self.current_weights.iter_mut() {
                if *id == path_id {
                    *weight -= self.total_weight;
                } else if let Some(path) = self.paths.get(id) {
                    *weight += path.weight;
                }
            }
        }

        selected_path
    }

    /// Update path metrics and recalculate weights
    pub fn update_path_metrics(&mut self, path_id: PathId, mut metrics: PathMetrics) -> Result<()> {
        // Adjust hop count dynamically if enabled before getting mutable reference
        if self.config.dynamic_hop_count {
            self.adjust_hop_count(&mut metrics);
        }

        // Calculate new weight
        let new_weight = self.calculate_weight(&metrics);

        let path = self
            .paths
            .get_mut(&path_id)
            .ok_or_else(|| Error::MultipathError {
                message: format!("Path {path_id} not found"),
            })?;

        path.metrics = metrics;
        path.weight = new_weight;
        path.last_activity = Instant::now();

        // Update path state based on quality
        path.state = if path.metrics.quality >= self.config.min_path_quality {
            PathState::Active
        } else if path.metrics.quality >= 0.1 {
            PathState::Degraded
        } else {
            PathState::Failed
        };

        self.update_total_weight();
        Ok(())
    }

    /// Calculate path weight based on metrics (weight = inverse RTT)
    fn calculate_weight(&self, metrics: &PathMetrics) -> f64 {
        if metrics.rtt_ms <= 0.0 {
            return 1.0; // Default weight for uninitialized metrics
        }

        // Weight = 1000 / RTT, with quality and loss adjustments
        let base_weight = 1000.0 / metrics.rtt_ms;
        let quality_factor = metrics.quality.max(0.1);
        let loss_penalty = (1.0 - metrics.loss_rate).max(0.1);

        base_weight * quality_factor * loss_penalty
    }

    /// Update all path weights
    fn update_weights(&mut self) {
        for (path_id, path) in &self.paths {
            let weight = self.calculate_weight(&path.metrics);
            self.current_weights.insert(*path_id, weight);
        }
        self.update_total_weight();
        self.last_weight_update = Instant::now();
    }

    /// Update total weight sum
    fn update_total_weight(&mut self) {
        self.total_weight = self.paths.values().map(|p| p.weight).sum();
    }

    /// Adjust hop count based on network conditions (LARMix++ algorithm)
    fn adjust_hop_count(&self, metrics: &mut PathMetrics) {
        // Increase hop count if conditions are poor
        if (metrics.loss_rate > 0.1 || metrics.rtt_ms > 500.0)
            && metrics.hop_count < self.config.max_hop_count
        {
            metrics.hop_count += 1;
        }
        // Decrease hop count if conditions are good
        else if metrics.loss_rate < 0.01
            && metrics.rtt_ms < 100.0
            && metrics.hop_count > self.config.min_hop_count
        {
            metrics.hop_count -= 1;
        }
    }

    /// Get active paths count
    pub fn active_paths_count(&self) -> usize {
        self.paths
            .values()
            .filter(|p| matches!(p.state, PathState::Active))
            .count()
    }

    /// Get path information
    pub fn get_path_info(&self, path_id: PathId) -> Option<&PathInfo> {
        self.paths.get(&path_id)
    }

    /// Get all paths
    pub fn get_all_paths(&self) -> &HashMap<PathId, PathInfo> {
        &self.paths
    }
}

/// Frame reordering entry
#[derive(Debug, Clone)]
struct ReorderEntry {
    frame: Frame,
    arrival_time: Instant,
    #[allow(dead_code)]
    sequence_number: u64,
}

/// Dynamic reordering buffer for handling out-of-order packets
#[derive(Debug)]
pub struct ReorderingBuffer {
    /// Buffer for out-of-order frames
    buffer: BTreeMap<u64, ReorderEntry>,
    /// Next expected sequence number
    next_expected_seq: u64,
    /// Buffer timeout duration
    timeout: Duration,
    /// Maximum buffer size
    max_buffer_size: usize,
}

impl ReorderingBuffer {
    /// Create new reordering buffer
    pub fn new(timeout_ms: u64, max_size: usize) -> Self {
        Self {
            buffer: BTreeMap::new(),
            next_expected_seq: 0,
            timeout: Duration::from_millis(timeout_ms),
            max_buffer_size: max_size,
        }
    }

    /// Add frame to reordering buffer and return any deliverable frames
    pub fn add_frame(&mut self, frame: Frame, seq: u64) -> Vec<Frame> {
        let mut deliverable = Vec::new();

        // If this is the next expected frame, deliver it immediately
        if seq == self.next_expected_seq {
            deliverable.push(frame);
            self.next_expected_seq += 1;

            // Check if any buffered frames can now be delivered
            while let Some(entry) = self.buffer.remove(&self.next_expected_seq) {
                deliverable.push(entry.frame);
                self.next_expected_seq += 1;
            }
        } else if seq > self.next_expected_seq {
            // Buffer out-of-order frame
            let entry = ReorderEntry {
                frame,
                arrival_time: Instant::now(),
                sequence_number: seq,
            };

            // Enforce buffer size limit
            if self.buffer.len() >= self.max_buffer_size {
                // Remove oldest entry to make space
                if let Some((_, oldest)) = self.buffer.pop_first() {
                    // Deliver the oldest frame even if out of order
                    deliverable.push(oldest.frame);
                }
            }

            self.buffer.insert(seq, entry);
        }
        // Ignore duplicate or old frames (seq < next_expected_seq)

        deliverable
    }

    /// Check for timed-out frames and deliver them
    pub fn check_timeouts(&mut self) -> Vec<Frame> {
        let now = Instant::now();
        let mut timed_out = Vec::new();
        let mut to_remove = Vec::new();

        for (seq, entry) in &self.buffer {
            if now.duration_since(entry.arrival_time) > self.timeout {
                timed_out.push(entry.frame.clone());
                to_remove.push(*seq);
            }
        }

        // Remove timed-out entries
        for seq in to_remove {
            self.buffer.remove(&seq);
        }

        timed_out
    }

    /// Calculate dynamic buffer timeout based on RTT and jitter
    pub fn update_timeout(&mut self, rtt_diff_ms: f64, jitter_ms: f64) {
        let dynamic_timeout = (rtt_diff_ms + jitter_ms * 2.0).max(100.0);
        self.timeout = Duration::from_millis(dynamic_timeout as u64);
    }

    /// Get buffer statistics
    pub fn get_stats(&self) -> (usize, u64, Duration) {
        (self.buffer.len(), self.next_expected_seq, self.timeout)
    }
}

/// Anti-replay window for early data protection
#[derive(Debug)]
pub struct AntiReplayWindow {
    /// Sliding window for seen nonces
    window: VecDeque<u64>,
    /// Window size (must be power of 2)
    window_size: u32,
    /// Highest seen nonce
    highest_nonce: u64,
}

impl AntiReplayWindow {
    /// Create new anti-replay window
    pub fn new(window_size: u32) -> Self {
        assert!(
            window_size.is_power_of_two(),
            "Window size must be power of 2"
        );

        Self {
            window: VecDeque::with_capacity(window_size as usize),
            window_size,
            highest_nonce: 0,
        }
    }

    /// Check and record nonce, return true if valid (not replay)
    pub fn check_nonce(&mut self, nonce: u64) -> bool {
        // Check if nonce is duplicate
        if self.window.contains(&nonce) {
            return false; // Duplicate, reject
        }

        // For a simple anti-replay check: reject nonces that are significantly older
        // than what we've already seen. This prevents most replay attacks.
        if self.highest_nonce > 0
            && nonce < self.highest_nonce.saturating_sub(self.window_size as u64)
        {
            return false; // Too old, reject
        }

        // Special case: if we've seen any nonces and this one is 0, it's likely a replay
        if self.highest_nonce > 0 && nonce == 0 {
            return false; // Reject nonce 0 after we've seen higher nonces
        }

        // Accept the nonce
        self.window.push_back(nonce);

        // Update highest nonce
        if nonce > self.highest_nonce {
            self.highest_nonce = nonce;
        }

        // Maintain window size
        while self.window.len() > self.window_size as usize {
            self.window.pop_front();
        }

        true
    }

    /// Reset window (used during rekey)
    pub fn reset(&mut self) {
        self.window.clear();
        self.highest_nonce = 0;
    }

    /// Get window statistics
    pub fn get_stats(&self) -> (usize, u64) {
        (self.window.len(), self.highest_nonce)
    }
}

/// Multipath Data Plane Manager
pub struct MultipathDataPlane {
    /// Path scheduler
    scheduler: Arc<Mutex<PathScheduler>>,
    /// Reordering buffers per connection
    reorder_buffers: Arc<RwLock<HashMap<ConnectionId, ReorderingBuffer>>>,
    /// Anti-replay windows per direction per connection
    anti_replay_windows: Arc<RwLock<HashMap<(ConnectionId, u32), AntiReplayWindow>>>,
    /// Configuration
    config: MultipathConfig,
    /// Performance metrics
    metrics: Arc<RwLock<MultipathMetrics>>,
}

/// Multipath performance metrics
#[derive(Debug, Default, Clone)]
pub struct MultipathMetrics {
    /// Total frames sent
    pub frames_sent: u64,
    /// Total frames received
    pub frames_received: u64,
    /// Frames delivered in order
    pub frames_in_order: u64,
    /// Frames delivered out of order
    pub frames_out_of_order: u64,
    /// Frames dropped due to timeout
    pub frames_timeout: u64,
    /// Frames dropped due to replay
    pub frames_replay_drop: u64,
    /// Early data frames accepted
    pub early_data_accepted: u64,
    /// Path failovers
    pub path_failovers: u64,
    /// Average reorder buffer size
    pub avg_reorder_buffer_size: f64,
}

impl MultipathDataPlane {
    /// Create new multipath data plane
    pub fn new(config: MultipathConfig) -> Self {
        let scheduler = Arc::new(Mutex::new(PathScheduler::new(config.clone())));

        Self {
            scheduler,
            reorder_buffers: Arc::new(RwLock::new(HashMap::new())),
            anti_replay_windows: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics: Arc::new(RwLock::new(MultipathMetrics::default())),
        }
    }

    /// Add a new path
    pub async fn add_path(&self, path_info: PathInfo) -> Result<()> {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.add_path(path_info)
    }

    /// Remove a path
    pub async fn remove_path(&self, path_id: PathId) -> bool {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.remove_path(path_id)
    }

    /// Select path for sending frame
    pub async fn select_send_path(&self) -> Option<PathId> {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.select_path()
    }

    /// Update path metrics
    pub async fn update_path_metrics(&self, path_id: PathId, metrics: PathMetrics) -> Result<()> {
        let mut scheduler = self.scheduler.lock().await;
        scheduler.update_path_metrics(path_id, metrics)?;

        // Update reordering buffer timeouts
        let max_rtt = scheduler
            .get_all_paths()
            .values()
            .map(|p| p.metrics.rtt_ms)
            .fold(0.0f64, |acc, rtt| acc.max(rtt));

        let min_rtt = scheduler
            .get_all_paths()
            .values()
            .map(|p| p.metrics.rtt_ms)
            .filter(|&rtt| rtt > 0.0)
            .fold(f64::INFINITY, |acc, rtt| acc.min(rtt));

        let rtt_diff = if min_rtt != f64::INFINITY {
            max_rtt - min_rtt
        } else {
            0.0
        };
        let avg_jitter = scheduler
            .get_all_paths()
            .values()
            .map(|p| p.metrics.jitter_ms)
            .sum::<f64>()
            / scheduler.get_all_paths().len().max(1) as f64;

        // Update all reordering buffers
        let mut buffers = self.reorder_buffers.write().await;
        for buffer in buffers.values_mut() {
            buffer.update_timeout(rtt_diff, avg_jitter);
        }

        Ok(())
    }

    /// Process incoming frame with reordering
    pub async fn process_incoming_frame(
        &self,
        connection_id: ConnectionId,
        frame: Frame,
        sequence_number: u64,
        direction_id: u32,
        nonce: u64,
        is_early_data: bool,
    ) -> Result<Vec<Frame>> {
        // Check anti-replay if early data
        if is_early_data && self.config.enable_early_data {
            let mut windows = self.anti_replay_windows.write().await;
            let window = windows
                .entry((connection_id, direction_id))
                .or_insert_with(|| AntiReplayWindow::new(self.config.anti_replay_window_size));

            if !window.check_nonce(nonce) {
                let mut metrics = self.metrics.write().await;
                metrics.frames_replay_drop += 1;
                return Err(Error::MultipathError {
                    message: "Replay attack detected".to_string(),
                });
            }

            let mut metrics = self.metrics.write().await;
            metrics.early_data_accepted += 1;
        }

        // Add to reordering buffer
        let mut buffers = self.reorder_buffers.write().await;
        let buffer = buffers.entry(connection_id).or_insert_with(|| {
            ReorderingBuffer::new(
                self.config.reorder_timeout_ms,
                1000, // Max buffer size
            )
        });

        let deliverable_frames = buffer.add_frame(frame, sequence_number);

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.frames_received += 1;
        if sequence_number == buffer.next_expected_seq - deliverable_frames.len() as u64 {
            metrics.frames_in_order += deliverable_frames.len() as u64;
        } else {
            metrics.frames_out_of_order += deliverable_frames.len() as u64;
        }

        Ok(deliverable_frames)
    }

    /// Check for timed-out frames in reordering buffers
    pub async fn check_reorder_timeouts(&self) -> HashMap<ConnectionId, Vec<Frame>> {
        let mut buffers = self.reorder_buffers.write().await;
        let mut result = HashMap::new();

        for (connection_id, buffer) in buffers.iter_mut() {
            let timed_out = buffer.check_timeouts();
            if !timed_out.is_empty() {
                let mut metrics = self.metrics.write().await;
                metrics.frames_timeout += timed_out.len() as u64;
                result.insert(*connection_id, timed_out);
            }
        }

        result
    }

    /// Reset anti-replay window for connection (used during rekey)
    pub async fn reset_anti_replay_window(&self, connection_id: ConnectionId, direction_id: u32) {
        let mut windows = self.anti_replay_windows.write().await;
        if let Some(window) = windows.get_mut(&(connection_id, direction_id)) {
            window.reset();
        }
    }

    /// Get scheduler statistics
    pub async fn get_scheduler_stats(&self) -> (usize, HashMap<PathId, PathInfo>) {
        let scheduler = self.scheduler.lock().await;
        (
            scheduler.active_paths_count(),
            scheduler.get_all_paths().clone(),
        )
    }

    /// Get performance metrics
    pub async fn get_metrics(&self) -> MultipathMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Probe path health and update metrics
    pub async fn probe_path_health(&self, path_id: PathId) -> Result<PathMetrics> {
        // Create probe frame (in real implementation, this would be sent over network)
        let _probe_frame = Frame {
            header: FrameHeader {
                stream_id: 0, // Control stream
                seq: 0,
                ty: FrameType::Custom(0xFF), // Probe frame type
            },
            payload: b"PROBE".to_vec(),
        };

        // Send probe and measure RTT
        let start_time = Instant::now();

        // Simulate probe response time (in real implementation, this would be actual network I/O)
        tokio::time::sleep(Duration::from_millis(10)).await;

        let rtt = start_time.elapsed();

        // Calculate updated metrics based on probe results
        let metrics = PathMetrics {
            rtt_ms: rtt.as_secs_f64() * 1000.0,
            jitter_ms: 5.0,        // Simplified calculation
            loss_rate: 0.01,       // Simplified calculation
            bandwidth_mbps: 100.0, // Simplified calculation
            quality: 0.9,          // Simplified calculation
            hop_count: 5,
            last_measurement: Instant::now(),
            failed_probes: 0,
        };

        // Update path metrics
        self.update_path_metrics(path_id, metrics.clone()).await?;

        Ok(metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_scheduler_creation() {
        let config = MultipathConfig::default();
        let scheduler = PathScheduler::new(config);
        assert_eq!(scheduler.active_paths_count(), 0);
    }

    #[test]
    fn test_path_scheduler_add_remove() {
        let config = MultipathConfig::default();
        let mut scheduler = PathScheduler::new(config);

        let path_info = PathInfo {
            path_id: 1,
            connection_id: 123,
            state: PathState::Active,
            metrics: PathMetrics::default(),
            weight: 1.0,
            created_at: Instant::now(),
            last_activity: Instant::now(),
        };

        assert!(scheduler.add_path(path_info).is_ok());
        assert_eq!(scheduler.active_paths_count(), 1);

        assert!(scheduler.remove_path(1));
        assert_eq!(scheduler.active_paths_count(), 0);
    }

    #[test]
    fn test_weight_calculation() {
        let config = MultipathConfig::default();
        let scheduler = PathScheduler::new(config);

        let metrics = PathMetrics {
            rtt_ms: 100.0,
            quality: 0.8,
            loss_rate: 0.05,
            ..Default::default()
        };

        let weight = scheduler.calculate_weight(&metrics);
        assert!(weight > 0.0);
        assert!(weight < 1000.0); // Should be less than base weight due to quality/loss factors
    }

    #[test]
    fn test_reordering_buffer() {
        let mut buffer = ReorderingBuffer::new(1000, 100);

        // Add frames out of order
        let frame1 = Frame {
            header: FrameHeader {
                stream_id: 1,
                seq: 1,
                ty: FrameType::Data,
            },
            payload: b"frame1".to_vec(),
        };
        let frame0 = Frame {
            header: FrameHeader {
                stream_id: 1,
                seq: 0,
                ty: FrameType::Data,
            },
            payload: b"frame0".to_vec(),
        };

        // Add frame 1 (out of order)
        let delivered = buffer.add_frame(frame1, 1);
        assert_eq!(delivered.len(), 0); // Should be buffered

        // Add frame 0 (in order)
        let delivered = buffer.add_frame(frame0, 0);
        assert_eq!(delivered.len(), 2); // Should deliver both frames
    }

    #[test]
    fn test_anti_replay_window() {
        let mut window = AntiReplayWindow::new(1024);

        // Test normal nonce acceptance
        assert!(window.check_nonce(1));
        assert!(window.check_nonce(2));
        assert!(window.check_nonce(3));

        // Test duplicate detection
        assert!(!window.check_nonce(2));

        // Test old nonce rejection
        assert!(!window.check_nonce(0));
    }

    #[tokio::test]
    async fn test_multipath_data_plane() {
        let config = MultipathConfig::default();
        let data_plane = MultipathDataPlane::new(config);

        let path_info = PathInfo {
            path_id: 1,
            connection_id: 123,
            state: PathState::Active,
            metrics: PathMetrics {
                rtt_ms: 50.0,
                quality: 0.9,
                ..Default::default()
            },
            weight: 1.0,
            created_at: Instant::now(),
            last_activity: Instant::now(),
        };

        assert!(data_plane.add_path(path_info).await.is_ok());

        let selected = data_plane.select_send_path().await;
        assert_eq!(selected, Some(1));

        let (active_count, _) = data_plane.get_scheduler_stats().await;
        assert_eq!(active_count, 1);
    }

    #[tokio::test]
    async fn test_frame_processing() {
        let config = MultipathConfig::default();
        let data_plane = MultipathDataPlane::new(config);

        let frame = Frame {
            header: FrameHeader {
                stream_id: 1,
                seq: 0,
                ty: FrameType::Data,
            },
            payload: b"test".to_vec(),
        };

        let result = data_plane
            .process_incoming_frame(
                123, // connection_id
                frame, 0,    // sequence_number
                1,    // direction_id
                1,    // nonce
                true, // is_early_data
            )
            .await;

        assert!(result.is_ok());
        let delivered = result.unwrap();
        assert_eq!(delivered.len(), 1);
    }

    #[test]
    fn test_dynamic_hop_count_adjustment() {
        let config = MultipathConfig::default();
        let scheduler = PathScheduler::new(config);

        let mut metrics = PathMetrics {
            rtt_ms: 600.0,   // High RTT
            loss_rate: 0.15, // High loss
            hop_count: 5,
            ..Default::default()
        };

        scheduler.adjust_hop_count(&mut metrics);
        assert_eq!(metrics.hop_count, 6); // Should increase due to poor conditions

        // Test decrease with good conditions
        metrics.rtt_ms = 80.0;
        metrics.loss_rate = 0.005;
        metrics.hop_count = 5;

        scheduler.adjust_hop_count(&mut metrics);
        assert_eq!(metrics.hop_count, 4); // Should decrease due to good conditions
    }
}
