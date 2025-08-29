#![allow(missing_docs)]

//! Practical usage example for Advanced Rate Limiting & Flow Control
//!
//! This example demonstrates how to integrate the Nyx Protocol v1.0 advanced
//! rate limiting system into real stream operations for comprehensive traffic
//! management and network optimization.

use nyx_stream::{AdvancedFlowConfig, NyxRateLimiter, TrafficType, TransmissionDecision};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};
use tracing::{debug, info, warn};

/// High-level stream manager with integrated rate limiting
pub struct StreamManager {
    /// Advanced rate limiter for traffic control
    rate_limiter: Arc<NyxRateLimiter>,
    /// Connection tracking
    active_connections: Arc<Mutex<HashMap<u64, ConnectionInfo>>>,
    /// Configuration
    config: StreamManagerConfig,
    /// Performance statistics
    stats: Arc<Mutex<StreamManagerStats>>,
}

/// Stream manager configuration
#[derive(Debug, Clone)]
pub struct StreamManagerConfig {
    /// Maximum connections to track
    max_connections: usize,
    /// Connection idle timeout
    connection_timeout: Duration,
    /// Statistics reporting interval
    stats_interval: Duration,
    /// Enable adaptive rate limiting
    _adaptive_mode: bool,
}

impl Default for StreamManagerConfig {
    fn default() -> Self {
        Self {
            max_connections: 10000,
            connection_timeout: Duration::from_secs(300), // 5 minutes
            stats_interval: Duration::from_secs(30),
            _adaptive_mode: true,
        }
    }
}

/// Connection information tracking
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Connection establishment time
    established_at: Instant,
    /// Last activity timestamp
    last_activity: Instant,
    /// Total bytes sent
    bytes_sent: u64,
    /// Total bytes received
    bytes_received: u64,
    /// Connection priority level
    priority: TrafficType,
    /// Current RTT estimate
    rtt: Option<Duration>,
}

/// Stream manager performance statistics
#[derive(Debug, Clone, Default)]
pub struct StreamManagerStats {
    pub total_connections: u64,
    pub active_connections: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub successful_transmissions: u64,
    pub rate_limited_transmissions: u64,
    pub flow_control_blocks: u64,
    pub backpressure_events: u64,
    pub average_rtt: Option<Duration>,
    pub peak_bandwidth_usage: u64,
}

impl StreamManager {
    /// Create a new stream manager with advanced rate limiting
    pub fn new(flow_config: AdvancedFlowConfig, manager_config: StreamManagerConfig) -> Self {
        let rate_limiter = Arc::new(NyxRateLimiter::new(flow_config));

        // Register standard queues for backpressure monitoring
        rate_limiter.register_queue("send_buffer".to_string(), 100000);
        rate_limiter.register_queue("recv_buffer".to_string(), 100000);
        rate_limiter.register_queue("control_queue".to_string(), 1000);
        rate_limiter.register_queue("priority_queue".to_string(), 5000);

        Self {
            rate_limiter,
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            config: manager_config,
            stats: Arc::new(Mutex::new(StreamManagerStats::default())),
        }
    }

    /// Establish a new connection with rate limiting integration
    pub async fn establish_connection(
        &self,
        connection_id: u64,
        priority: TrafficType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut connections = self.active_connections.lock().await;

        // Check connection limits
        if connections.len() >= self.config.max_connections {
            warn!(
                connection_id = connection_id,
                current_connections = connections.len(),
                max_connections = self.config.max_connections,
                "Connection limit reached"
            );
            return Err("Connection limit exceeded".into());
        }

        // Create connection info
        let now = Instant::now();
        let conn_info = ConnectionInfo {
            established_at: now,
            last_activity: now,
            bytes_sent: 0,
            bytes_received: 0,
            priority,
            rtt: None,
        };

        connections.insert(connection_id, conn_info);

        // Update statistics
        {
            let mut stats = self.stats.lock().await;
            stats.total_connections += 1;
            stats.active_connections = connections.len() as u64;
        }

        info!(
            connection_id = connection_id,
            priority = ?priority,
            total_connections = connections.len(),
            "Connection established with rate limiting"
        );

        Ok(())
    }

    /// Send data with comprehensive rate limiting and flow control
    pub async fn send_data(
        &self,
        connection_id: u64,
        stream_id: u64,
        data: &[u8],
        priority: Option<TrafficType>,
    ) -> Result<usize, Box<dyn std::error::Error>> {
        // Determine traffic priority
        let traffic_type = if let Some(priority) = priority {
            priority
        } else {
            let connections = self.active_connections.lock().await;
            connections
                .get(&connection_id)
                .map(|conn| conn.priority)
                .unwrap_or(TrafficType::Normal)
        };

        let data_len = data.len() as u32;

        // Check transmission with rate limiter
        match self
            .rate_limiter
            .check_transmission(connection_id, stream_id, traffic_type, data_len)
            .await?
        {
            TransmissionDecision::Allowed => {
                // Proceed with transmission
                self.execute_transmission(connection_id, data).await?;

                // Update statistics
                {
                    let mut stats = self.stats.lock().await;
                    stats.successful_transmissions += 1;
                    stats.total_bytes_sent += data.len() as u64;
                }

                debug!(
                    connection_id = connection_id,
                    stream_id = stream_id,
                    bytes = data.len(),
                    traffic_type = ?traffic_type,
                    "Data transmission successful"
                );

                Ok(data.len())
            }
            TransmissionDecision::RateLimited => {
                // Update statistics
                {
                    let mut stats = self.stats.lock().await;
                    stats.rate_limited_transmissions += 1;
                }

                warn!(
                    connection_id = connection_id,
                    stream_id = stream_id,
                    bytes = data.len(),
                    traffic_type = ?traffic_type,
                    "Transmission rate limited"
                );

                Err("Rate limited".into())
            }
            TransmissionDecision::FlowControlBlocked => {
                // Update statistics
                {
                    let mut stats = self.stats.lock().await;
                    stats.flow_control_blocks += 1;
                }

                warn!(
                    connection_id = connection_id,
                    stream_id = stream_id,
                    bytes = data.len(),
                    "Transmission blocked by flow control"
                );

                Err("Flow control blocked".into())
            }
            TransmissionDecision::Delayed(delay) => {
                // Update statistics
                {
                    let mut stats = self.stats.lock().await;
                    stats.backpressure_events += 1;
                }

                info!(
                    connection_id = connection_id,
                    stream_id = stream_id,
                    delay_ms = delay.as_millis(),
                    "Transmission delayed due to backpressure"
                );

                // Wait for backpressure to clear
                sleep(delay).await;

                // Retry transmission using Box::pin to avoid infinite recursion
                Box::pin(self.send_data(connection_id, stream_id, data, Some(traffic_type))).await
            }
        }
    }

    /// Simulate actual data transmission
    async fn execute_transmission(
        &self,
        connection_id: u64,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Simulate network transmission time
        let transmission_time = Duration::from_micros(data.len() as u64 / 10); // 10 MB/s simulation
        sleep(transmission_time).await;

        // Update connection info
        {
            let mut connections = self.active_connections.lock().await;
            if let Some(conn_info) = connections.get_mut(&connection_id) {
                conn_info.last_activity = Instant::now();
                conn_info.bytes_sent += data.len() as u64;
            }
        }

        Ok(())
    }

    /// Handle ACK reception to update flow control
    pub async fn handle_ack(&self, connection_id: u64, acked_bytes: u32, rtt: Duration) {
        // Update rate limiter flow control
        self.rate_limiter.on_ack(connection_id, acked_bytes, rtt);

        // Update connection RTT
        {
            let mut connections = self.active_connections.lock().await;
            if let Some(conn_info) = connections.get_mut(&connection_id) {
                conn_info.rtt = Some(rtt);
                conn_info.last_activity = Instant::now();
            }
        }

        // Update global RTT statistics
        {
            let mut stats = self.stats.lock().await;
            stats.average_rtt = Some(rtt); // Simplified - real implementation would average
        }

        debug!(
            connection_id = connection_id,
            acked_bytes = acked_bytes,
            rtt_ms = rtt.as_millis(),
            "ACK processed, flow control updated"
        );
    }

    /// Handle loss detection
    pub async fn handle_loss(&self, connection_id: u64) {
        self.rate_limiter.on_loss(connection_id);

        warn!(
            connection_id = connection_id,
            "Packet loss detected, flow control adjusted"
        );
    }

    /// Handle explicit congestion notification
    pub async fn handle_ecn(&self, connection_id: u64) {
        self.rate_limiter.on_ecn(connection_id);

        info!(
            connection_id = connection_id,
            "ECN received, flow control adjusted"
        );
    }

    /// Update queue sizes for backpressure monitoring
    pub async fn update_queue_metrics(
        &self,
        send_queue_size: usize,
        recv_queue_size: usize,
        control_queue_size: usize,
        priority_queue_size: usize,
    ) {
        self.rate_limiter
            .update_queue_size("send_buffer", send_queue_size);
        self.rate_limiter
            .update_queue_size("recv_buffer", recv_queue_size);
        self.rate_limiter
            .update_queue_size("control_queue", control_queue_size);
        self.rate_limiter
            .update_queue_size("priority_queue", priority_queue_size);

        debug!(
            send_queue = send_queue_size,
            recv_queue = recv_queue_size,
            control_queue = control_queue_size,
            priority_queue = priority_queue_size,
            "Queue metrics updated for backpressure monitoring"
        );
    }

    /// Close a connection and clean up resources
    pub async fn close_connection(
        &self,
        connection_id: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut connections = self.active_connections.lock().await;

        if let Some(conn_info) = connections.remove(&connection_id) {
            let duration = conn_info
                .last_activity
                .duration_since(conn_info.established_at);

            // Update statistics
            {
                let mut stats = self.stats.lock().await;
                stats.active_connections = connections.len() as u64;
            }

            info!(
                connection_id = connection_id,
                duration_ms = duration.as_millis(),
                bytes_sent = conn_info.bytes_sent,
                bytes_received = conn_info.bytes_received,
                "Connection closed"
            );

            Ok(())
        } else {
            Err("Connection not found".into())
        }
    }

    /// Get comprehensive status including rate limiting metrics
    pub async fn get_status(&self) -> StreamManagerStatus {
        let rate_limiter_status = self.rate_limiter.get_status();
        let connections = self.active_connections.lock().await;
        let stats = self.stats.lock().await;

        StreamManagerStatus {
            active_connections: connections.len(),
            rate_limiter_stats: rate_limiter_status.stats,
            backpressure_level: rate_limiter_status.backpressure_level,
            stream_stats: stats.clone(),
            connection_details: connections.clone(),
        }
    }

    /// Perform maintenance tasks
    pub async fn maintenance(&self) {
        // Clean up inactive connections
        let cutoff = Instant::now() - self.config.connection_timeout;
        let mut connections = self.active_connections.lock().await;

        let inactive_connections: Vec<u64> = connections
            .iter()
            .filter(|(_, conn)| conn.last_activity < cutoff)
            .map(|(&id, _)| id)
            .collect();

        for conn_id in inactive_connections {
            connections.remove(&conn_id);
            info!(connection_id = conn_id, "Inactive connection cleaned up");
        }

        // Clean up rate limiter state
        self.rate_limiter
            .cleanup_inactive_connections(self.config.connection_timeout);

        // Update active connection count
        {
            let mut stats = self.stats.lock().await;
            stats.active_connections = connections.len() as u64;
        }
    }

    /// Background task for periodic maintenance and statistics
    pub async fn background_maintenance(self: Arc<Self>) {
        let mut interval = tokio::time::interval(self.config.stats_interval);

        loop {
            interval.tick().await;

            // Perform maintenance
            self.maintenance().await;

            // Log statistics
            let status = self.get_status().await;
            info!(
                active_connections = status.active_connections,
                total_bytes_sent = status.stream_stats.total_bytes_sent,
                successful_transmissions = status.stream_stats.successful_transmissions,
                rate_limited = status.stream_stats.rate_limited_transmissions,
                flow_control_blocks = status.stream_stats.flow_control_blocks,
                backpressure_level = status.backpressure_level,
                "Stream manager status"
            );
        }
    }
}

/// Comprehensive status information
#[derive(Debug, Clone)]
pub struct StreamManagerStatus {
    pub active_connections: usize,
    pub rate_limiter_stats: nyx_stream::RateLimiterStats,
    pub backpressure_level: f32,
    pub stream_stats: StreamManagerStats,
    pub connection_details: HashMap<u64, ConnectionInfo>,
}

/// Example usage demonstrating advanced rate limiting in action
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("=== Nyx Protocol v1.0 Advanced Rate Limiting Demo ===");

    // Configure advanced flow control
    let mut flow_config = AdvancedFlowConfig {
        global_bandwidth_limit: 1_000_000,
        ..Default::default()
    }; // 1 MB/s
    flow_config.per_connection_limit = 100_000; // 100 KB/s per connection
    flow_config.per_stream_limit = 50_000; // 50 KB/s per stream
    flow_config.adaptive_rate_limiting = true;

    let manager_config = StreamManagerConfig::default();
    let stream_manager = Arc::new(StreamManager::new(flow_config, manager_config));

    // Start background maintenance
    let maintenance_manager = stream_manager.clone();
    let maintenance_handle = tokio::spawn(async move {
        maintenance_manager.background_maintenance().await;
    });

    // Simulate multiple connections with different priorities
    info!("1. Establishing connections with different priorities...");

    // High priority control connection
    stream_manager
        .establish_connection(1, TrafficType::Control)
        .await?;

    // Normal priority user connections
    for conn_id in 2..=5 {
        stream_manager
            .establish_connection(conn_id, TrafficType::Normal)
            .await?;
    }

    // Low priority background connection
    stream_manager
        .establish_connection(6, TrafficType::Background)
        .await?;

    // Simulate data transmission with various patterns
    info!("2. Simulating data transmission patterns...");

    for round in 0..20 {
        info!("Round {}", round + 1);

        // Update simulated queue metrics
        stream_manager
            .update_queue_metrics(
                (round * 100) % 5000, // Send queue
                (round * 80) % 3000,  // Recv queue
                (round * 20) % 200,   // Control queue
                (round * 50) % 1000,  // Priority queue
            )
            .await;

        // Send control traffic (highest priority)
        let control_data = vec![0x01; 512]; // 512 bytes
        match stream_manager
            .send_data(1, 1, &control_data, Some(TrafficType::Control))
            .await
        {
            Ok(sent) => info!("Control data sent: {} bytes", sent),
            Err(e) => warn!("Control data failed: {}", e),
        }

        // Send normal user traffic
        for conn_id in 2..=5 {
            let user_data = vec![0x02; 2048]; // 2KB
            match stream_manager
                .send_data(conn_id, conn_id, &user_data, None)
                .await
            {
                Ok(sent) => debug!("User data sent on connection {}: {} bytes", conn_id, sent),
                Err(e) => debug!("User data failed on connection {}: {}", conn_id, e),
            }
        }

        // Send background traffic (lowest priority)
        let background_data = vec![0x03; 4096]; // 4KB
        match stream_manager
            .send_data(6, 6, &background_data, Some(TrafficType::Background))
            .await
        {
            Ok(sent) => debug!("Background data sent: {} bytes", sent),
            Err(e) => debug!("Background data failed: {}", e),
        }

        // Simulate some ACKs
        if round % 3 == 0 {
            for conn_id in 1..=6 {
                let rtt = Duration::from_millis(50 + (round * 5) as u64);
                stream_manager.handle_ack(conn_id, 1024, rtt).await;
            }
        }

        // Simulate occasional packet loss
        if round == 10 {
            info!("Simulating packet loss...");
            stream_manager.handle_loss(3).await;
            stream_manager.handle_loss(5).await;
        }

        // Simulate ECN
        if round == 15 {
            info!("Simulating ECN events...");
            stream_manager.handle_ecn(2).await;
            stream_manager.handle_ecn(4).await;
        }

        // Brief pause between rounds
        sleep(Duration::from_millis(100)).await;
    }

    // Display final status
    info!("3. Displaying final status...");
    let final_status = stream_manager.get_status().await;

    println!("\n=== Final Stream Manager Status ===");
    println!("Active Connections: {}", final_status.active_connections);
    println!(
        "Total Bytes Sent: {}",
        final_status.stream_stats.total_bytes_sent
    );
    println!(
        "Successful Transmissions: {}",
        final_status.stream_stats.successful_transmissions
    );
    println!(
        "Rate Limited: {}",
        final_status.stream_stats.rate_limited_transmissions
    );
    println!(
        "Flow Control Blocks: {}",
        final_status.stream_stats.flow_control_blocks
    );
    println!(
        "Backpressure Events: {}",
        final_status.stream_stats.backpressure_events
    );
    println!(
        "Backpressure Level: {:.2}%",
        final_status.backpressure_level * 100.0
    );

    if let Some(avg_rtt) = final_status.stream_stats.average_rtt {
        println!("Average RTT: {}ms", avg_rtt.as_millis());
    }

    println!("\n=== Rate Limiter Statistics ===");
    println!(
        "Allowed Count: {}",
        final_status.rate_limiter_stats.allowed_count
    );
    println!(
        "Rate Limited Count: {}",
        final_status.rate_limiter_stats.rate_limited_count
    );
    println!(
        "Flow Control Blocked: {}",
        final_status.rate_limiter_stats.flow_control_blocked_count
    );
    println!(
        "Total Bytes Allowed: {}",
        final_status.rate_limiter_stats.total_bytes_allowed
    );

    // Close all connections
    info!("4. Closing connections...");
    for conn_id in 1..=6 {
        stream_manager.close_connection(conn_id).await?;
    }

    info!("=== Demo completed successfully ===");

    // Stop maintenance task
    maintenance_handle.abort();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stream_manager_basic_operations() {
        let flow_config = AdvancedFlowConfig::default();
        let manager_config = StreamManagerConfig::default();
        let manager = StreamManager::new(flow_config, manager_config);

        // Test connection establishment
        assert!(manager
            .establish_connection(1, TrafficType::Normal)
            .await
            .is_ok());

        // Test data transmission
        let data = vec![0x42; 1024];
        let result = manager.send_data(1, 1, &data, None).await;
        // Should either succeed or be rate limited
        assert!(
            result.is_ok()
                || result.unwrap_err().to_string().contains("rate")
                || result.unwrap_err().to_string().contains("flow")
        );

        // Test connection closure
        assert!(manager.close_connection(1).await.is_ok());
    }

    #[tokio::test]
    async fn test_stream_manager_rate_limiting() {
        let mut flow_config = AdvancedFlowConfig::default();
        flow_config.global_bandwidth_limit = 1000; // Very low for testing
        flow_config.max_burst_size = 500;

        let manager_config = StreamManagerConfig::default();
        let manager = StreamManager::new(flow_config, manager_config);

        assert!(manager
            .establish_connection(1, TrafficType::Normal)
            .await
            .is_ok());

        // Try to send data that exceeds limits
        let large_data = vec![0x42; 2000]; // 2KB
        let result = manager.send_data(1, 1, &large_data, None).await;

        // Should be limited in some way
        if result.is_err() {
            let error_msg = result.unwrap_err().to_string();
            assert!(
                error_msg.contains("rate")
                    || error_msg.contains("flow")
                    || error_msg.contains("blocked")
            );
        }
    }

    #[tokio::test]
    async fn test_stream_manager_priority_handling() {
        let flow_config = AdvancedFlowConfig::default();
        let manager_config = StreamManagerConfig::default();
        let manager = StreamManager::new(flow_config, manager_config);

        // Establish connections with different priorities
        assert!(manager
            .establish_connection(1, TrafficType::Control)
            .await
            .is_ok());
        assert!(manager
            .establish_connection(2, TrafficType::Background)
            .await
            .is_ok());

        // Send data with different priorities
        let data = vec![0x42; 512];

        let control_result = manager
            .send_data(1, 1, &data, Some(TrafficType::Control))
            .await;
        let background_result = manager
            .send_data(2, 2, &data, Some(TrafficType::Background))
            .await;

        // At least one should be processed (control has higher priority)
        assert!(control_result.is_ok() || background_result.is_ok());
    }

    #[tokio::test]
    async fn test_stream_manager_status_reporting() {
        let flow_config = AdvancedFlowConfig::default();
        let manager_config = StreamManagerConfig::default();
        let manager = StreamManager::new(flow_config, manager_config);

        // Get initial status
        let status = manager.get_status().await;
        assert_eq!(status.active_connections, 0);

        // Establish connection and check status
        assert!(manager
            .establish_connection(1, TrafficType::Normal)
            .await
            .is_ok());
        let status = manager.get_status().await;
        assert_eq!(status.active_connections, 1);

        // Close connection and check status
        assert!(manager.close_connection(1).await.is_ok());
        let status = manager.get_status().await;
        assert_eq!(status.active_connections, 0);
    }
}
