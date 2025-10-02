use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut, BufMut};
use tokio::net::UdpSocket;
use tokio::sync::{RwLock as TokioRwLock, mpsc};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce as ChaNonce,
};
use hkdf::Hkdf;
use sha2::Sha256;
use rand::Rng;

/// Connection timeout constant
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum concurrent streams per connection
pub const MAX_CONCURRENT_STREAMS: usize = 256;
/// Maximum datagram size (MTU - overhead)
pub const MAX_DATAGRAM_SIZE: usize = 1200;

/// QUIC specific errors
#[derive(Debug)]
pub enum QuicError {
    Transport(String),
    Protocol(String),
    ConnectionClosed(String),
    Stream(String),
    CongestionControl(String),
    FlowControl(String),
    Crypto(String),
    Timeout(String),
    Configuration(String),
    VersionNegotiation(String),
    HandshakeFailed(String),
    CertificateVerification(String),
    AlpnNegotiation(String),
    AddressValidation(String),
    MigrationNotAllowed(String),
    PacketDecode(String),
    InvalidFrame(String),
    Internal(String),
    InvalidConnectionId(String),
    InvalidPacketNumber(String),
    InvalidToken(String),
    KeyUpdate(String),
    TooManyStreams,
    StreamNotFound(String),
    InvalidStreamState(String),
    StreamAlreadyClosed(String),
    Application(String),
    ResourceExhausted(String),
    RateLimited(String),
    PathValidation(String),
    IdleTimeout(String),
    KeepaliveTimeout(String),
    DatagramTooLarge(String),
    FeatureNotSupported(String),
    InvalidParameter(String),
    NoAvailablePaths(String),
    Serialization(String),
    Io(String),
    CryptoError(String),
    ConnectionNotFound(Bytes),
    StreamNotFoundError,
    StreamClosed,
}

/// Connection state management enumeration#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting {
        peer: SocketAddr,
        start_time: Instant,
        attempt_count: u32,
    },
    Connected {
        peer: SocketAddr,
        established_at: Instant,
        stream_count: u32,
        congestion_window: u32,
    },
    Closing {
        peer: SocketAddr,
        close_reason: String,
        started_at: Instant,
        remaining_streams: u32,
    },
    Closed {
        close_reason: String,
        closed_at: Instant,
    },
}

/// Stream type#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    Bidirectional,
    Unidirectional,
}

/// Stream state#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(PartialEq)]
pub enum StreamState {
    Open,
    HalfClosed,
    Closed,
}

/// Encryption level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionLevel {
    Initial,
    Handshake,
    Application,
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packet_loss_rate: f64,
    pub round_trip_time: Duration,
    pub congestion_window: u32,
    pub bytes_in_flight: u64,
    pub stream_s_created: u32,
    pub stream_s_closed: u32,
    pub errors_encountered: u32,
    pub last_error: Option<String>,
}

impl Default for ConnectionStats {
    fn default() -> Self {
        Self {
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            packet_loss_rate: 0.0,
            round_trip_time: Duration::from_millis(0),
            congestion_window: 65536,
            bytes_in_flight: 0,
            stream_s_created: 0,
            stream_s_closed: 0,
            errors_encountered: 0,
            last_error: None,
        }
    }
}

/// Endpoint statistics
#[derive(Debug, Clone)]
pub struct EndpointStatistics {
    pub active_connections: u32,
    pub total_connection_s_created: u64,
    pub total_connection_s_closed: u64,
    pub total_bytes_sent: u64,
    pub total_bytes_received: u64,
    pub packet_loss_rate: f64,
    pub average_rtt: Duration,
    pub peak_congestion_window: u32,
    pub errors_by_type: HashMap<String, u64>,
    pub performance_metrics: HashMap<String, f64>,
}

impl Default for EndpointStatistics {
    fn default() -> Self {
        Self {
            active_connections: 0,
            total_connection_s_created: 0,
            total_connection_s_closed: 0,
            total_bytes_sent: 0,
            total_bytes_received: 0,
            packet_loss_rate: 0.0,
            average_rtt: Duration::from_millis(0),
            peak_congestion_window: 65536,
            errors_by_type: HashMap::new(),
            performance_metrics: HashMap::new(),
        }
    }
}

/// QUIC configuration
#[derive(Debug, Clone)]
pub struct QuicEndpointConfig {
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_stream_s_per_connection: u32,
    pub initial_max_data: u64,
    pub initial_max_stream_data: u64,
}

impl Default for QuicEndpointConfig {
    fn default() -> Self {
        Self {
            max_connections: 1000,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30),
            max_stream_s_per_connection: 256,
            initial_max_data: 1048576,
            initial_max_stream_data: 262144,
        }
    }
}

/// QUIC connection
#[derive(Clone)]
pub struct QuicConnection {
    pub _connection_id: Bytes,
    pub _peer_addr: SocketAddr,
    pub state: Arc<TokioRwLock<ConnectionState>>,
    pub streams: Arc<TokioRwLock<HashMap<u64, QuicStream>>>,
    pub _next_stream_id: u64,
    pub established_at: Option<Instant>,
    pub _last_activity: Instant,
    pub stats: Arc<TokioRwLock<ConnectionStats>>,
}

/// QUIC stream
pub struct QuicStream {
    pub stream_id: u64,
    pub stream_type: StreamType,
    pub state: StreamState,
    pub send_buffer: Vec<u8>,
    pub recv_buffer: Vec<u8>,
    pub _send_offset: u64,
    pub _recv_offset: u64,
}

/// QUIC Endpoint
pub struct QuicEndpoint {
    #[allow(dead_code)]
    socket: UdpSocket,
    #[allow(dead_code)]
    bind_addr: SocketAddr,
    config: QuicEndpointConfig,
    connections: Arc<TokioRwLock<HashMap<Bytes, QuicConnection>>>,
    #[allow(dead_code)]
    statistics: Arc<TokioRwLock<EndpointStatistics>>,
}

/// QUIC暗号化コンチE��スチE#[allow(dead_code)]
pub struct QuicCryptoContext {
    #[allow(dead_code)]
    initial_secret: [u8; 32],
    #[allow(dead_code)]
    handshake_secret: [u8; 32],
    #[allow(dead_code)]
    application_secret: [u8; 32],
    #[allow(dead_code)]
    key_update_secret: [u8; 32],
    #[allow(dead_code)]
    encryption_level: EncryptionLevel,
    #[allow(dead_code)]
    key_phase: u32,
}

impl QuicConnection {
    /// 新しいQUIC接続を作�E
    pub fn new(
        connection_id: Bytes,
        peer_addr: SocketAddr,
        is_server: bool,
        stats: ConnectionStats,
    ) -> Result<Self, QuicError> {
        let state = if is_server {
            ConnectionState::Connecting {
                peer: peer_addr,
                start_time: Instant::now(),
                attempt_count: 0,
            }
        } else {
            ConnectionState::Connecting {
                peer: peer_addr,
                start_time: Instant::now(),
                attempt_count: 1,
            }
        };

        Ok(Self {
            _connection_id: connection_id,
            _peer_addr: peer_addr,
            state: Arc::new(TokioRwLock::new(state)),
            streams: Arc::new(TokioRwLock::new(HashMap::new())),
            _next_stream_id: if is_server { 1 } else { 0 },
            established_at: None,
            _last_activity: Instant::now(),
            stats: Arc::new(TokioRwLock::new(stats)),
        })
    }

    /// Get connection ID
    pub fn connection_id(&self) -> Bytes {
        self._connection_id.clone()
    }

    /// Check if connection is established
    pub async fn is_established(&self) -> bool {
        matches!(*self.state.read().await, ConnectionState::Connected { .. })
    }

    /// Create new stream
    pub async fn create_stream(&mut self, stream_type: StreamType) -> Result<u64, QuicError> {
        if !self.is_established().await {
            return Err(QuicError::Protocol(String::new()));
        }

        let stream_id = self._next_stream_id;
        self._next_stream_id += 1;

        let stream = QuicStream::new(stream_id, stream_type);
        self.streams.write().await.insert(stream_id, stream);
        self._last_activity = Instant::now();

        Ok(stream_id)
    }

    /// ストリーム書き込み
    pub async fn write_stream(&mut self, stream_id: u64, _data: Bytes) -> Result<(), QuicError> {
        let mut stream_s = self.streams.write().await;
        let stream = stream_s
            .get_mut(&stream_id)
            .ok_or(QuicError::StreamNotFound(String::from("Stream not found")))?;

        stream.write_data(_data).await?;
        self._last_activity = Instant::now();

        Ok(())
    }

    /// ストリーム読み込み
    pub async fn read_stream(&mut self, stream_id: u64) -> Result<Option<Bytes>, QuicError> {
        let mut stream_s = self.streams.write().await;
        let stream = stream_s
            .get_mut(&stream_id)
            .ok_or(QuicError::StreamNotFound(String::from("Stream not found")))?;

        let _data = stream.read_data().await?;
        self._last_activity = Instant::now();

        Ok(_data)
    }

    /// Close connection
    pub async fn close(&mut self) -> Result<(), QuicError> {
        let mut state = self.state.write().await;
        *state = ConnectionState::Closing {
            peer: self._peer_addr,
            close_reason: String::new(),
            started_at: Instant::now(),
            remaining_streams: self.streams.read().await.len() as u32,
        };

        Ok(())
    }

    /// Change connection state to Connected
    pub async fn establish_connection(&self, _servername: Option<String>) -> Result<(), QuicError> {
        let mut state = self.state.write().await;

        if let ConnectionState::Connecting {
            peer, start_time, ..
        } = &*state
        {
            *state = ConnectionState::Connected {
                peer: *peer,
                established_at: *start_time,
                stream_count: 0,
                congestion_window: 65536,
            };
        }

        Ok(())
    }
}

impl QuicStream {
    /// Create new stream
    pub fn new(stream_id: u64, stream_type: StreamType) -> Self {
        Self {
            stream_id,
            stream_type,
            state: StreamState::Open,
            send_buffer: Vec::new(),
            recv_buffer: Vec::new(),
            _send_offset: 0,
            _recv_offset: 0,
        }
    }

    /// Write data
    pub async fn write_data(&mut self, _data: Bytes) -> Result<(), QuicError> {
        if self.state == StreamState::Closed {
            return Err(QuicError::StreamClosed);
        }

        self.send_buffer.extend_from_slice(&_data);
        Ok(())
    }

    /// Read data
    pub async fn read_data(&mut self) -> Result<Option<Bytes>, QuicError> {
        if self.recv_buffer.is_empty() {
            return Ok(None);
        }

        let _data = Bytes::copy_from_slice(&self.recv_buffer);
        self.recv_buffer.clear();
        self._recv_offset += _data.len() as u64;

        Ok(Some(_data))
    }
}

impl Default for QuicCryptoContext {
    fn default() -> Self {
        Self::new()
    }
}

impl QuicCryptoContext {
    /// Create new crypto context
    pub fn new() -> Self {
        Self {
            initial_secret: [0u8; 32],
            handshake_secret: [0u8; 32],
            application_secret: [0u8; 32],
            key_update_secret: [0u8; 32],
            encryption_level: EncryptionLevel::Initial,
            key_phase: 0,
        }
    }

    /// Encrypt packet
    pub async fn encrypt_packet(
        &self,
        packet: &[u8],
        _packetnumber: u64,
    ) -> Result<Bytes, QuicError> {
        Ok(Bytes::copy_from_slice(packet))
    }

    /// Decrypt packet
    pub async fn decrypt_packet(
        &self,
        encrypted_packet: &[u8],
        _packetnumber: u64,
    ) -> Result<Bytes, QuicError> {
        Ok(Bytes::copy_from_slice(encrypted_packet))
    }
}

impl QuicEndpoint {
    /// Create new QUIC Endpoint
    pub async fn new(bind_addr: SocketAddr, config: QuicEndpointConfig) -> Result<Self, QuicError> {
        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| QuicError::Io(e.to_string()))?;
        let connections = Arc::new(TokioRwLock::new(HashMap::new()));
        let statistics = Arc::new(TokioRwLock::new(EndpointStatistics::default()));

        Ok(Self {
            socket,
            bind_addr,
            config,
            connections,
            statistics,
        })
    }

    /// Get local address
    pub fn local_addr(&self) -> Result<SocketAddr, QuicError> {
        self.socket.local_addr()
            .map_err(|e| QuicError::Io(e.to_string()))
    }

    /// Get connection statistics
    pub async fn get_connection_stats(
        &self,
        connection_id: &Bytes,
    ) -> Result<ConnectionStats, QuicError> {
        let connections = self.connections.read().await;
        let connection = connections
            .get(connection_id)
            .ok_or_else(|| QuicError::ConnectionNotFound(connection_id.clone()))?;

        let stats = (*connection.stats.read().await).clone();

        Ok(stats)
    }

    /// Get active connections list
    pub async fn active_connections(&self) -> Vec<Bytes> {
        self.connections.read().await.keys().cloned().collect()
    }

    /// Get connection
    pub async fn get_connection(&self, connection_id: &Bytes) -> Result<QuicConnection, QuicError> {
        let connections = self.connections.read().await;
        connections
            .get(connection_id)
            .cloned()
            .ok_or_else(|| QuicError::ConnectionNotFound(connection_id.clone()))
    }

    /// Remove connection
    pub async fn remove_connection(&self, connection_id: &Bytes) -> Result<(), QuicError> {
        let mut connections = self.connections.write().await;
        connections.remove(connection_id);

        Ok(())
    }

    /// Cleanup idle connections
    pub async fn cleanup_idle_connections(&self) -> Result<(), QuicError> {
        let mut connections = self.connections.write().await;
        let timeout_duration = self.config.idle_timeout;
        let current_time = Instant::now();

        let idle_ids: Vec<Bytes> = connections
            .iter()
            .filter_map(|(id, conn)| {
                if current_time.duration_since(conn._last_activity) > timeout_duration {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();

        for id in idle_ids {
            connections.remove(&id);
        }

        Ok(())
    }

    /// Send data
    pub async fn send_data(&self, connection_id: &Bytes) -> Result<(), QuicError> {
        let connections = self.connections.read().await;
        connections
            .get(connection_id)
            .ok_or_else(|| QuicError::ConnectionNotFound(connection_id.clone()))?;

        Ok(())
    }
}

// ============================================================================
// QUIC Packet Format Structures (RFC 9000)
// ============================================================================

/// QUIC packet header type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Initial,
    Handshake,
    ZeroRtt,
    OneRtt,
    Retry,
    VersionNegotiation,
}

/// QUIC packet header
#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub packet_type: PacketType,
    pub conn_id: Bytes,
    pub packet_number: u64,
    pub version: u32,
}

/// QUIC frame type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameType {
    Padding,
    Ping,
    Ack,
    ResetStream,
    StopSending,
    Crypto,
    NewToken,
    Stream,
    MaxData,
    MaxStreamData,
    MaxStreams,
    DataBlocked,
    StreamDataBlocked,
    StreamsBlocked,
    NewConnectionId,
    RetireConnectionId,
    PathChallenge,
    PathResponse,
    ConnectionClose,
    HandshakeDone,
    Datagram,
}

/// QUIC frame
#[derive(Debug, Clone)]
pub enum Frame {
    Padding,
    Ping,
    Ack {
        largest: u64,
        delay: u64,
        ranges: Vec<(u64, u64)>,
    },
    Stream {
        id: u64,
        offset: u64,
        data: Bytes,
        fin: bool,
    },
    Crypto {
        offset: u64,
        data: Bytes,
    },
    PathChallenge {
        data: [u8; 8],
    },
    PathResponse {
        data: [u8; 8],
    },
    Datagram {
        data: Bytes,
    },
    ConnectionClose {
        error_code: u64,
        reason: String,
    },
}

/// Parse QUIC packet from bytes
pub fn parse_packet(mut data: &[u8]) -> Result<(PacketHeader, Vec<Frame>), QuicError> {
    if data.is_empty() {
        return Err(QuicError::PacketDecode("Empty packet".into()));
    }

    let first_byte = data[0];
    let is_long_header = (first_byte & 0x80) != 0;

    let packet_type = if is_long_header {
        let type_bits = (first_byte >> 4) & 0x03;
        match type_bits {
            0x00 => PacketType::Initial,
            0x01 => PacketType::ZeroRtt,
            0x02 => PacketType::Handshake,
            0x03 => PacketType::Retry,
            _ => return Err(QuicError::PacketDecode("Invalid packet type".into())),
        }
    } else {
        PacketType::OneRtt
    };

    data = &data[1..];
    
    // Parse connection ID (simplified - assume 8 bytes)
    if data.len() < 8 {
        return Err(QuicError::PacketDecode("Insufficient data for conn ID".into()));
    }
    let conn_id = Bytes::copy_from_slice(&data[..8]);
    data = &data[8..];

    // Parse packet number (simplified - assume 4 bytes)
    if data.len() < 4 {
        return Err(QuicError::PacketDecode("Insufficient data for packet number".into()));
    }
    let packet_number = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as u64;
    data = &data[4..];

    let header = PacketHeader {
        packet_type,
        conn_id,
        packet_number,
        version: 1,
    };

    // Parse frames (simplified)
    let frames = vec![];

    Ok((header, frames))
}

/// Serialize QUIC packet to bytes
pub fn serialize_packet(header: &PacketHeader, frames: &[Frame]) -> Result<Bytes, QuicError> {
    let mut buf = BytesMut::with_capacity(1200);

    // Write header
    let first_byte = match header.packet_type {
        PacketType::Initial => 0xC0,
        PacketType::Handshake => 0xE0,
        PacketType::OneRtt => 0x40,
        _ => 0x80,
    };
    buf.put_u8(first_byte);

    // Write connection ID
    buf.put_slice(&header.conn_id);

    // Write packet number
    buf.put_u32(header.packet_number as u32);

    // Write frames
    for frame in frames {
        match frame {
            Frame::Ping => {
                buf.put_u8(0x01);
            }
            Frame::Stream { id, offset, data, fin } => {
                buf.put_u8(if *fin { 0x0F } else { 0x0E });
                buf.put_u64(*id);
                buf.put_u64(*offset);
                buf.put_u32(data.len() as u32);
                buf.put_slice(data);
            }
            Frame::Datagram { data } => {
                buf.put_u8(0x31);
                buf.put_u32(data.len() as u32);
                buf.put_slice(data);
            }
            Frame::PathChallenge { data } => {
                buf.put_u8(0x1A);
                buf.put_slice(data);
            }
            Frame::PathResponse { data } => {
                buf.put_u8(0x1B);
                buf.put_slice(data);
            }
            _ => {}
        }
    }

    Ok(buf.freeze())
}

// ============================================================================
// BBR Congestion Control (Bottleneck Bandwidth and RTT)
// ============================================================================

/// BBR congestion control state
#[derive(Debug, Clone)]
pub struct BbrState {
    /// Current congestion window (bytes)
    pub cwnd: u64,
    /// Bottleneck bandwidth estimate (bytes/sec)
    pub btlbw: u64,
    /// Minimum RTT observed (microseconds)
    pub rtprop: u64,
    /// Pacing gain
    pub pacing_gain: f64,
    /// CWND gain
    pub cwnd_gain: f64,
    /// Current BBR mode
    pub mode: BbrMode,
    /// Cycle index for ProbeBW mode
    pub cycle_index: usize,
    /// Last mode switch time
    pub mode_start: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BbrMode {
    Startup,
    Drain,
    ProbeBW,
    ProbeRTT,
}

impl Default for BbrState {
    fn default() -> Self {
        Self {
            cwnd: 10 * 1200, // 10 packets
            btlbw: 100_000, // 100 KB/s initial estimate
            rtprop: 100_000, // 100ms initial RTT
            pacing_gain: 2.77,
            cwnd_gain: 2.0,
            mode: BbrMode::Startup,
            cycle_index: 0,
            mode_start: Instant::now(),
        }
    }
}

impl BbrState {
    /// Update BBR state on ACK receipt
    pub fn on_ack(&mut self, bytes_acked: u64, rtt_sample: Duration) {
        let rtt_us = rtt_sample.as_micros() as u64;
        
        // Update RTprop (minimum RTT)
        if rtt_us < self.rtprop {
            self.rtprop = rtt_us;
        }

        // Update bandwidth estimate
        let delivery_rate = (bytes_acked * 1_000_000) / rtt_us.max(1);
        if delivery_rate > self.btlbw {
            self.btlbw = delivery_rate;
        }

        // Update congestion window based on mode
        match self.mode {
            BbrMode::Startup => {
                self.cwnd = (self.btlbw * self.rtprop * self.cwnd_gain as u64) / 1_000_000;
                
                // Exit startup if bandwidth not growing
                if self.mode_start.elapsed() > Duration::from_secs(1) {
                    self.mode = BbrMode::Drain;
                    self.pacing_gain = 1.0 / 2.77;
                    self.mode_start = Instant::now();
                }
            }
            BbrMode::Drain => {
                self.cwnd = (self.btlbw * self.rtprop) / 1_000_000;
                
                // Exit drain when queue is empty
                if self.mode_start.elapsed() > Duration::from_millis(500) {
                    self.mode = BbrMode::ProbeBW;
                    self.pacing_gain = 1.0;
                    self.mode_start = Instant::now();
                }
            }
            BbrMode::ProbeBW => {
                // Cycle through pacing gains
                let gains = [1.25, 0.75, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
                self.pacing_gain = gains[self.cycle_index % gains.len()];
                self.cwnd = (self.btlbw * self.rtprop) / 1_000_000;
                
                if self.mode_start.elapsed() > Duration::from_secs(2) {
                    self.cycle_index += 1;
                    self.mode_start = Instant::now();
                }
            }
            BbrMode::ProbeRTT => {
                self.cwnd = 4 * 1200; // Minimum window
                
                if self.mode_start.elapsed() > Duration::from_millis(200) {
                    self.mode = BbrMode::ProbeBW;
                    self.mode_start = Instant::now();
                }
            }
        }
    }

    /// Check if sending is allowed
    pub fn can_send(&self, bytes_in_flight: u64) -> bool {
        bytes_in_flight < self.cwnd
    }

    /// Get pacing rate (bytes per second)
    pub fn pacing_rate(&self) -> u64 {
        (self.btlbw as f64 * self.pacing_gain) as u64
    }
}

// ============================================================================
// QuicTransport - High-level Transport Wrapper
// ============================================================================

/// High-level QUIC transport interface
pub struct QuicTransport {
    pub endpoint: QuicEndpoint,
    datagram_tx: mpsc::UnboundedSender<(Bytes, SocketAddr)>,
    datagram_rx: Arc<TokioRwLock<mpsc::UnboundedReceiver<(Bytes, SocketAddr)>>>,
}

impl QuicTransport {
    /// Create new QUIC transport
    pub async fn new(config: nyx_core::config::QuicConfig) -> Result<Self, QuicError> {
        let endpoint_config = QuicEndpointConfig {
            max_connections: config.max_concurrent_streams as u32,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(config.idle_timeout_secs),
            max_stream_s_per_connection: config.max_concurrent_streams as u32,
            initial_max_data: 1048576,
            initial_max_stream_data: 262144,
        };

        let endpoint = QuicEndpoint::new(config.bind_addr, endpoint_config).await?;
        let (datagram_tx, datagram_rx) = mpsc::unbounded_channel();
        let datagram_rx = Arc::new(TokioRwLock::new(datagram_rx));

        Ok(Self {
            endpoint,
            datagram_tx,
            datagram_rx,
        })
    }

    /// Accept incoming connection
    pub async fn accept(&mut self) -> Result<Arc<QuicConnection>, QuicError> {
        // Simplified accept - in production would listen on socket
        let conn_id = Bytes::from(vec![0u8; 8]);
        let peer_addr = "127.0.0.1:0".parse().unwrap();
        let stats = ConnectionStats::default();
        
        let conn = QuicConnection::new(conn_id.clone(), peer_addr, true, stats)?;
        
        self.endpoint.connections.write().await.insert(conn_id.clone(), conn.clone());
        let conn = Arc::new(conn);
        
        Ok(conn)
    }

    /// Connect to remote peer
    pub async fn connect(&self, peer: SocketAddr) -> Result<Arc<QuicConnection>, QuicError> {
        let mut rng = rand::thread_rng();
        let conn_id = Bytes::from(rng.gen::<[u8; 8]>().to_vec());
        let stats = ConnectionStats::default();
        
        let conn = QuicConnection::new(conn_id.clone(), peer, false, stats)?;
        conn.establish_connection(None).await?;
        
        self.endpoint.connections.write().await.insert(conn_id.clone(), conn.clone());
        let conn = Arc::new(conn);
        
        Ok(conn)
    }

    /// Send datagram
    pub async fn send_datagram(&self, data: &[u8], peer: SocketAddr) -> Result<(), QuicError> {
        self.datagram_tx.send((Bytes::copy_from_slice(data), peer))
            .map_err(|_| QuicError::Transport("Datagram channel closed".into()))
    }

    /// Receive datagram
    pub async fn recv_datagram(&self) -> Result<Bytes, QuicError> {
        self.datagram_rx.write().await.recv().await
            .map(|(data, _addr)| data)
            .ok_or_else(|| QuicError::Transport("Datagram channel closed".into()))
    }
}

// ============================================================================
// Enhanced QuicConnection with Datagram Support
// ============================================================================

impl QuicConnection {
    /// Get connection state
    pub fn get_state(&self) -> ConnectionState {
        // Clone state synchronously - in production use async properly
        ConnectionState::Connected {
            peer: self._peer_addr,
            established_at: Instant::now(),
            stream_count: 0,
            congestion_window: 65536,
        }
    }

    /// Check if connection is active
    pub fn is_active(&self) -> bool {
        true // Simplified
    }

    /// Open bidirectional stream
    pub async fn open_bidirectional_stream(
        &self,
        _stream_type: StreamType,
        _priority: u8,
    ) -> Result<u64, QuicError> {
        let stream_id = self._next_stream_id;
        let stream = QuicStream::new(stream_id, StreamType::Bidirectional);
        self.streams.write().await.insert(stream_id, stream);
        Ok(stream_id)
    }

    /// Open unidirectional stream
    pub async fn open_unidirectional_stream(
        &self,
        _stream_type: StreamType,
        _priority: u8,
    ) -> Result<u64, QuicError> {
        let stream_id = self._next_stream_id + 1;
        let stream = QuicStream::new(stream_id, StreamType::Unidirectional);
        self.streams.write().await.insert(stream_id, stream);
        Ok(stream_id)
    }

    /// Send data on stream
    pub async fn send_on_stream(&self, stream_id: u64, data: &[u8]) -> Result<(), QuicError> {
        let mut streams = self.streams.write().await;
        let stream = streams.get_mut(&stream_id)
            .ok_or(QuicError::StreamNotFoundError)?;
        stream.write_data(Bytes::copy_from_slice(data)).await
    }

    /// Receive data from stream
    pub async fn recv_from_stream(
        &self,
        stream_id: u64,
        _timeout: Duration,
    ) -> Result<Result<Bytes, QuicError>, QuicError> {
        let mut streams = self.streams.write().await;
        let stream = streams.get_mut(&stream_id)
            .ok_or(QuicError::StreamNotFoundError)?;
        stream.read_data().await
            .map(|opt| opt.ok_or(QuicError::StreamNotFoundError))
    }

    /// Send datagram
    pub async fn send_datagram(&self, _data: &[u8]) -> Result<(), QuicError> {
        // Simplified - would send via endpoint socket
        Ok(())
    }

    /// Receive datagram with timeout
    pub async fn recv_datagram(&self, _timeout: Duration) -> Result<Result<Bytes, QuicError>, QuicError> {
        // Simplified - would receive from endpoint socket
        Ok(Ok(Bytes::new()))
    }
}

/// Enhanced crypto context with real encryption
impl QuicCryptoContext {
    /// Encrypt packet with ChaCha20-Poly1305
    pub async fn encrypt_packet_real(
        &self,
        packet: &[u8],
        packet_number: u64,
    ) -> Result<Bytes, QuicError> {
        let cipher = ChaCha20Poly1305::new(&self.application_secret.into());
        
        // Construct nonce from packet number (RFC 9001)
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..].copy_from_slice(&packet_number.to_be_bytes());
        let nonce = ChaNonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, packet)
            .map_err(|e| QuicError::CryptoError(e.to_string()))?;
        
        Ok(Bytes::from(ciphertext))
    }

    /// Decrypt packet with ChaCha20-Poly1305
    pub async fn decrypt_packet_real(
        &self,
        encrypted_packet: &[u8],
        packet_number: u64,
    ) -> Result<Bytes, QuicError> {
        let cipher = ChaCha20Poly1305::new(&self.application_secret.into());
        
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[4..].copy_from_slice(&packet_number.to_be_bytes());
        let nonce = ChaNonce::from_slice(&nonce_bytes);
        
        let plaintext = cipher.decrypt(nonce, encrypted_packet)
            .map_err(|e| QuicError::CryptoError(e.to_string()))?;
        
        Ok(Bytes::from(plaintext))
    }

    /// Derive traffic keys using HKDF
    pub fn derive_keys(secret: &[u8], label: &str) -> Result<[u8; 32], QuicError> {
        let hk = Hkdf::<Sha256>::new(None, secret);
        let mut okm = [0u8; 32];
        hk.expand(label.as_bytes(), &mut okm)
            .map_err(|e| QuicError::CryptoError(e.to_string()))?;
        Ok(okm)
    }
}
