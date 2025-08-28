//! Path Performance Testing Module for Nyx Daemon
//! Provides comprehensive testing and benchmarking for path performance metrics
//! Includes latency measurement, bandwidth testing, and path quality evaluation

use crate::errors::{DaemonError, Result};
use crate::path_builder::PathBuilder;
use nyx_transport::UdpTransport;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, timeout};

#[derive(Debug, Clone, Default)]
pub struct DaemonConfig {
    // Dummy config for now
    pub max_paths: usize,
}
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct PathPerformanceConfig {
    /// Number of test packets to send per measurement
    pub test_packet_count: usize,
    /// Size of test packets in bytes
    pub test_packet_size: usize,
    /// Timeout for individual path tests
    pub test_timeout: Duration,
    /// Interval between performance measurements
    pub measurement_interval: Duration,
    /// Number of paths to test simultaneously
    pub concurrent_path_limit: usize,
    /// Minimum required bandwidth (bytes/sec)
    pub min_bandwidth_threshold: u64,
    /// Maximum acceptable latency
    pub max_latency_threshold: Duration,
    /// Packet loss threshold (0.0 - 1.0)
    pub max_loss_threshold: f64,
}

impl Default for PathPerformanceConfig {
    fn default() -> Self {
        Self {
            test_packet_count: 100,
            test_packet_size: 1280,
            test_timeout: Duration::from_secs(5),
            measurement_interval: Duration::from_secs(30),
            concurrent_path_limit: 10,
            min_bandwidth_threshold: 100_000, // 100 KB/s
            max_latency_threshold: Duration::from_millis(500),
            max_loss_threshold: 0.05, // 5%
        }
    }
}

#[derive(Debug, Clone)]
pub struct PathPerformanceMetrics {
    pub path_id: String,
    pub target_addr: SocketAddr,
    pub avg_latency: Duration,
    pub min_latency: Duration,
    pub max_latency: Duration,
    pub jitter: Duration,
    pub bandwidth_estimate: u64, // bytes/sec
    pub packet_loss_rate: f64,   // 0.0 - 1.0
    pub test_timestamp: SystemTime,
    pub test_duration: Duration,
    pub packets_sent: usize,
    pub packets_received: usize,
    pub quality_score: f64, // 0.0 - 1.0
}

impl PathPerformanceMetrics {
    fn calculate_quality_score(&mut self) {
        // Normalized scores (0.0 - 1.0, higher is better)
        let latency_score = 1.0 - (self.avg_latency.as_millis() as f64 / 1000.0).min(1.0);
        let bandwidth_score = (self.bandwidth_estimate as f64 / 10_000_000.0).min(1.0); // Normalize to 10Mbps
        let loss_score = 1.0 - self.packet_loss_rate;
        let jitter_score = 1.0 - (self.jitter.as_millis() as f64 / 100.0).min(1.0);

        // Weighted average
        self.quality_score =
            (latency_score * 0.3 + bandwidth_score * 0.3 + loss_score * 0.3 + jitter_score * 0.1)
                .clamp(0.0, 1.0);
    }
}

#[derive(Debug, Clone)]
pub struct TestPacket {
    pub id: u64,
    pub sent_time: Instant,
    pub size: usize,
    pub data: Vec<u8>,
}

impl TestPacket {
    fn new(id: u64, size: usize) -> Self {
        let data = vec![0xAA; size]; // Test pattern
        Self {
            id,
            sent_time: Instant::now(),
            size,
            data,
        }
    }
}

/// Path Performance Test Engine
pub struct PathPerformanceTest {
    config: PathPerformanceConfig,
    path_builder: Arc<PathBuilder>,
    transport: Arc<UdpTransport>,
    active_tests: Arc<Mutex<HashMap<String, TestSession>>>,
    performance_history: Arc<RwLock<HashMap<String, Vec<PathPerformanceMetrics>>>>,
    test_counter: Arc<Mutex<u64>>,
}

struct TestSession {
    path_id: String,
    target_addr: SocketAddr,
    packets_sent: HashMap<u64, TestPacket>,
    packets_received: Vec<(u64, Instant)>, // (packet_id, received_time)
    start_time: Instant,
    #[allow(dead_code)]
    expected_packets: usize,
}

impl PathPerformanceTest {
    pub fn new(
        config: PathPerformanceConfig,
        path_builder: Arc<PathBuilder>,
        transport: Arc<UdpTransport>,
    ) -> Self {
        Self {
            config,
            path_builder,
            transport,
            active_tests: Arc::new(Mutex::new(HashMap::new())),
            performance_history: Arc::new(RwLock::new(HashMap::new())),
            test_counter: Arc::new(Mutex::new(0)),
        }
    }

    /// Start continuous path performance monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        let config = self.config.clone();
        let path_builder = Arc::clone(&self.path_builder);
        let active_tests = Arc::clone(&self.active_tests);
        let performance_history = Arc::clone(&self.performance_history);
        let transport = Arc::clone(&self.transport);
        let test_counter = Arc::clone(&self.test_counter);

        tokio::spawn(async move {
            let mut interval = interval(config.measurement_interval);

            loop {
                interval.tick().await;

                if let Err(e) = Self::run_performance_tests(
                    &config,
                    &path_builder,
                    &transport,
                    &active_tests,
                    &performance_history,
                    &test_counter,
                )
                .await
                {
                    error!("Performance test failed: {}", e);
                }
            }
        });

        info!("Path performance monitoring started");
        Ok(())
    }

    /// Test performance of a specific path
    pub async fn test_path_performance(
        &self,
        path_id: &str,
        target_addr: SocketAddr,
    ) -> Result<PathPerformanceMetrics> {
        let test_start = Instant::now();
        let session_id = format!("{path_id}_{}", test_start.elapsed().as_nanos());

        // Create test session
        let session = TestSession {
            path_id: path_id.to_string(),
            target_addr,
            packets_sent: HashMap::new(),
            packets_received: Vec::new(),
            start_time: test_start,
            expected_packets: self.config.test_packet_count,
        };

        {
            let mut active_tests = self.active_tests.lock().await;
            active_tests.insert(session_id.clone(), session);
        }

        // Perform the test
        let result = timeout(
            self.config.test_timeout,
            self.execute_path_test(&session_id, target_addr),
        )
        .await;

        // Clean up and calculate metrics
        let metrics = match result {
            Ok(Ok(metrics)) => metrics,
            Ok(Err(e)) => {
                warn!("Path test failed for {}: {}", path_id, e);
                return Err(e);
            }
            Err(_) => {
                warn!("Path test timed out for {}", path_id);
                return Err(DaemonError::Timeout("Path test timeout".to_string()));
            }
        };

        // Remove test session
        {
            let mut active_tests = self.active_tests.lock().await;
            active_tests.remove(&session_id);
        }

        // Store metrics in history
        {
            let mut history = self.performance_history.write().await;
            history
                .entry(path_id.to_string())
                .or_insert_with(Vec::new)
                .push(metrics.clone());

            // Keep only recent history (last 100 measurements)
            if let Some(path_history) = history.get_mut(path_id) {
                if path_history.len() > 100 {
                    path_history.remove(0);
                }
            }
        }

        info!(
            "Path {} performance: latency={:?}, bandwidth={} bytes/s, loss={:.2}%, quality={:.3}",
            path_id,
            metrics.avg_latency,
            metrics.bandwidth_estimate,
            metrics.packet_loss_rate * 100.0,
            metrics.quality_score
        );

        Ok(metrics)
    }

    /// Execute the actual performance test
    async fn execute_path_test(
        &self,
        session_id: &str,
        target_addr: SocketAddr,
    ) -> Result<PathPerformanceMetrics> {
        let packet_id_counter = {
            let mut counter = self.test_counter.lock().await;
            *counter += 1;
            *counter
        };

        // Send test packets
        for i in 0..self.config.test_packet_count {
            let packet_id = packet_id_counter * 1000 + i as u64;
            let test_packet = TestPacket::new(packet_id, self.config.test_packet_size);

            // Record sent packet
            {
                let mut active_tests = self.active_tests.lock().await;
                if let Some(session) = active_tests.get_mut(session_id) {
                    session.packets_sent.insert(packet_id, test_packet.clone());
                }
            }

            // Send packet via transport
            self.transport
                .send_to(&test_packet.data, target_addr)
                .await
                .map_err(|e| DaemonError::Transport(format!("Failed to send test packet: {e}")))?;

            // Small delay between packets to avoid overwhelming
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Wait for responses (simplified - in real implementation would listen for responses)
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Calculate metrics
        self.calculate_test_metrics(session_id).await
    }

    /// Calculate performance metrics from test session
    async fn calculate_test_metrics(&self, session_id: &str) -> Result<PathPerformanceMetrics> {
        let active_tests = self.active_tests.lock().await;
        let session = active_tests
            .get(session_id)
            .ok_or_else(|| DaemonError::Internal("Test session not found".to_string()))?;

        let test_duration = session.start_time.elapsed();
        let packets_sent = session.packets_sent.len();
        let packets_received = session.packets_received.len();

        // Calculate latency metrics
        let mut latencies = Vec::new();
        for (packet_id, received_time) in &session.packets_received {
            if let Some(sent_packet) = session.packets_sent.get(packet_id) {
                let latency = received_time.duration_since(sent_packet.sent_time);
                latencies.push(latency);
            }
        }

        let (avg_latency, min_latency, max_latency, jitter) = if !latencies.is_empty() {
            latencies.sort();
            let avg = latencies.iter().sum::<Duration>() / latencies.len() as u32;
            let min = latencies[0];
            let max = latencies[latencies.len() - 1];

            // Calculate jitter (standard deviation of latencies)
            let variance: f64 = latencies
                .iter()
                .map(|&lat| {
                    let diff = lat.as_nanos() as i64 - avg.as_nanos() as i64;
                    (diff * diff) as f64
                })
                .sum::<f64>()
                / latencies.len() as f64;
            let jitter = Duration::from_nanos(variance.sqrt() as u64);

            (avg, min, max, jitter)
        } else {
            (
                Duration::from_millis(999),
                Duration::from_millis(999),
                Duration::from_millis(999),
                Duration::from_millis(0),
            )
        };

        // Calculate bandwidth
        let bytes_transferred = packets_received * self.config.test_packet_size;
        let bandwidth_estimate = if test_duration.as_secs() > 0 {
            bytes_transferred as u64 / test_duration.as_secs()
        } else {
            bytes_transferred as u64 * 1000 / test_duration.as_millis().max(1) as u64
        };

        // Calculate packet loss rate
        let packet_loss_rate = if packets_sent > 0 {
            1.0 - (packets_received as f64 / packets_sent as f64)
        } else {
            1.0
        };

        let mut metrics = PathPerformanceMetrics {
            path_id: session.path_id.clone(),
            target_addr: session.target_addr,
            avg_latency,
            min_latency,
            max_latency,
            jitter,
            bandwidth_estimate,
            packet_loss_rate,
            test_timestamp: SystemTime::now(),
            test_duration,
            packets_sent,
            packets_received,
            quality_score: 0.0, // Will be calculated
        };

        metrics.calculate_quality_score();
        Ok(metrics)
    }

    /// Run performance tests for all known paths
    async fn run_performance_tests(
        config: &PathPerformanceConfig,
        path_builder: &PathBuilder,
        transport: &UdpTransport,
        active_tests: &Arc<Mutex<HashMap<String, TestSession>>>,
        performance_history: &Arc<RwLock<HashMap<String, Vec<PathPerformanceMetrics>>>>,
        test_counter: &Arc<Mutex<u64>>,
    ) -> Result<()> {
        // Get available paths from path builder
        let paths = path_builder.get_available_paths().await?;

        // Limit concurrent tests
        let test_paths: Vec<_> = paths
            .into_iter()
            .take(config.concurrent_path_limit)
            .collect();

        let mut test_tasks = Vec::new();

        for (path_id, target_addr) in test_paths {
            let test_instance = PathPerformanceTest {
                config: config.clone(),
                path_builder: Arc::new(path_builder.clone()),
                transport: Arc::new(transport.clone()),
                active_tests: Arc::clone(active_tests),
                performance_history: Arc::clone(performance_history),
                test_counter: Arc::clone(test_counter),
            };

            let task = tokio::spawn(async move {
                match test_instance
                    .test_path_performance(&path_id, target_addr)
                    .await
                {
                    Ok(metrics) => {
                        debug!(
                            "Path {} test completed: quality={:.3}",
                            path_id, metrics.quality_score
                        );
                    }
                    Err(e) => {
                        warn!("Path {} test failed: {}", path_id, e);
                    }
                }
            });

            test_tasks.push(task);
        }

        // Wait for all tests to complete
        for task in test_tasks {
            let _ = task.await;
        }

        Ok(())
    }

    /// Get performance history for a specific path
    pub async fn get_path_history(&self, path_id: &str) -> Vec<PathPerformanceMetrics> {
        let history = self.performance_history.read().await;
        history.get(path_id).cloned().unwrap_or_default()
    }

    /// Get performance summary for all paths
    pub async fn get_performance_summary(&self) -> HashMap<String, PathPerformanceMetrics> {
        let history = self.performance_history.read().await;
        let mut summary = HashMap::new();

        for (path_id, metrics_list) in history.iter() {
            if let Some(latest_metric) = metrics_list.last() {
                summary.insert(path_id.clone(), latest_metric.clone());
            }
        }

        summary
    }

    /// Check if a path meets performance thresholds
    pub fn meets_performance_thresholds(&self, metrics: &PathPerformanceMetrics) -> bool {
        metrics.bandwidth_estimate >= self.config.min_bandwidth_threshold
            && metrics.avg_latency <= self.config.max_latency_threshold
            && metrics.packet_loss_rate <= self.config.max_loss_threshold
    }

    /// Get paths that meet performance requirements
    pub async fn get_good_paths(&self) -> Vec<String> {
        let summary = self.get_performance_summary().await;
        summary
            .into_iter()
            .filter(|(_, metrics)| self.meets_performance_thresholds(metrics))
            .map(|(path_id, _)| path_id)
            .collect()
    }

    /// Reset performance history
    pub async fn reset_history(&self) {
        let mut history = self.performance_history.write().await;
        history.clear();
        info!("Performance history reset");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[tokio::test]
    async fn test_path_performance_config() {
        let config = PathPerformanceConfig::default();
        assert_eq!(config.test_packet_count, 100);
        assert!(config.test_timeout > Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_metrics_quality_calculation() {
        let mut metrics = PathPerformanceMetrics {
            path_id: "test".to_string(),
            target_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            avg_latency: Duration::from_millis(50),
            min_latency: Duration::from_millis(40),
            max_latency: Duration::from_millis(60),
            jitter: Duration::from_millis(5),
            bandwidth_estimate: 1_000_000, // 1 MB/s
            packet_loss_rate: 0.01,        // 1%
            test_timestamp: SystemTime::now(),
            test_duration: Duration::from_secs(1),
            packets_sent: 100,
            packets_received: 99,
            quality_score: 0.0,
        };

        metrics.calculate_quality_score();
        assert!(metrics.quality_score > 0.0);
        assert!(metrics.quality_score <= 1.0);
    }

    #[tokio::test]
    async fn test_test_packet_creation() {
        let packet = TestPacket::new(1, 1280);
        assert_eq!(packet.id, 1);
        assert_eq!(packet.size, 1280);
        assert_eq!(packet.data.len(), 1280);
        assert!(packet.data.iter().all(|&b| b == 0xAA));
    }

    #[tokio::test]
    async fn test_performance_thresholds() -> Result<()> {
        let config = PathPerformanceConfig::default();
        let test = PathPerformanceTest::new(
            config.clone(),
            Arc::new(PathBuilder::new(Default::default())?),
            Arc::new(
                UdpTransport::new(Default::default())
                    .map_err(|_| DaemonError::transport("Transport error"))?,
            ),
        );

        let good_metrics = PathPerformanceMetrics {
            path_id: "good".to_string(),
            target_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
            avg_latency: Duration::from_millis(100),
            min_latency: Duration::from_millis(90),
            max_latency: Duration::from_millis(110),
            jitter: Duration::from_millis(5),
            bandwidth_estimate: 200_000, // Above threshold
            packet_loss_rate: 0.02,      // Below threshold
            test_timestamp: SystemTime::now(),
            test_duration: Duration::from_secs(1),
            packets_sent: 100,
            packets_received: 98,
            quality_score: 0.8,
        };

        assert!(test.meets_performance_thresholds(&good_metrics));

        let bad_metrics = PathPerformanceMetrics {
            bandwidth_estimate: 50_000, // Below threshold
            packet_loss_rate: 0.1,      // Above threshold
            ..good_metrics
        };

        assert!(!test.meets_performance_thresholds(&bad_metrics));

        Ok(())
    }
}
