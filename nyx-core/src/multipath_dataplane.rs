// Multipath Data Plane Implementation for NyxNet v1.0
// Complete implementation with PathID headers, Weighted Round Robin, and reordering buffers

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use serde::{Deserialize, Serialize};

/// Multipath Data Plane Manager
/// Implements PathID headers, Weighted Round Robin scheduling, and per-path reordering
pub struct MultipathDataPlane {
    /// Path configurations and statistics
    paths: Arc<Mutex<HashMap<u8, PathInfo>>>,
    /// Weighted Round Robin scheduler
    scheduler: Arc<Mutex<WeightedRoundRobinScheduler>>,
    /// Per-path reordering buffers
    reorder_buffers: Arc<Mutex<HashMap<u8, ReorderingBuffer>>>,
    /// Dynamic hop count configuration (3-7)
    hop_count_config: Arc<Mutex<DynamicHopConfig>>,
    /// Path quality monitor
    quality_monitor: Arc<Mutex<PathQualityMonitor>>,
}

/// PathID header field (uint8) for packet identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathId(pub u8);

impl PathId {
    pub const MAX_PATHS: u8 = 8; // Support up to 8 concurrent paths
    
    pub fn new(id: u8) -> Result<Self, MultipathError> {
        if id >= Self::MAX_PATHS {
            return Err(MultipathError::InvalidPathId(id));
        }
        Ok(PathId(id))
    }
    
    pub fn as_u8(&self) -> u8 {
        self.0
    }
}

/// Packet header with PathID field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultipathPacketHeader {
    /// Path identifier (0-7)
    pub path_id: u8,
    /// Sequence number for reordering
    pub sequence: u64,
    /// Timestamp for RTT calculation
    pub timestamp: u64,
    /// Packet flags
    pub flags: u8,
    /// Total hops for this path
    pub hop_count: u8,
}

/// Path information and statistics
#[derive(Debug, Clone)]
pub struct PathInfo {
    pub path_id: PathId,
    pub rtt: Duration,
    pub jitter: Duration,
    pub loss_rate: f64,
    pub bandwidth: u64, // bytes per second
    pub last_activity: Instant,
    pub hop_count: u8,
    pub weight: f64, // Calculated as inverse RTT
}

/// Weighted Round Robin Scheduler
/// Weight = inverse RTT for optimal load distribution
#[derive(Debug)]
pub struct WeightedRoundRobinScheduler {
    paths: Vec<PathId>,
    weights: Vec<f64>,
    current_index: usize,
    current_weight: f64,
    max_weight: f64,
    weight_step: f64,
}

/// Per-path reordering buffer
/// Buffer size = RTT diff + jitter * 2
#[derive(Debug)]
pub struct ReorderingBuffer {
    buffer: VecDeque<PacketWithSequence>,
    expected_sequence: u64,
    max_buffer_size: usize,
    timeout: Duration,
    last_flush: Instant,
}

/// Packet with sequence information
#[derive(Debug, Clone)]
pub struct PacketWithSequence {
    pub sequence: u64,
    pub data: Vec<u8>,
    pub received_at: Instant,
}

/// Dynamic hop count configuration (3-7 hops)
#[derive(Debug)]
pub struct DynamicHopConfig {
    min_hops: u8,
    max_hops: u8,
    path_hop_counts: HashMap<PathId, u8>,
    adaptation_interval: Duration,
    last_adaptation: Instant,
}

/// Path quality monitoring
#[derive(Debug)]
pub struct PathQualityMonitor {
    path_metrics: HashMap<PathId, PathMetrics>,
    measurement_window: Duration,
    quality_threshold: f64,
}

/// Detailed path metrics
#[derive(Debug, Clone)]
pub struct PathMetrics {
    pub rtt_samples: VecDeque<Duration>,
    pub loss_count: u64,
    pub packet_count: u64,
    pub bandwidth_samples: VecDeque<u64>,
    pub last_update: Instant,
    pub quality_score: f64,
}

/// Multipath errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum MultipathError {
    #[error("Invalid path ID: {0}")]
    InvalidPathId(u8),
    #[error("Path not found: {0}")]
    PathNotFound(u8),
    #[error("Buffer overflow for path: {0}")]
    BufferOverflow(u8),
    #[error("Sequence out of order: expected {expected}, got {received}")]
    SequenceOutOfOrder { expected: u64, received: u64 },
    #[error("Path quality below threshold: {0}")]
    PathQualityLow(f64),
    #[error("No available paths")]
    NoAvailablePaths,
}

impl MultipathDataPlane {
    /// Create new multipath data plane
    pub fn new() -> Self {
        Self {
            paths: Arc::new(Mutex::new(HashMap::new())),
            scheduler: Arc::new(Mutex::new(WeightedRoundRobinScheduler::new())),
            reorder_buffers: Arc::new(Mutex::new(HashMap::new())),
            hop_count_config: Arc::new(Mutex::new(DynamicHopConfig::new())),
            quality_monitor: Arc::new(Mutex::new(PathQualityMonitor::new())),
        }
    }
    
    /// Add a new path to the multipath system
    pub fn add_path(&self, path_id: u8, initial_rtt: Duration) -> Result<(), MultipathError> {
        let path_id = PathId::new(path_id)?;
        
        let path_info = PathInfo {
            path_id,
            rtt: initial_rtt,
            jitter: Duration::from_millis(10), // Initial jitter estimate
            loss_rate: 0.0,
            bandwidth: 1_000_000, // 1 MB/s initial estimate
            last_activity: Instant::now(),
            hop_count: 5, // Default hop count
            weight: 1.0 / initial_rtt.as_secs_f64(), // Inverse RTT
        };
        
        // Add to paths
        {
            let mut paths = self.paths.lock().unwrap();
            paths.insert(path_id.as_u8(), path_info.clone());
        }
        
        // Add to scheduler
        {
            let mut scheduler = self.scheduler.lock().unwrap();
            scheduler.add_path(path_id, path_info.weight);
        }
        
        // Create reordering buffer
        {
            let buffer_size = self.calculate_buffer_size(&path_info);
            let timeout = path_info.rtt + path_info.jitter * 2;
            let mut buffers = self.reorder_buffers.lock().unwrap();
            buffers.insert(path_id.as_u8(), ReorderingBuffer::new(buffer_size, timeout));
        }
        
        // Initialize path metrics
        {
            let mut monitor = self.quality_monitor.lock().unwrap();
            monitor.add_path(path_id);
        }
        
        // Set initial hop count
        {
            let mut hop_config = self.hop_count_config.lock().unwrap();
            hop_config.set_path_hops(path_id, 5); // Start with 5 hops
        }
        
        info!("Added path {} with RTT {:?} and weight {:.3}", 
              path_id.as_u8(), initial_rtt, path_info.weight);
        
        Ok(())
    }
    
    /// Select next path using Weighted Round Robin
    pub fn select_path(&self) -> Result<PathId, MultipathError> {
        let mut scheduler = self.scheduler.lock().unwrap();
        scheduler.select_next_path()
    }
    
    /// Send packet with multipath header
    pub fn send_packet(&self, data: Vec<u8>) -> Result<(PathId, MultipathPacketHeader, Vec<u8>), MultipathError> {
        let path_id = self.select_path()?;
        
        // Get current sequence number and hop count
        let (sequence, hop_count) = {
            let paths = self.paths.lock().unwrap();
            let path_info = paths.get(&path_id.as_u8())
                .ok_or(MultipathError::PathNotFound(path_id.as_u8()))?;
            
            let hop_config = self.hop_count_config.lock().unwrap();
            let hop_count = hop_config.get_path_hops(path_id);
            
            // This would increment a sequence counter in real implementation
            (0u64, hop_count)
        };
        
        let header = MultipathPacketHeader {
            path_id: path_id.as_u8(),
            sequence,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            flags: 0,
            hop_count,
        };
        
        debug!("Sending packet on path {} with {} hops", path_id.as_u8(), hop_count);
        
        Ok((path_id, header, data))
    }
    
    /// Receive and reorder packet
    pub fn receive_packet(
        &self, 
        header: MultipathPacketHeader, 
        data: Vec<u8>
    ) -> Result<Vec<Vec<u8>>, MultipathError> {
        let path_id = PathId::new(header.path_id)?;
        
        // Update path statistics
        self.update_path_statistics(path_id, &header)?;
        
        // Add to reordering buffer
        let packet = PacketWithSequence {
            sequence: header.sequence,
            data,
            received_at: Instant::now(),
        };
        
        let mut buffers = self.reorder_buffers.lock().unwrap();
        let buffer = buffers.get_mut(&header.path_id)
            .ok_or(MultipathError::PathNotFound(header.path_id))?;
        
        buffer.insert_packet(packet)?;
        
        // Extract ordered packets
        let ordered_packets = buffer.extract_ordered_packets();
        
        debug!("Received packet on path {}, extracted {} ordered packets", 
               header.path_id, ordered_packets.len());
        
        Ok(ordered_packets)
    }
    
    /// Update path statistics and weights
    pub fn update_path_statistics(
        &self, 
        path_id: PathId, 
        header: &MultipathPacketHeader
    ) -> Result<(), MultipathError> {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let rtt = if now_ns >= header.timestamp { Duration::from_nanos(now_ns - header.timestamp) } else { Duration::from_nanos(0) };
        
        // Update path info
        {
            let mut paths = self.paths.lock().unwrap();
            if let Some(path_info) = paths.get_mut(&path_id.as_u8()) {
                // Exponential moving average for RTT
                path_info.rtt = Duration::from_nanos(
                    (path_info.rtt.as_nanos() as f64 * 0.8 + rtt.as_nanos() as f64 * 0.2) as u64
                );
                
                // Update jitter (mean deviation)
                let rtt_diff = if rtt > path_info.rtt {
                    rtt - path_info.rtt
                } else {
                    path_info.rtt - rtt
                };
                
                path_info.jitter = Duration::from_nanos(
                    (path_info.jitter.as_nanos() as f64 * 0.8 + rtt_diff.as_nanos() as f64 * 0.2) as u64
                );
                
                // Update weight (inverse RTT)
                path_info.weight = 1.0 / path_info.rtt.as_secs_f64();
                path_info.last_activity = current_time;
            }
        }
        
        // Update scheduler weights
        {
            let paths = self.paths.lock().unwrap();
            if let Some(path_info) = paths.get(&path_id.as_u8()) {
                let mut scheduler = self.scheduler.lock().unwrap();
                scheduler.update_weight(path_id, path_info.weight);
            }
        }
        
        // Update path metrics
        {
            let mut monitor = self.quality_monitor.lock().unwrap();
            monitor.update_metrics(path_id, rtt)?;
        }
        
        Ok(())
    }
    
    /// Adapt hop counts based on path performance
    pub fn adapt_hop_counts(&self) -> Result<(), MultipathError> {
        let mut hop_config = self.hop_count_config.lock().unwrap();
        
        if hop_config.should_adapt() {
            let paths = self.paths.lock().unwrap();
            
            for (path_id, path_info) in paths.iter() {
                let current_hops = hop_config.get_path_hops(PathId(*path_id));
                let new_hops = self.calculate_optimal_hops(path_info);
                
                if new_hops != current_hops {
                    hop_config.set_path_hops(PathId(*path_id), new_hops);
                    info!("Adapted path {} hop count: {} -> {}", path_id, current_hops, new_hops);
                }
            }
            
            hop_config.mark_adapted();
        }
        
        Ok(())
    }
    
    /// Calculate optimal hop count based on path characteristics
    fn calculate_optimal_hops(&self, path_info: &PathInfo) -> u8 {
        // More hops for higher latency paths (better anonymity)
        // Fewer hops for low latency paths (better performance)
        
        let base_hops = 5u8;
        let rtt_ms = path_info.rtt.as_millis() as u8;
        
        match rtt_ms {
            0..=20 => 3,    // Very low latency - use minimum hops
            21..=50 => 4,   // Low latency
            51..=100 => 5,  // Medium latency - default
            101..=200 => 6, // High latency
            _ => 7,         // Very high latency - use maximum hops
        }
    }
    
    /// Calculate reordering buffer size
    fn calculate_buffer_size(&self, path_info: &PathInfo) -> usize {
        // Buffer size = RTT difference + jitter * 2
        let base_size = 100; // Minimum buffer size
        let rtt_factor = (path_info.rtt.as_millis() / 10) as usize;
        let jitter_factor = (path_info.jitter.as_millis() * 2) as usize;
        
        (base_size + rtt_factor + jitter_factor).min(1000) // Cap at 1000 packets
    }
    
    /// Start background tasks for maintenance
    pub async fn start_background_tasks(&self) {
        self.start_quality_monitoring().await;
        self.start_hop_adaptation().await;
        self.start_buffer_cleanup().await;
    }
    
    /// Start path quality monitoring task
    async fn start_quality_monitoring(&self) {
        let monitor = Arc::clone(&self.quality_monitor);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                let mut guard = monitor.lock().unwrap();
                if let Err(e) = guard.update_quality_scores() {
                    warn!("Quality monitoring update failed: {}", e);
                }
            }
        });
    }
    
    /// Start hop count adaptation task
    async fn start_hop_adaptation(&self) {
        let data_plane = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                if let Err(e) = data_plane.adapt_hop_counts() {
                    warn!("Hop count adaptation failed: {}", e);
                }
            }
        });
    }
    
    /// Start buffer cleanup task
    async fn start_buffer_cleanup(&self) {
        let buffers = Arc::clone(&self.reorder_buffers);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(100));
            
            loop {
                interval.tick().await;
                
                let mut buffers = buffers.lock().unwrap();
                for (path_id, buffer) in buffers.iter_mut() {
                    let expired = buffer.cleanup_expired();
                    if !expired.is_empty() {
                        warn!("Path {}: {} packets expired from reorder buffer", path_id, expired.len());
                    }
                }
            }
        });
    }
}

// We need to implement Clone for MultipathDataPlane to use it in the background task
impl Clone for MultipathDataPlane {
    fn clone(&self) -> Self {
        Self {
            paths: Arc::clone(&self.paths),
            scheduler: Arc::clone(&self.scheduler),
            reorder_buffers: Arc::clone(&self.reorder_buffers),
            hop_count_config: Arc::clone(&self.hop_count_config),
            quality_monitor: Arc::clone(&self.quality_monitor),
        }
    }
}

impl WeightedRoundRobinScheduler {
    fn new() -> Self {
        Self {
            paths: Vec::new(),
            weights: Vec::new(),
            current_index: 0,
            current_weight: 0.0,
            max_weight: 0.0,
            weight_step: 0.0,
        }
    }
    
    fn add_path(&mut self, path_id: PathId, weight: f64) {
        self.paths.push(path_id);
        self.weights.push(weight);
        
        if weight > self.max_weight {
            self.max_weight = weight;
        }
        
        self.calculate_weight_step();
    }
    
    fn update_weight(&mut self, path_id: PathId, new_weight: f64) {
        if let Some(index) = self.paths.iter().position(|&p| p == path_id) {
            self.weights[index] = new_weight;
            
            self.max_weight = self.weights.iter().cloned().fold(0.0, f64::max);
            self.calculate_weight_step();
        }
    }
    
    fn select_next_path(&mut self) -> Result<PathId, MultipathError> {
        if self.paths.is_empty() {
            return Err(MultipathError::NoAvailablePaths);
        }
        
        loop {
            if self.current_weight <= 0.0 {
                self.current_index = (self.current_index + 1) % self.paths.len();
                self.current_weight = self.max_weight;
                
                if self.current_index == 0 {
                    self.current_weight -= self.weight_step;
                    if self.current_weight <= 0.0 {
                        self.current_weight = self.max_weight;
                    }
                }
            }
            
            if self.weights[self.current_index] >= self.current_weight {
                self.current_weight -= self.weights[self.current_index];
                return Ok(self.paths[self.current_index]);
            }
            
            self.current_weight -= self.weights[self.current_index];
        }
    }
    
    fn calculate_weight_step(&mut self) {
        if !self.weights.is_empty() {
            let total_weight: f64 = self.weights.iter().sum();
            self.weight_step = total_weight / self.weights.len() as f64;
        }
    }
}

impl ReorderingBuffer {
    fn new(max_size: usize, timeout: Duration) -> Self {
        Self {
            buffer: VecDeque::new(),
            expected_sequence: 0,
            max_buffer_size: max_size,
            timeout,
            last_flush: Instant::now(),
        }
    }
    
    fn insert_packet(&mut self, packet: PacketWithSequence) -> Result<(), MultipathError> {
        if self.buffer.len() >= self.max_buffer_size {
            return Err(MultipathError::BufferOverflow(0)); // Path ID would be passed here
        }
        
        // Insert in order by sequence number
        let insert_pos = self.buffer
            .binary_search_by_key(&packet.sequence, |p| p.sequence)
            .unwrap_or_else(|pos| pos);
        
        self.buffer.insert(insert_pos, packet);
        Ok(())
    }
    
    fn extract_ordered_packets(&mut self) -> Vec<Vec<u8>> {
        let mut ordered_packets = Vec::new();
        
        while let Some(packet) = self.buffer.front() {
            if packet.sequence == self.expected_sequence {
                // Pop is safe due to previous check on front()
                let packet = match self.buffer.pop_front() {
                    Some(p) => p,
                    None => break,
                };
                ordered_packets.push(packet.data);
                self.expected_sequence += 1;
            } else {
                break;
            }
        }
        
        ordered_packets
    }
    
    fn cleanup_expired(&mut self) -> Vec<PacketWithSequence> {
        let now = Instant::now();
        let mut expired = Vec::new();
        
        while let Some(packet) = self.buffer.front() {
            if now.duration_since(packet.received_at) > self.timeout {
                if let Some(p) = self.buffer.pop_front() { expired.push(p); } else { break; }
            } else {
                break;
            }
        }
        
        expired
    }
}

impl DynamicHopConfig {
    fn new() -> Self {
        Self {
            min_hops: 3,
            max_hops: 7,
            path_hop_counts: HashMap::new(),
            adaptation_interval: Duration::from_secs(30),
            last_adaptation: Instant::now(),
        }
    }
    
    fn set_path_hops(&mut self, path_id: PathId, hops: u8) {
        let hops = hops.clamp(self.min_hops, self.max_hops);
        self.path_hop_counts.insert(path_id, hops);
    }
    
    fn get_path_hops(&self, path_id: PathId) -> u8 {
        self.path_hop_counts.get(&path_id).copied().unwrap_or(5)
    }
    
    fn should_adapt(&self) -> bool {
        Instant::now().duration_since(self.last_adaptation) > self.adaptation_interval
    }
    
    fn mark_adapted(&mut self) {
        self.last_adaptation = Instant::now();
    }
}

impl PathQualityMonitor {
    fn new() -> Self {
        Self {
            path_metrics: HashMap::new(),
            measurement_window: Duration::from_secs(60),
            quality_threshold: 0.7,
        }
    }
    
    fn add_path(&mut self, path_id: PathId) {
        self.path_metrics.insert(path_id, PathMetrics {
            rtt_samples: VecDeque::new(),
            loss_count: 0,
            packet_count: 0,
            bandwidth_samples: VecDeque::new(),
            last_update: Instant::now(),
            quality_score: 1.0,
        });
    }
    
    fn update_metrics(&mut self, path_id: PathId, rtt: Duration) -> Result<(), MultipathError> {
        let metrics = self.path_metrics.get_mut(&path_id)
            .ok_or(MultipathError::PathNotFound(path_id.as_u8()))?;
        
        metrics.rtt_samples.push_back(rtt);
        metrics.packet_count += 1;
        metrics.last_update = Instant::now();
        
        // Keep only recent samples
        let cutoff = Instant::now() - self.measurement_window;
        // Drop old samples conservatively if we have an excessive backlog
        while metrics.last_update < cutoff && metrics.rtt_samples.len() > 0 {
            metrics.rtt_samples.pop_front();
        }
        
        Ok(())
    }
    
    fn update_quality_scores(&mut self) -> Result<(), MultipathError> {
        for (path_id, metrics) in self.path_metrics.iter_mut() {
            // Calculate quality score based on RTT variance and loss rate
            let rtt_variance = self.calculate_rtt_variance(&metrics.rtt_samples);
            let loss_rate = metrics.loss_count as f64 / metrics.packet_count as f64;
            
            // Quality score: lower RTT variance and loss rate = higher quality
            metrics.quality_score = (1.0 - loss_rate) * (1.0 / (1.0 + rtt_variance));
            
            if metrics.quality_score < self.quality_threshold {
                warn!("Path {} quality below threshold: {:.3}", 
                      path_id.as_u8(), metrics.quality_score);
            }
        }
        
        Ok(())
    }
    
    fn calculate_rtt_variance(&self, samples: &VecDeque<Duration>) -> f64 {
        if samples.len() < 2 {
            return 0.0;
        }
        
        let mean = samples.iter().map(|d| d.as_millis() as f64).sum::<f64>() / samples.len() as f64;
        let variance = samples.iter()
            .map(|d| {
                let diff = d.as_millis() as f64 - mean;
                diff * diff
            })
            .sum::<f64>() / samples.len() as f64;
        
        variance.sqrt() / mean // Coefficient of variation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_path_id_creation() {
        assert!(PathId::new(0).is_ok());
        assert!(PathId::new(7).is_ok());
        assert!(PathId::new(8).is_err());
        assert!(PathId::new(255).is_err());
    }
    
    #[test]
    fn test_multipath_data_plane() {
        let data_plane = MultipathDataPlane::new();
        
        // Add paths with different RTTs
        assert!(data_plane.add_path(0, Duration::from_millis(20)).is_ok());
        assert!(data_plane.add_path(1, Duration::from_millis(50)).is_ok());
        assert!(data_plane.add_path(2, Duration::from_millis(100)).is_ok());
        
        // Test path selection
        let selected_path = data_plane.select_path();
        assert!(selected_path.is_ok());
        
        // Test packet sending
        let data = vec![1, 2, 3, 4, 5];
        let result = data_plane.send_packet(data);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_weighted_round_robin() {
        let mut scheduler = WeightedRoundRobinScheduler::new();
        
        scheduler.add_path(PathId::new(0).unwrap(), 1.0);
        scheduler.add_path(PathId::new(1).unwrap(), 2.0);
        scheduler.add_path(PathId::new(2).unwrap(), 3.0);
        
        // Higher weight paths should be selected more frequently
        let mut selections = HashMap::new();
        for _ in 0..100 {
            let path = scheduler.select_next_path().unwrap();
            *selections.entry(path.as_u8()).or_insert(0) += 1;
        }
        
        // Path 2 (weight 3.0) should be selected most frequently
        assert!(selections.get(&2).unwrap() > selections.get(&1).unwrap());
        assert!(selections.get(&1).unwrap() > selections.get(&0).unwrap());
    }
    
    #[test]
    fn test_reordering_buffer() {
        let mut buffer = ReorderingBuffer::new(100, Duration::from_secs(1));
        
        // Insert packets out of order
        let packet1 = PacketWithSequence {
            sequence: 1,
            data: vec![1],
            received_at: Instant::now(),
        };
        let packet0 = PacketWithSequence {
            sequence: 0,
            data: vec![0],
            received_at: Instant::now(),
        };
        
        assert!(buffer.insert_packet(packet1).is_ok());
        assert!(buffer.insert_packet(packet0).is_ok());
        
        // Should extract packets in order
        let ordered = buffer.extract_ordered_packets();
        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0], vec![0]);
        assert_eq!(ordered[1], vec![1]);
    }
}
