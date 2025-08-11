use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, debug, trace};
use crate::types::NodeEndpoint;

/// Advanced routing algorithms for multi-path optimization
pub mod advanced_routing {
    use super::*;

    /// Routing algorithm types
    #[derive(Debug, Clone, PartialEq)]
    pub enum RoutingAlgorithm {
        /// Round-robin distribution
        RoundRobin,
        /// Weighted round-robin based on path quality
        WeightedRoundRobin,
        /// Least connections algorithm
        LeastConnections,
        /// Latency-based routing
        LatencyBased,
        /// Bandwidth-based routing
        BandwidthBased,
        /// Adaptive routing based on multiple metrics
        Adaptive,
    }

    /// Path quality metrics for routing decisions
    #[derive(Debug, Clone)]
    pub struct PathQuality {
        pub endpoint: NodeEndpoint,
        pub latency: Duration,
        pub bandwidth: u64, // bytes per second
        pub packet_loss: f32, // 0.0 to 1.0
        pub jitter: Duration,
        pub congestion_level: f32, // 0.0 to 1.0
        pub reliability_score: f32, // 0.0 to 1.0
        pub last_updated: Instant,
        pub active_connections: u32,
        pub total_bytes_sent: u64,
        pub total_bytes_received: u64,
        pub error_count: u32,
    }

    impl Default for PathQuality {
        fn default() -> Self {
            Self {
                endpoint: NodeEndpoint::new("0.0.0.0:0".parse().unwrap()),
                latency: Duration::from_millis(100),
                bandwidth: 1_000_000, // 1 Mbps default
                packet_loss: 0.0,
                jitter: Duration::from_millis(10),
                congestion_level: 0.0,
                reliability_score: 1.0,
                last_updated: Instant::now(),
                active_connections: 0,
                total_bytes_sent: 0,
                total_bytes_received: 0,
                error_count: 0,
            }
        }
    }

    /// Advanced routing configuration
    #[derive(Debug, Clone)]
    pub struct AdvancedRoutingConfig {
        pub algorithm: RoutingAlgorithm,
        pub max_paths: usize,
        pub path_probe_interval: Duration,
        pub quality_measurement_window: Duration,
        pub reordering_buffer_size: usize,
        pub reordering_timeout: Duration,
        pub congestion_threshold: f32,
        pub failover_threshold: f32,
        pub load_balancing_weights: HashMap<String, f32>,
        pub adaptive_learning_rate: f32,
    }

    impl Default for AdvancedRoutingConfig {
        fn default() -> Self {
            Self {
                algorithm: RoutingAlgorithm::WeightedRoundRobin,
                max_paths: 4,
                path_probe_interval: Duration::from_secs(5),
                quality_measurement_window: Duration::from_secs(30),
                reordering_buffer_size: 100,
                reordering_timeout: Duration::from_millis(100),
                congestion_threshold: 0.8,
                failover_threshold: 0.1,
                load_balancing_weights: HashMap::new(),
                adaptive_learning_rate: 0.1,
            }
        }
    }

    /// Per-path reordering buffer for handling out-of-order packets
    #[derive(Debug)]
    struct ReorderingBuffer {
        buffer: HashMap<u32, (Vec<u8>, Instant)>, // seq_num -> (data, timestamp)
        next_expected: u32,
        max_size: usize,
        timeout: Duration,
    }

    impl ReorderingBuffer {
        fn new(max_size: usize, timeout: Duration) -> Self {
            Self {
                buffer: HashMap::new(),
                next_expected: 0,
                max_size,
                timeout,
            }
        }

        /// Add packet to reordering buffer
        fn add_packet(&mut self, seq_num: u32, data: Vec<u8>) -> Vec<Vec<u8>> {
            let mut ready_packets = Vec::new();
            
            // Add to buffer
            self.buffer.insert(seq_num, (data, Instant::now()));
            
            // Extract ready packets in order
            while let Some((packet_data, _)) = self.buffer.remove(&self.next_expected) {
                ready_packets.push(packet_data);
                self.next_expected += 1;
            }
            
            // Clean up old packets
            self.cleanup_expired();
            
            // Limit buffer size
            if self.buffer.len() > self.max_size {
                self.force_drain(&mut ready_packets);
            }
            
            ready_packets
        }

        /// Clean up expired packets
        fn cleanup_expired(&mut self) {
            let now = Instant::now();
            self.buffer.retain(|_, (_, timestamp)| now.duration_since(*timestamp) < self.timeout);
        }

        /// Force drain buffer when it's full
        fn force_drain(&mut self, ready_packets: &mut Vec<Vec<u8>>) {
            let mut seq_nums: Vec<u32> = self.buffer.keys().copied().collect();
            seq_nums.sort();
            
            for seq_num in seq_nums.into_iter().take(self.max_size / 2) {
                if let Some((data, _)) = self.buffer.remove(&seq_num) {
                    ready_packets.push(data);
                    if seq_num >= self.next_expected {
                        self.next_expected = seq_num + 1;
                    }
                }
            }
        }
    }

    /// Advanced multi-path routing manager
    pub struct AdvancedRouter {
        config: AdvancedRoutingConfig,
        paths: Arc<RwLock<HashMap<NodeEndpoint, PathQuality>>>,
        round_robin_index: Arc<RwLock<usize>>,
        reordering_buffers: Arc<RwLock<HashMap<NodeEndpoint, ReorderingBuffer>>>,
        sequence_number: Arc<RwLock<u32>>,
    }

    impl AdvancedRouter {
        pub fn new(config: AdvancedRoutingConfig) -> Self {
            Self {
                config,
                paths: Arc::new(RwLock::new(HashMap::new())),
                round_robin_index: Arc::new(RwLock::new(0)),
                reordering_buffers: Arc::new(RwLock::new(HashMap::new())),
                sequence_number: Arc::new(RwLock::new(0)),
            }
        }

        /// Add a new path to the routing table
        pub async fn add_path(&self, endpoint: NodeEndpoint) -> Result<(), RoutingError> {
            let mut paths = self.paths.write().await;
            
            if paths.len() >= self.config.max_paths {
                return Err(RoutingError::MaxPathsExceeded);
            }

            let mut quality = PathQuality::default();
            quality.endpoint = endpoint.clone();
            
            paths.insert(endpoint.clone(), quality);
            
            // Initialize reordering buffer for this path
            let mut buffers = self.reordering_buffers.write().await;
            buffers.insert(
                endpoint.clone(), 
                ReorderingBuffer::new(
                    self.config.reordering_buffer_size,
                    self.config.reordering_timeout
                )
            );

            info!("Added new path to routing table: {}", endpoint);
            Ok(())
        }

        /// Remove a path from the routing table
        pub async fn remove_path(&self, endpoint: &NodeEndpoint) -> Result<(), RoutingError> {
            let mut paths = self.paths.write().await;
            let mut buffers = self.reordering_buffers.write().await;
            
            paths.remove(endpoint);
            buffers.remove(endpoint);
            
            info!("Removed path from routing table: {}", endpoint);
            Ok(())
        }

        /// Select the best path for sending data
        pub async fn select_path(&self) -> Result<NodeEndpoint, RoutingError> {
            let paths = self.paths.read().await;
            
            if paths.is_empty() {
                return Err(RoutingError::NoPaths);
            }

            match self.config.algorithm {
                RoutingAlgorithm::RoundRobin => self.round_robin_selection(&paths).await,
                RoutingAlgorithm::WeightedRoundRobin => self.weighted_round_robin_selection(&paths).await,
                RoutingAlgorithm::LeastConnections => self.least_connections_selection(&paths).await,
                RoutingAlgorithm::LatencyBased => self.latency_based_selection(&paths).await,
                RoutingAlgorithm::BandwidthBased => self.bandwidth_based_selection(&paths).await,
                RoutingAlgorithm::Adaptive => self.adaptive_selection(&paths).await,
            }
        }

        /// Round-robin path selection
        async fn round_robin_selection(&self, paths: &HashMap<NodeEndpoint, PathQuality>) -> Result<NodeEndpoint, RoutingError> {
            let mut index = self.round_robin_index.write().await;
            let path_list: Vec<&NodeEndpoint> = paths.keys().collect();
            
            if path_list.is_empty() {
                return Err(RoutingError::NoPaths);
            }

            let selected = path_list[*index % path_list.len()].clone();
            *index = (*index + 1) % path_list.len();
            
            debug!("Selected path via round-robin: {}", selected);
            Ok(selected)
        }

        /// Weighted round-robin based on path quality
        async fn weighted_round_robin_selection(&self, paths: &HashMap<NodeEndpoint, PathQuality>) -> Result<NodeEndpoint, RoutingError> {
            // Proportional deterministic cycling: build cumulative slots preserving ratios
            // Scale weights so that max becomes ~1000 slots, others proportional (min 1)
            let mut items: Vec<(NodeEndpoint, f32)> = paths.iter()
                .map(|(ep,q)| (ep.clone(), self.calculate_path_weight(q)))
                .collect();
            if items.is_empty() { return Err(RoutingError::NoPaths); }
            let max_w = items.iter().map(|(_,w)| *w).fold(0.0, f32::max);
            let scale = if max_w > 0.0 { 1000.0 / max_w } else { 1.0 };
            let mut slots: Vec<NodeEndpoint> = Vec::new();
            for (ep,w) in items.into_iter() {
                let count = ((w * scale).round() as i32).clamp(1, 5000) as usize;
                // Push ep 'count' times
                slots.extend(std::iter::repeat(ep).take(count));
            }
            if slots.is_empty() { return Err(RoutingError::NoPaths); }
            let mut index = self.round_robin_index.write().await;
            if *index >= slots.len() { *index = 0; }
            let selected = slots[*index].clone();
            *index += 1;
            debug!("Selected path via weighted round-robin: {} (slots={})", selected, slots.len());
            Ok(selected)
        }

        /// Least connections path selection
        async fn least_connections_selection(&self, paths: &HashMap<NodeEndpoint, PathQuality>) -> Result<NodeEndpoint, RoutingError> {
            let selected = paths
                .iter()
                .min_by_key(|(_, quality)| quality.active_connections)
                .map(|(endpoint, _)| endpoint.clone())
                .ok_or(RoutingError::NoPaths)?;

            debug!("Selected path via least connections: {}", selected);
            Ok(selected)
        }

        /// Latency-based path selection
        async fn latency_based_selection(&self, paths: &HashMap<NodeEndpoint, PathQuality>) -> Result<NodeEndpoint, RoutingError> {
            let selected = paths
                .iter()
                .filter(|(_, quality)| quality.reliability_score > self.config.failover_threshold)
                .min_by_key(|(_, quality)| quality.latency)
                .map(|(endpoint, _)| endpoint.clone())
                .ok_or(RoutingError::NoPaths)?;

            debug!("Selected path via latency-based: {} (latency: {:?})", 
                   selected, paths.get(&selected).unwrap().latency);
            Ok(selected)
        }

        /// Bandwidth-based path selection
        async fn bandwidth_based_selection(&self, paths: &HashMap<NodeEndpoint, PathQuality>) -> Result<NodeEndpoint, RoutingError> {
            let selected = paths
                .iter()
                .filter(|(_, quality)| quality.congestion_level < self.config.congestion_threshold)
                .max_by_key(|(_, quality)| quality.bandwidth)
                .map(|(endpoint, _)| endpoint.clone())
                .ok_or(RoutingError::NoPaths)?;

            debug!("Selected path via bandwidth-based: {} (bandwidth: {} bps)", 
                   selected, paths.get(&selected).unwrap().bandwidth);
            Ok(selected)
        }

        /// Adaptive path selection based on multiple metrics
        async fn adaptive_selection(&self, paths: &HashMap<NodeEndpoint, PathQuality>) -> Result<NodeEndpoint, RoutingError> {
            let selected = paths
                .iter()
                .max_by(|(_, a), (_, b)| {
                    let score_a = self.calculate_adaptive_score(a);
                    let score_b = self.calculate_adaptive_score(b);
                    score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(endpoint, _)| endpoint.clone())
                .ok_or(RoutingError::NoPaths)?;

            debug!("Selected path via adaptive: {} (score: {:.3})", 
                   selected, self.calculate_adaptive_score(paths.get(&selected).unwrap()));
            Ok(selected)
        }

        /// Calculate path weight for weighted round-robin
        fn calculate_path_weight(&self, quality: &PathQuality) -> f32 {
            // Combine multiple metrics into a single weight
            let latency_score = 1.0 / (quality.latency.as_millis() as f32 + 1.0);
            let bandwidth_score = quality.bandwidth as f32 / 1_000_000.0; // Normalize to Mbps
            let reliability_score = quality.reliability_score;
            let congestion_score = 1.0 - quality.congestion_level;
            // 正規化: 各スコアをだいたい 0..1 に収め総和を 0..1.5 程度に
            let raw = latency_score * 0.25 + bandwidth_score * 0.25 + reliability_score * 0.3 + congestion_score * 0.2;
            raw.max(0.01)
        }

        /// Calculate adaptive scoring for multi-metric routing
        fn calculate_adaptive_score(&self, quality: &PathQuality) -> f32 {
            let latency_score = 1.0 / (quality.latency.as_millis() as f32 + 1.0);
            let bandwidth_score = quality.bandwidth as f32 / 10_000_000.0; // Normalize to 10 Mbps
            let loss_score = 1.0 - quality.packet_loss;
            let reliability_score = quality.reliability_score;
            let congestion_score = 1.0 - quality.congestion_level;
            let connection_score = 1.0 / (quality.active_connections as f32 + 1.0);

            // Weighted combination of all metrics
            latency_score * 0.2 + 
            bandwidth_score * 0.2 + 
            loss_score * 0.15 + 
            reliability_score * 0.2 + 
            congestion_score * 0.15 + 
            connection_score * 0.1
        }

        /// Update path quality metrics
        pub async fn update_path_quality(&self, endpoint: &NodeEndpoint, quality: PathQuality) -> Result<(), RoutingError> {
            let mut paths = self.paths.write().await;
            
            if let Some(existing_quality) = paths.get_mut(endpoint) {
                // Bootstrap: if this looks like the untouched default metrics, adopt new quality directly
                let is_bootstrap = existing_quality.total_bytes_sent == 0
                    && existing_quality.total_bytes_received == 0
                    && existing_quality.error_count == 0
                    && existing_quality.latency == Duration::from_millis(100)
                    && existing_quality.bandwidth == 1_000_000;

                if is_bootstrap {
                    *existing_quality = quality;
                } else {
                    // Apply exponential moving average for smooth updates
                    let alpha = self.config.adaptive_learning_rate;
                    existing_quality.latency = Duration::from_nanos(
                        ((1.0 - alpha) * existing_quality.latency.as_nanos() as f32 +
                         alpha * quality.latency.as_nanos() as f32) as u64
                    );
                    existing_quality.bandwidth = 
                        ((1.0 - alpha) * existing_quality.bandwidth as f32 +
                         alpha * quality.bandwidth as f32) as u64;
                    existing_quality.packet_loss = 
                        (1.0 - alpha) * existing_quality.packet_loss +
                        alpha * quality.packet_loss;
                    existing_quality.congestion_level = 
                        (1.0 - alpha) * existing_quality.congestion_level +
                        alpha * quality.congestion_level;
                    existing_quality.reliability_score = 
                        (1.0 - alpha) * existing_quality.reliability_score +
                        alpha * quality.reliability_score;
                    existing_quality.last_updated = Instant::now();
                }
                
                trace!("Updated path quality for {}: latency={:?}, bandwidth={}, loss={:.3}", 
                       endpoint, existing_quality.latency, existing_quality.bandwidth, existing_quality.packet_loss);
            }
            
            Ok(())
        }

        /// Process incoming packet with reordering
        pub async fn process_incoming_packet(&self, endpoint: &NodeEndpoint, seq_num: u32, data: Vec<u8>) -> Vec<Vec<u8>> {
            let mut buffers = self.reordering_buffers.write().await;
            
            if let Some(buffer) = buffers.get_mut(endpoint) {
                buffer.add_packet(seq_num, data)
            } else {
                // No reordering buffer for this path, return packet as-is
                vec![data]
            }
        }

        /// Get next sequence number for outgoing packets
        pub async fn next_sequence_number(&self) -> u32 {
            let mut seq = self.sequence_number.write().await;
            let current = *seq;
            *seq = seq.wrapping_add(1);
            current
        }

        /// Get current routing statistics
        pub async fn get_routing_stats(&self) -> RoutingStats {
            let paths = self.paths.read().await;
            
            RoutingStats {
                total_paths: paths.len(),
                active_paths: paths.values().filter(|q| q.reliability_score > self.config.failover_threshold).count(),
                avg_latency: paths.values().map(|q| q.latency.as_millis()).sum::<u128>() as f64 / paths.len() as f64,
                total_bandwidth: paths.values().map(|q| q.bandwidth).sum(),
                avg_packet_loss: paths.values().map(|q| q.packet_loss).sum::<f32>() / paths.len() as f32,
                algorithm: self.config.algorithm.clone(),
            }
        }

        /// Perform path quality probing
        pub async fn probe_path_quality(&self, endpoint: &NodeEndpoint) -> Result<PathQuality, RoutingError> {
            // This would be implemented to actually measure path quality
            // For now, return current quality with simulated updates
            let paths = self.paths.read().await;
            paths.get(endpoint).cloned().ok_or(RoutingError::PathNotFound)
        }

        /// Start background tasks for quality monitoring
        pub async fn start_monitoring(&self) -> Result<(), RoutingError> {
            let paths_clone = self.paths.clone();
            let config_clone = self.config.clone();
            
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(config_clone.path_probe_interval);
                
                loop {
                    interval.tick().await;
                    
                    let paths_read = paths_clone.read().await;
                    for endpoint in paths_read.keys() {
                        // Simulate quality probing
                        trace!("Probing path quality for {}", endpoint);
                        // In a real implementation, this would send probe packets
                        // and measure actual network metrics
                    }
                }
            });
            
            info!("Started advanced routing quality monitoring");
            Ok(())
        }
    }

    /// Routing error types
    #[derive(Debug, thiserror::Error)]
    pub enum RoutingError {
        #[error("No paths available")]
        NoPaths,
        #[error("Maximum number of paths exceeded")]
        MaxPathsExceeded,
        #[error("Path not found")]
        PathNotFound,
        #[error("Invalid routing configuration")]
        InvalidConfig,
        #[error("Path quality measurement failed")]
        QualityMeasurementFailed,
    }

    /// Routing statistics
    #[derive(Debug, Clone)]
    pub struct RoutingStats {
        pub total_paths: usize,
        pub active_paths: usize,
        pub avg_latency: f64,
        pub total_bandwidth: u64,
        pub avg_packet_loss: f32,
        pub algorithm: RoutingAlgorithm,
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::net::SocketAddr;

        fn create_test_endpoint(port: u16) -> NodeEndpoint {
            let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
            NodeEndpoint::new(addr)
        }

        #[tokio::test]
        async fn test_basic_routing() {
            let config = AdvancedRoutingConfig::default();
            let router = AdvancedRouter::new(config);
            
            let endpoint1 = create_test_endpoint(8001);
            let endpoint2 = create_test_endpoint(8002);
            
            router.add_path(endpoint1.clone()).await.unwrap();
            router.add_path(endpoint2.clone()).await.unwrap();
            
            let selected = router.select_path().await.unwrap();
            assert!(selected == endpoint1 || selected == endpoint2);
        }

        #[tokio::test]
        async fn test_weighted_round_robin() {
            let mut config = AdvancedRoutingConfig::default();
            config.algorithm = RoutingAlgorithm::WeightedRoundRobin;
            let router = AdvancedRouter::new(config);
            
            let endpoint1 = create_test_endpoint(8001);
            let endpoint2 = create_test_endpoint(8002);
            
            router.add_path(endpoint1.clone()).await.unwrap();
            router.add_path(endpoint2.clone()).await.unwrap();
            
            // Update path qualities with different weights
            let mut quality1 = PathQuality::default();
            quality1.endpoint = endpoint1.clone();
            quality1.latency = Duration::from_millis(10);
            quality1.bandwidth = 10_000_000;
            
            let mut quality2 = PathQuality::default();
            quality2.endpoint = endpoint2.clone();
            quality2.latency = Duration::from_millis(50);
            quality2.bandwidth = 1_000_000;
            
            router.update_path_quality(&endpoint1, quality1).await.unwrap();
            router.update_path_quality(&endpoint2, quality2).await.unwrap();
            
            // Select paths multiple times and verify distribution probabilistically
            let mut selections: HashMap<String, u32> = HashMap::new();
            for _ in 0..1500 { // cover full weighted cycle (max slots ~1000)
                let selected = router.select_path().await.unwrap();
                let key = format!("{}", selected);
                *selections.entry(key).or_insert(0) += 1;
            }
            let c1 = *selections.get(&format!("{}", endpoint1)).unwrap_or(&0);
            let c2 = *selections.get(&format!("{}", endpoint2)).unwrap_or(&0);
            // Both paths must receive traffic
            assert!(c1 > 0 && c2 > 0, "expected both paths selected (c1={}, c2={})", c1, c2);
            // Compute expected ratio from weight function directly to avoid hard-coding.
            // Reconstruct weights used when building slots.
            let mut q_fast = PathQuality::default();
            q_fast.latency = Duration::from_millis(10); q_fast.bandwidth = 10_000_000; q_fast.reliability_score = 1.0; q_fast.congestion_level = 0.0;
            let mut q_slow = PathQuality::default();
            q_slow.latency = Duration::from_millis(50); q_slow.bandwidth = 1_000_000; q_slow.reliability_score = 1.0; q_slow.congestion_level = 0.0;
            let w_fast = router.calculate_path_weight(&q_fast);
            let w_slow = router.calculate_path_weight(&q_slow);
            let expected_ratio = (w_fast.max(w_slow) / w_fast.min(w_slow)).max(1.0);
            // Empirical ratio from counts (largest / smallest)
            let ratio = (c1.max(c2) as f32) / (c1.min(c2) as f32);
            // Allow 40% tolerance due to integer rounding & cycle boundary (1500 selections spans >1 cycle)
            let lower = expected_ratio * 0.6; let upper = expected_ratio * 1.4;
            assert!(ratio >= lower && ratio <= upper, "ratio out of tolerance: counts=({},{}), ratio={}, expected≈{:.2} tol=[{:.2},{:.2}]", c1, c2, ratio, expected_ratio, lower, upper);
        }

        #[tokio::test]
        async fn test_latency_based_routing() {
            let mut config = AdvancedRoutingConfig::default();
            config.algorithm = RoutingAlgorithm::LatencyBased;
            let router = AdvancedRouter::new(config);
            
            let endpoint1 = create_test_endpoint(8001);
            let endpoint2 = create_test_endpoint(8002);
            
            router.add_path(endpoint1.clone()).await.unwrap();
            router.add_path(endpoint2.clone()).await.unwrap();
            
            // Update path qualities with different latencies
            let mut quality1 = PathQuality::default();
            quality1.endpoint = endpoint1.clone();
            quality1.latency = Duration::from_millis(5);
            
            let mut quality2 = PathQuality::default();
            quality2.endpoint = endpoint2.clone();
            quality2.latency = Duration::from_millis(50);
            
            router.update_path_quality(&endpoint1, quality1).await.unwrap();
            router.update_path_quality(&endpoint2, quality2).await.unwrap();
            
            // Should consistently select the lower latency path
            let selected = router.select_path().await.unwrap();
            assert_eq!(selected, endpoint1);
        }

        #[tokio::test]
        async fn test_reordering_buffer() {
            let mut buffer = ReorderingBuffer::new(10, Duration::from_millis(1000));
            
            // Add packets out of order
            let packets1 = buffer.add_packet(2, vec![2, 2, 2]);
            assert!(packets1.is_empty()); // Not ready yet
            
            let packets2 = buffer.add_packet(0, vec![0, 0, 0]);
            assert_eq!(packets2.len(), 1); // Should return packet 0
            
            let packets3 = buffer.add_packet(1, vec![1, 1, 1]);
            assert_eq!(packets3.len(), 2); // Should return packets 1 and 2
        }

        #[tokio::test]
        async fn test_adaptive_scoring() {
            let config = AdvancedRoutingConfig::default();
            let router = AdvancedRouter::new(config);
            
            let mut quality1 = PathQuality::default();
            quality1.latency = Duration::from_millis(10);
            quality1.bandwidth = 10_000_000;
            quality1.packet_loss = 0.01;
            quality1.reliability_score = 0.9;
            
            let mut quality2 = PathQuality::default();
            quality2.latency = Duration::from_millis(100);
            quality2.bandwidth = 1_000_000;
            quality2.packet_loss = 0.1;
            quality2.reliability_score = 0.7;
            
            let score1 = router.calculate_adaptive_score(&quality1);
            let score2 = router.calculate_adaptive_score(&quality2);
            
            assert!(score1 > score2, "Quality1 should have higher score than Quality2");
        }

        #[tokio::test]
        async fn test_path_limits() {
            let mut config = AdvancedRoutingConfig::default();
            config.max_paths = 2;
            let router = AdvancedRouter::new(config);
            
            let endpoint1 = create_test_endpoint(8001);
            let endpoint2 = create_test_endpoint(8002);
            let endpoint3 = create_test_endpoint(8003);
            
            assert!(router.add_path(endpoint1).await.is_ok());
            assert!(router.add_path(endpoint2).await.is_ok());
            assert!(router.add_path(endpoint3).await.is_err()); // Should fail - max paths exceeded
        }

        #[tokio::test]
        async fn test_latency_failover_on_reliability_filter() {
            // If a path's reliability drops below failover_threshold it should be excluded
            // by latency-based routing even if it has lower latency.
            let mut config = AdvancedRoutingConfig::default();
            config.algorithm = RoutingAlgorithm::LatencyBased;
            config.failover_threshold = 0.5; // exclude paths reliability <= 0.5
            let router = AdvancedRouter::new(config);

            let fast_unreliable = create_test_endpoint(8101);
            let slow_reliable = create_test_endpoint(8102);
            router.add_path(fast_unreliable.clone()).await.unwrap();
            router.add_path(slow_reliable.clone()).await.unwrap();

            let mut q_fast = PathQuality::default();
            q_fast.endpoint = fast_unreliable.clone();
            q_fast.latency = Duration::from_millis(5); // very low latency
            q_fast.reliability_score = 0.2; // below threshold -> should be filtered out

            let mut q_slow = PathQuality::default();
            q_slow.endpoint = slow_reliable.clone();
            q_slow.latency = Duration::from_millis(40); // higher latency
            q_slow.reliability_score = 0.9; // acceptable

            router.update_path_quality(&fast_unreliable, q_fast).await.unwrap();
            router.update_path_quality(&slow_reliable, q_slow).await.unwrap();

            let selected = router.select_path().await.unwrap();
            assert_eq!(selected, slow_reliable, "Expected failover to reliable path despite higher latency");
        }
    }
}
