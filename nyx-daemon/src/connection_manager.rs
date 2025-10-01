//! Connection Manager
//!
//! Manages connection-level state including:
//! - Congestion control (BBR algorithm)
//! - RTT estimation (EWMA with min/max tracking)
//! - Send rate limiting (token bucket)
//! - ACK/STREAM frame processing
//!
//! Design decisions:
//! - BBR over Cubic: Modern algorithm optimized for varied network conditions
//! - EWMA for RTT: Balance between responsiveness and stability
//! - Token bucket: Simple and effective rate limiting

#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Connection identifier
pub type ConnectionId = u32;

/// Connection Manager configuration
#[derive(Debug, Clone)]
pub struct ConnectionManagerConfig {
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    /// Initial congestion window (in packets)
    pub initial_cwnd: usize,
    /// Maximum congestion window (in packets)
    pub max_cwnd: usize,
    /// RTT smoothing factor (0.0-1.0, typical 0.125)
    pub rtt_alpha: f64,
    /// Token bucket rate (bytes per second)
    pub rate_limit_bps: u64,
    /// Token bucket capacity (bytes)
    pub rate_limit_capacity: u64,
}

impl Default for ConnectionManagerConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            initial_cwnd: 10,        // 10 packets (RFC 6928)
            max_cwnd: 1000,          // Cap at 1000 packets
            rtt_alpha: 0.125,        // Standard TCP alpha
            rate_limit_bps: 100_000_000,  // 100 Mbps
            rate_limit_capacity: 1_000_000, // 1 MB burst
        }
    }
}

/// BBR congestion control state
#[derive(Debug, Clone)]
pub struct BbrState {
    /// Current congestion window (packets)
    pub cwnd: usize,
    /// Bottleneck bandwidth estimate (bytes/sec)
    pub btlbw: u64,
    /// Round-trip propagation time (ms)
    pub rtprop: Duration,
    /// Last update timestamp
    pub last_update: Instant,
    /// Pacing gain (multiplicative factor)
    pub pacing_gain: f64,
    /// Cwnd gain (multiplicative factor)
    pub cwnd_gain: f64,
}

impl Default for BbrState {
    fn default() -> Self {
        Self {
            cwnd: 10,
            btlbw: 1_000_000, // 1 Mbps initial estimate
            rtprop: Duration::from_millis(100),
            last_update: Instant::now(),
            pacing_gain: 1.0,
            cwnd_gain: 2.0,
        }
    }
}

/// RTT estimator using EWMA
#[derive(Debug, Clone)]
pub struct RttEstimator {
    /// Smoothed RTT (SRTT)
    pub srtt: Duration,
    /// RTT variance (RTTVAR)
    pub rttvar: Duration,
    /// Minimum observed RTT
    pub min_rtt: Duration,
    /// Maximum observed RTT
    pub max_rtt: Duration,
    /// Sample count
    pub sample_count: u64,
    /// Smoothing factor (alpha)
    alpha: f64,
}

impl RttEstimator {
    pub fn new(alpha: f64) -> Self {
        Self {
            srtt: Duration::from_millis(100), // Initial estimate
            rttvar: Duration::from_millis(50),
            min_rtt: Duration::from_secs(999999),
            max_rtt: Duration::ZERO,
            sample_count: 0,
            alpha,
        }
    }

    /// Update RTT estimate with new sample
    /// Implements RFC 6298 algorithm
    pub fn update(&mut self, sample: Duration) {
        self.sample_count += 1;

        // Track min/max
        if sample < self.min_rtt {
            self.min_rtt = sample;
        }
        if sample > self.max_rtt {
            self.max_rtt = sample;
        }

        if self.sample_count == 1 {
            // First measurement
            self.srtt = sample;
            self.rttvar = sample / 2;
        } else {
            // EWMA update
            let diff = if sample > self.srtt {
                sample - self.srtt
            } else {
                self.srtt - sample
            };

            self.rttvar = Duration::from_secs_f64(
                (1.0 - self.alpha) * self.rttvar.as_secs_f64() 
                + self.alpha * diff.as_secs_f64()
            );

            self.srtt = Duration::from_secs_f64(
                (1.0 - self.alpha) * self.srtt.as_secs_f64() 
                + self.alpha * sample.as_secs_f64()
            );
        }

        debug!(
            "RTT updated: sample={:?}, srtt={:?}, rttvar={:?}, min={:?}, max={:?}",
            sample, self.srtt, self.rttvar, self.min_rtt, self.max_rtt
        );
    }

    /// Calculate retransmission timeout (RTO)
    /// RTO = SRTT + 4 * RTTVAR (RFC 6298)
    pub fn rto(&self) -> Duration {
        let rto = self.srtt + self.rttvar * 4;
        // Clamp to [1s, 60s]
        rto.clamp(Duration::from_secs(1), Duration::from_secs(60))
    }
}

/// Token bucket for rate limiting
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Current token count (bytes)
    tokens: u64,
    /// Maximum capacity (bytes)
    capacity: u64,
    /// Refill rate (bytes/sec)
    rate: u64,
    /// Last refill timestamp
    last_refill: Instant,
}

impl TokenBucket {
    pub fn new(rate: u64, capacity: u64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            rate,
            last_refill: Instant::now(),
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let new_tokens = (elapsed.as_secs_f64() * self.rate as f64) as u64;

        if new_tokens > 0 {
            self.tokens = (self.tokens + new_tokens).min(self.capacity);
            self.last_refill = now;
        }
    }

    /// Try to consume tokens (returns true if successful)
    pub fn consume(&mut self, amount: u64) -> bool {
        self.refill();

        if self.tokens >= amount {
            self.tokens -= amount;
            true
        } else {
            false
        }
    }

    /// Get current token count
    pub fn available(&mut self) -> u64 {
        self.refill();
        self.tokens
    }
}

/// Connection state
pub struct Connection {
    pub id: ConnectionId,
    pub bbr: BbrState,
    pub rtt: RttEstimator,
    pub rate_limiter: TokenBucket,
    pub created_at: Instant,
    pub last_activity: Instant,
    /// Bytes transmitted
    pub bytes_tx: u64,
    /// Bytes received
    pub bytes_rx: u64,
    /// Packets transmitted
    pub packets_tx: u64,
    /// Packets received
    pub packets_rx: u64,
    /// Retransmission queue (packet_id, data, timestamp)
    pub retx_queue: VecDeque<(u64, Vec<u8>, Instant)>,
}

impl Connection {
    pub fn new(id: ConnectionId, config: &ConnectionManagerConfig) -> Self {
        Self {
            id,
            bbr: BbrState::default(),
            rtt: RttEstimator::new(config.rtt_alpha),
            rate_limiter: TokenBucket::new(config.rate_limit_bps, config.rate_limit_capacity),
            created_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_tx: 0,
            bytes_rx: 0,
            packets_tx: 0,
            packets_rx: 0,
            retx_queue: VecDeque::new(),
        }
    }

    /// Update BBR state based on ACK
    pub fn on_ack(&mut self, acked_bytes: u64, rtt_sample: Duration) {
        self.last_activity = Instant::now();
        self.bytes_rx += acked_bytes;

        // Update RTT estimate
        self.rtt.update(rtt_sample);

        // Update BBR bandwidth estimate
        let elapsed = self.last_activity.duration_since(self.bbr.last_update);
        if elapsed.as_millis() > 0 {
            let bw = (acked_bytes * 1000) / elapsed.as_millis() as u64;
            // EWMA for bandwidth
            self.bbr.btlbw = ((self.bbr.btlbw as f64 * 0.875) + (bw as f64 * 0.125)) as u64;
        }

        // Update RTprop (minimum RTT observed in last 10s)
        if rtt_sample < self.bbr.rtprop 
            || self.last_activity.duration_since(self.bbr.last_update) > Duration::from_secs(10) 
        {
            self.bbr.rtprop = rtt_sample;
        }

        // Update cwnd using BBR algorithm
        let bdp = (self.bbr.btlbw as f64 * self.bbr.rtprop.as_secs_f64()) as usize / 1500; // Assume 1500 byte MTU
        self.bbr.cwnd = ((bdp as f64 * self.bbr.cwnd_gain) as usize).max(4); // Min 4 packets

        self.bbr.last_update = self.last_activity;

        debug!(
            "Connection {} ACK: cwnd={}, btlbw={}, rtprop={:?}, srtt={:?}",
            self.id, self.bbr.cwnd, self.bbr.btlbw, self.bbr.rtprop, self.rtt.srtt
        );
    }

    /// Check if connection can send (within cwnd and rate limit)
    pub fn can_send(&mut self, bytes: u64) -> bool {
        // Check rate limiter
        if !self.rate_limiter.consume(bytes) {
            return false;
        }

        // Check cwnd (simplified - in production would track in-flight bytes)
        let in_flight_estimate = self.retx_queue.len();
        if in_flight_estimate >= self.bbr.cwnd {
            return false;
        }

        true
    }

    /// Record packet transmission
    pub fn on_send(&mut self, packet_id: u64, data: Vec<u8>) {
        self.last_activity = Instant::now();
        self.bytes_tx += data.len() as u64;
        self.packets_tx += 1;

        // Add to retransmission queue
        self.retx_queue.push_back((packet_id, data, Instant::now()));

        // Limit queue size
        while self.retx_queue.len() > 1000 {
            self.retx_queue.pop_front();
        }
    }

    /// Remove ACKed packets from retx queue
    pub fn on_packet_acked(&mut self, packet_id: u64) {
        self.retx_queue.retain(|(id, _, _)| *id != packet_id);
    }
}

/// Connection Manager
pub struct ConnectionManager {
    connections: Arc<RwLock<HashMap<ConnectionId, Connection>>>,
    config: ConnectionManagerConfig,
    next_connection_id: Arc<RwLock<ConnectionId>>,
}

impl ConnectionManager {
    pub fn new(config: ConnectionManagerConfig) -> Self {
        info!("ConnectionManager initialized with config: {:?}", config);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            config,
            next_connection_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Create a new connection
    pub async fn create_connection(&self) -> Result<ConnectionId, ConnectionError> {
        let mut conns = self.connections.write().await;

        if conns.len() >= self.config.max_connections {
            return Err(ConnectionError::TooManyConnections);
        }

        let mut next_id = self.next_connection_id.write().await;
        let conn_id = *next_id;
        *next_id += 1;

        let conn = Connection::new(conn_id, &self.config);
        conns.insert(conn_id, conn);

        info!("Created connection {}", conn_id);
        Ok(conn_id)
    }

    /// Close a connection
    pub async fn close_connection(&self, conn_id: ConnectionId) -> Result<(), ConnectionError> {
        let mut conns = self.connections.write().await;
        
        if conns.remove(&conn_id).is_some() {
            info!("Closed connection {}", conn_id);
            Ok(())
        } else {
            Err(ConnectionError::ConnectionNotFound)
        }
    }

    /// Get connection status
    pub async fn get_connection_status(&self, conn_id: ConnectionId) -> Option<ConnectionStatus> {
        let conns = self.connections.read().await;
        conns.get(&conn_id).map(|conn| ConnectionStatus {
            id: conn.id,
            age: conn.created_at.elapsed(),
            idle_time: conn.last_activity.elapsed(),
            cwnd: conn.bbr.cwnd,
            btlbw: conn.bbr.btlbw,
            srtt: conn.rtt.srtt,
            min_rtt: conn.rtt.min_rtt,
            max_rtt: conn.rtt.max_rtt,
            bytes_tx: conn.bytes_tx,
            bytes_rx: conn.bytes_rx,
            packets_tx: conn.packets_tx,
            packets_rx: conn.packets_rx,
            retx_queue_len: conn.retx_queue.len(),
        })
    }

    /// Process ACK frame
    pub async fn process_ack(
        &self,
        conn_id: ConnectionId,
        acked_packets: Vec<u64>,
        acked_bytes: u64,
        rtt_sample: Duration,
    ) -> Result<(), ConnectionError> {
        let mut conns = self.connections.write().await;
        
        let conn = conns.get_mut(&conn_id)
            .ok_or(ConnectionError::ConnectionNotFound)?;

        // Update connection state
        conn.on_ack(acked_bytes, rtt_sample);

        // Remove ACKed packets from retx queue
        for packet_id in acked_packets {
            conn.on_packet_acked(packet_id);
        }

        Ok(())
    }

    /// Check if connection can send
    pub async fn can_send(&self, conn_id: ConnectionId, bytes: u64) -> Result<bool, ConnectionError> {
        let mut conns = self.connections.write().await;
        
        let conn = conns.get_mut(&conn_id)
            .ok_or(ConnectionError::ConnectionNotFound)?;

        Ok(conn.can_send(bytes))
    }

    /// Record packet transmission
    pub async fn on_send(
        &self,
        conn_id: ConnectionId,
        packet_id: u64,
        data: Vec<u8>,
    ) -> Result<(), ConnectionError> {
        let mut conns = self.connections.write().await;
        
        let conn = conns.get_mut(&conn_id)
            .ok_or(ConnectionError::ConnectionNotFound)?;

        conn.on_send(packet_id, data);
        Ok(())
    }

    /// Get all connection IDs
    pub async fn list_connections(&self) -> Vec<ConnectionId> {
        let conns = self.connections.read().await;
        conns.keys().copied().collect()
    }
}

/// Connection status (for API exposure)
#[derive(Debug, Clone)]
pub struct ConnectionStatus {
    pub id: ConnectionId,
    pub age: Duration,
    pub idle_time: Duration,
    pub cwnd: usize,
    pub btlbw: u64,
    pub srtt: Duration,
    pub min_rtt: Duration,
    pub max_rtt: Duration,
    pub bytes_tx: u64,
    pub bytes_rx: u64,
    pub packets_tx: u64,
    pub packets_rx: u64,
    pub retx_queue_len: usize,
}

/// Connection errors
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Connection not found")]
    ConnectionNotFound,

    #[error("Too many connections")]
    TooManyConnections,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rtt_estimator() {
        let mut est = RttEstimator::new(0.125);

        // First sample
        est.update(Duration::from_millis(100));
        assert_eq!(est.srtt, Duration::from_millis(100));
        assert_eq!(est.min_rtt, Duration::from_millis(100));
        assert_eq!(est.max_rtt, Duration::from_millis(100));

        // Second sample
        est.update(Duration::from_millis(120));
        assert!(est.srtt > Duration::from_millis(100));
        assert!(est.srtt < Duration::from_millis(120));
        assert_eq!(est.min_rtt, Duration::from_millis(100));
        assert_eq!(est.max_rtt, Duration::from_millis(120));

        // RTO should be reasonable
        let rto = est.rto();
        assert!(rto >= Duration::from_secs(1));
        assert!(rto <= Duration::from_secs(60));
    }

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(1000, 5000); // 1000 bytes/sec, 5000 capacity

        // Should succeed (has initial capacity)
        assert!(bucket.consume(5000));

        // Should fail (no tokens)
        assert!(!bucket.consume(1));

        // Wait for refill (simulated)
        std::thread::sleep(Duration::from_millis(100));
        
        // Should have ~100 tokens
        assert!(bucket.available() > 0);
    }

    #[tokio::test]
    async fn test_connection_lifecycle() {
        let manager = ConnectionManager::new(ConnectionManagerConfig::default());

        // Create connection
        let conn_id = manager.create_connection().await.unwrap();
        assert_eq!(conn_id, 1);

        // Get status
        let status = manager.get_connection_status(conn_id).await;
        assert!(status.is_some());
        let status = status.unwrap();
        assert_eq!(status.id, conn_id);
        assert_eq!(status.cwnd, 10); // Initial cwnd

        // Close connection
        manager.close_connection(conn_id).await.unwrap();

        // Should be gone
        let status = manager.get_connection_status(conn_id).await;
        assert!(status.is_none());
    }

    #[tokio::test]
    async fn test_ack_processing() {
        let manager = ConnectionManager::new(ConnectionManagerConfig::default());
        let conn_id = manager.create_connection().await.unwrap();

        // Process ACK
        manager.process_ack(
            conn_id,
            vec![1, 2, 3],
            4500, // 3 packets * 1500 bytes
            Duration::from_millis(50),
        ).await.unwrap();

        // Check updated status
        let status = manager.get_connection_status(conn_id).await.unwrap();
        assert_eq!(status.bytes_rx, 4500);
        assert_eq!(status.srtt, Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_send_flow_control() {
        let manager = ConnectionManager::new(ConnectionManagerConfig::default());
        let conn_id = manager.create_connection().await.unwrap();

        // Should be able to send
        assert!(manager.can_send(conn_id, 1500).await.unwrap());

        // Record send
        manager.on_send(conn_id, 1, vec![0u8; 1500]).await.unwrap();

        // Check status
        let status = manager.get_connection_status(conn_id).await.unwrap();
        assert_eq!(status.packets_tx, 1);
        assert_eq!(status.bytes_tx, 1500);
        assert_eq!(status.retx_queue_len, 1);
    }

    #[tokio::test]
    async fn test_max_connections() {
        let config = ConnectionManagerConfig {
            max_connections: 2,
            ..Default::default()
        };
        let manager = ConnectionManager::new(config);

        // Create 2 connections
        manager.create_connection().await.unwrap();
        manager.create_connection().await.unwrap();

        // Third should fail
        let result = manager.create_connection().await;
        assert!(matches!(result, Err(ConnectionError::TooManyConnections)));
    }

    #[tokio::test]
    async fn test_list_connections() {
        let manager = ConnectionManager::new(ConnectionManagerConfig::default());

        let conn1 = manager.create_connection().await.unwrap();
        let conn2 = manager.create_connection().await.unwrap();

        let list = manager.list_connections().await;
        assert_eq!(list.len(), 2);
        assert!(list.contains(&conn1));
        assert!(list.contains(&conn2));
    }
}
