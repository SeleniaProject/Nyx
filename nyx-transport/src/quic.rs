use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::net::UdpSocket;
use tokio::sync::RwLock as TokioRwLock;

/// QUIC固有のエラー型
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// 接続の状態管理を行うための列挙型
#[derive(Debug, Clone, PartialEq, Eq)]
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
        remaining_stream_s: u32,
    },
    Closed {
        close_reason: String,
        closed_at: Instant,
    },
}

/// ストリームタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamType {
    Bidirectional,
    Unidirectional,
}

/// ストリーム状態
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamState {
    Open,
    HalfClosed,
    Closed,
}

/// 暗号化レベル
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionLevel {
    Initial,
    Handshake,
    Application,
}

/// 接続統計
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

/// エンドポイント統計
#[derive(Debug, Clone)]
pub struct EndpointStatistics {
    pub active_connection_s: u32,
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
            active_connection_s: 0,
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

/// QUIC設定
#[derive(Debug, Clone)]
pub struct QuicEndpointConfig {
    pub max_connection_s: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_stream_s_per_connection: u32,
    pub initial_max_data: u64,
    pub initial_max_stream_data: u64,
}

impl Default for QuicEndpointConfig {
    fn default() -> Self {
        Self {
            max_connection_s: 1000,
            connection_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30),
            max_stream_s_per_connection: 256,
            initial_max_data: 1048576,
            initial_max_stream_data: 262144,
        }
    }
}

/// QUIC接続
#[derive(Clone)]
pub struct QuicConnection {
    pub _connection_id: Bytes,
    pub _peer_addr: SocketAddr,
    pub state: Arc<TokioRwLock<ConnectionState>>,
    pub stream_s: Arc<TokioRwLock<HashMap<u64, QuicStream>>>,
    pub _next_stream_id: u64,
    pub established_at: Option<Instant>,
    pub _last_activity: Instant,
    pub stat_s: Arc<TokioRwLock<ConnectionStats>>,
}

/// QUICストリーム
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
    socket: UdpSocket,
    bind_addr: SocketAddr,
    config: QuicEndpointConfig,
    connection_s: Arc<TokioRwLock<HashMap<Bytes, QuicConnection>>>,
    statistics: Arc<TokioRwLock<EndpointStatistics>>,
}

/// QUIC暗号化コンテキスト
#[allow(dead_code)]
pub struct QuicCryptoContext {
    initial_secret: [u8; 32],
    handshake_secret: [u8; 32],
    application_secret: [u8; 32],
    key_update_secret: [u8; 32],
    encryption_level: EncryptionLevel,
    key_phase: u32,
}

impl QuicConnection {
    /// 新しいQUIC接続を作成
    pub fn new(
        connection_id: Bytes,
        peer_addr: SocketAddr,
        is_server: bool,
        stat_s: ConnectionStats,
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
            stream_s: Arc::new(TokioRwLock::new(HashMap::new())),
            _next_stream_id: if is_server { 1 } else { 0 },
            established_at: None,
            _last_activity: Instant::now(),
            stat_s: Arc::new(TokioRwLock::new(stat_s)),
        })
    }

    /// Connection ID取得
    pub fn connection_id(&self) -> Bytes {
        self._connection_id.clone()
    }

    /// 接続が確立されているかチェック
    pub async fn is_established(&self) -> bool {
        matches!(*self.state.read().await, ConnectionState::Connected { .. })
    }

    /// 新しいストリーム作成
    pub async fn create_stream(&mut self, stream_type: StreamType) -> Result<u64, QuicError> {
        if !self.is_established().await {
            return Err(QuicError::Protocol(String::new()));
        }

        let stream_id = self._next_stream_id;
        self._next_stream_id += 1;

        let stream = QuicStream::new(stream_id, stream_type);
        self.stream_s.write().await.insert(stream_id, stream);
        self._last_activity = Instant::now();

        Ok(stream_id)
    }

    /// ストリーム書き込み
    pub async fn write_stream(&mut self, stream_id: u64, _data: Bytes) -> Result<(), QuicError> {
        let mut stream_s = self.stream_s.write().await;
        let stream = stream_s
            .get_mut(&stream_id)
            .ok_or(QuicError::StreamNotFound(String::from("Stream not found")))?;

        stream.write_data(_data).await?;
        self._last_activity = Instant::now();

        Ok(())
    }

    /// ストリーム読み込み
    pub async fn read_stream(&mut self, stream_id: u64) -> Result<Option<Bytes>, QuicError> {
        let mut stream_s = self.stream_s.write().await;
        let stream = stream_s
            .get_mut(&stream_id)
            .ok_or(QuicError::StreamNotFound(String::from("Stream not found")))?;

        let _data = stream.read_data().await?;
        self._last_activity = Instant::now();

        Ok(_data)
    }

    /// 接続を閉じる
    pub async fn close(&mut self) -> Result<(), QuicError> {
        let mut state = self.state.write().await;
        *state = ConnectionState::Closing {
            peer: self._peer_addr,
            close_reason: String::new(),
            started_at: Instant::now(),
            remaining_stream_s: self.stream_s.read().await.len() as u32,
        };

        Ok(())
    }

    /// 接続状態を Connected に変更
    pub async fn establish_connection(&self, _servername: Option<String>) -> Result<(), QuicError> {
        let mut state = self.state.write().await;

        if let ConnectionState::Connecting {
            peer, start_time, ..
        } = state.clone()
        {
            *state = ConnectionState::Connected {
                peer,
                established_at: start_time,
                stream_count: 0,
                congestion_window: 65536,
            };
        }

        Ok(())
    }
}

impl QuicStream {
    /// 新しいストリーム作成
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

    /// データ書き込み
    pub async fn write_data(&mut self, _data: Bytes) -> Result<(), QuicError> {
        if self.state == StreamState::Closed {
            return Err(QuicError::StreamClosed);
        }

        self.send_buffer.extend_from_slice(&_data);
        Ok(())
    }

    /// データ読み込み
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
    /// 新しい暗号化コンテキスト作成
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

    /// パケット暗号化
    pub async fn encrypt_packet(
        &self,
        packet: &[u8],
        _packetnumber: u64,
    ) -> Result<Bytes, QuicError> {
        Ok(Bytes::copy_from_slice(packet))
    }

    /// パケット復号化
    pub async fn decrypt_packet(
        &self,
        encrypted_packet: &[u8],
        _packetnumber: u64,
    ) -> Result<Bytes, QuicError> {
        Ok(Bytes::copy_from_slice(encrypted_packet))
    }
}

impl QuicEndpoint {
    /// 新しいQUIC Endpoint作成
    pub async fn new(bind_addr: SocketAddr, config: QuicEndpointConfig) -> Result<Self, QuicError> {
        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| QuicError::Io(e.to_string()))?;
        let connection_s = Arc::new(TokioRwLock::new(HashMap::new()));
        let statistics = Arc::new(TokioRwLock::new(EndpointStatistics::default()));

        Ok(Self {
            socket,
            bind_addr,
            config,
            connection_s,
            statistics,
        })
    }

    /// 接続統計取得
    pub async fn get_connection_stats(
        &self,
        connection_id: &Bytes,
    ) -> Result<ConnectionStats, QuicError> {
        let connection_s = self.connection_s.read().await;
        let connection = connection_s
            .get(connection_id)
            .ok_or_else(|| QuicError::ConnectionNotFound(connection_id.clone()))?;

        let stat_s = connection.stat_s.read().await.clone();

        Ok(stat_s)
    }

    /// アクティブな接続一覧取得
    pub async fn active_connection_s(&self) -> Vec<Bytes> {
        self.connection_s.read().await.keys().cloned().collect()
    }

    /// 接続取得
    pub async fn get_connection(&self, connection_id: &Bytes) -> Result<QuicConnection, QuicError> {
        let connection_s = self.connection_s.read().await;
        connection_s
            .get(connection_id)
            .cloned()
            .ok_or_else(|| QuicError::ConnectionNotFound(connection_id.clone()))
    }

    /// 接続削除
    pub async fn remove_connection(&self, connection_id: &Bytes) -> Result<(), QuicError> {
        let mut connection_s = self.connection_s.write().await;
        connection_s.remove(connection_id);

        Ok(())
    }

    /// アイドル接続をクリーンアップ
    pub async fn cleanup_idle_connection_s(&self) -> Result<(), QuicError> {
        let mut connection_s = self.connection_s.write().await;
        let timeout_duration = self.config.idle_timeout;
        let current_time = Instant::now();

        let idle_ids: Vec<Bytes> = connection_s
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
            connection_s.remove(&id);
        }

        Ok(())
    }

    /// データ送信
    pub async fn send_data(&self, connection_id: &Bytes) -> Result<(), QuicError> {
        let connection_s = self.connection_s.read().await;
        connection_s
            .get(connection_id)
            .ok_or_else(|| QuicError::ConnectionNotFound(connection_id.clone()))?;

        Ok(())
    }
}
