// Pure Rust implementation of protocol types to replace protobuf/gRPC dependencies
// This avoids C/C++ dependencies while maintaining compatibility with the daemon API

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Custom timestamp message to replace google.protobuf.Timestamp
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Timestamp {
    pub seconds: i64,
    pub nanos: i32,
}

impl Timestamp {
    pub fn now() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        Self {
            seconds: now.as_secs() as i64,
            nanos: now.subsec_nanos() as i32,
        }
    }
}

// Custom empty message to replace google.protobuf.Empty
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Empty {}

// Extended NodeInfo with comprehensive daemon status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeInfo {
    pub node_id: String,
    pub version: String,
    pub uptime_sec: u32,
    pub bytes_in: u64,
    pub bytes_out: u64,
    pub pid: u32,
    pub active_streams: u32,
    pub connected_peers: u32,
    pub mix_routes: Vec<String>,
    pub performance: Option<PerformanceMetrics>,
    pub resources: Option<ResourceUsage>,
    pub topology: Option<NetworkTopology>,
    /// Nyx Protocol compliance level (Core / Plus / Full)
    pub compliance_level: Option<String>,
    /// Numeric capability identifiers advertised by this node (spec Appendix)
    pub capabilities: Option<Vec<u32>>,
}

// Performance metrics for the daemon
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceMetrics {
    pub cover_traffic_rate: f64,    // packets/sec
    pub avg_latency_ms: f64,        // milliseconds
    pub packet_loss_rate: f64,      // 0.0-1.0
    pub bandwidth_utilization: f64, // 0.0-1.0
    pub cpu_usage: f64,             // 0.0-1.0
    pub memory_usage_mb: f64,       // megabytes
    pub total_packets_sent: u64,
    pub total_packets_received: u64,
    pub retransmissions: u64,
    pub connection_success_rate: f64, // 0.0-1.0
}

// System resource usage information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub memory_rss_bytes: u64, // Alias for memory_bytes
    pub memory_vms_bytes: u64, // Virtual memory
    pub memory_percent: f64,
    pub disk_usage_bytes: u64,
    pub disk_total_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub network_bytes_sent: u64,     // Added for compatibility
    pub network_bytes_received: u64, // Added for compatibility
    pub file_descriptors: u32,
    pub open_file_descriptors: u32, // Alias for file_descriptors
    pub thread_count: u32,
}

// Network topology information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkTopology {
    pub total_nodes: u32,
    pub active_nodes: u32,
    pub mix_nodes: u32,
    pub gateway_nodes: u32,
    pub network_diameter: u32,
    pub clustering_coefficient: f64,
    pub peers: Vec<String>,             // Added for compatibility
    pub paths: Vec<String>,             // Added for compatibility
    pub total_nodes_known: u32,         // Added for compatibility
    pub reachable_nodes: u32,           // Added for compatibility
    pub current_region: String,         // Added for compatibility
    pub available_regions: Vec<String>, // Added for compatibility
}

// Health check request and response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HealthRequest {
    pub include_details: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
    pub active_connections: u32,
    pub checks: Vec<HealthCheck>,
    pub checked_at: Option<Timestamp>,
}

// Stream management types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamId {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamOptions {
    pub reliability: bool,
    pub ordered: bool,
    pub max_retries: u32,
    pub timeout_ms: u64,
    pub buffer_size: u32,
    pub multipath: bool,
    pub max_paths: u32,
    pub path_strategy: String,
    pub auto_reconnect: bool,
    pub max_retry_attempts: u32,
    pub compression: bool,
    pub cipher_suite: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenRequest {
    pub destination: String,
    pub target_address: String, // Alias for destination
    pub options: Option<StreamOptions>,
    /// Optional metadata for authorization and custom headers
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamResponse {
    pub stream_id: String,
    pub status: String,
    pub target_address: String,
    pub initial_stats: Option<StreamStats>,
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamStats {
    pub stream_id: String,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub retransmissions: u64,
    pub rtt_ms: f64,
    pub bandwidth_bps: f64,
    pub bandwidth_mbps: f64,  // Converted from bandwidth_bps
    pub paths: Vec<PathStat>, // Path statistics
    pub target_address: String,
    pub state: String,
    pub created_at: Option<Timestamp>,
    pub last_activity: Option<Timestamp>,
    pub avg_rtt_ms: f64,
    pub min_rtt_ms: f64,
    pub max_rtt_ms: f64,
    pub packet_loss_rate: f64,
    pub connection_errors: u64,
    pub timeout_errors: u64,
    pub last_error: String,
    pub last_error_at: Option<Timestamp>,
    pub stream_info: Option<StreamInfo>,
    pub path_stats: Vec<StreamPathStats>,
    pub timestamp: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathStat {
    pub path_id: String,
    pub rtt_ms: f64,
    pub bandwidth_mbps: f64,
    pub status: String,
    pub packet_count: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamInfo {
    pub stream_id: String,
    pub target_address: String,
    pub state: String,
    pub status: String,
    pub destination: String,
    pub created_at: Option<Timestamp>,
    pub last_activity: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamPathStats {
    pub path_id: String,
    pub status: String,
    pub rtt_ms: f64,
    pub bandwidth_mbps: f64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packet_count: u64,
    pub success_rate: f64,
    pub latency_ms: f64,
    pub bandwidth_bps: f64,
    pub packet_loss_rate: f64,
    pub reliability_score: f64,
}

// Data transfer types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataRequest {
    pub stream_id: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataResponse {
    pub success: bool,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReceiveResponse {
    pub stream_id: String,
    pub data: Vec<u8>,
    pub more_data: bool,
}

// Path management types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathRequest {
    pub destination: String,
    pub target: String, // Alias for destination
    pub num_hops: u32,
    pub hops: u32, // Alias for num_hops
    pub preferences: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathResponse {
    pub path_id: String,
    pub path: Vec<String>, // Alias for hops
    pub hops: Vec<String>,
    pub latency_ms: f64,
    pub estimated_latency_ms: f64, // Alias for latency_ms
    pub bandwidth_estimate: f64,
    pub estimated_bandwidth_mbps: f64, // Alias for bandwidth_estimate
    pub reliability_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathInfo {
    pub path_id: String,
    pub hops: Vec<String>,
    pub latency_ms: f64,
    pub total_latency_ms: f64, // Alias for latency_ms
    pub bandwidth_bps: f64,
    pub min_bandwidth_mbps: f64, // Converted from bandwidth_bps
    pub reliability_score: f64,
    pub last_used: Option<Timestamp>,
    pub status: String,
    pub packet_count: u64,
    pub success_rate: f64,
    pub created_at: Option<Timestamp>,
}

// Peer information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerInfo {
    pub peer_id: String,
    pub node_id: String, // Alias for peer_id
    pub address: String,
    pub last_seen: Option<Timestamp>,
    pub connection_status: String,
    pub status: String, // Alias for connection_status
    pub latency_ms: f64,
    pub reliability_score: f64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub bandwidth_mbps: f64,
    pub connection_count: u32,
    pub region: String,
}

// Event system types - NOTE: See unified Event definition below
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
// pub struct Event {
//     pub timestamp: Option<Timestamp>,
//     pub event_type: String,
//     pub data: HashMap<String, String>,
//     pub event_data: Option<SystemEvent>,
//     pub r#type: String,
//     pub detail: String,
//     pub severity: String,
//     pub attributes: HashMap<String, String>,
// }

// NOTE: See unified SystemEvent definition below
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
// pub struct SystemEvent {
//     pub timestamp: Option<Timestamp>,
//     pub event_type: String,
//     pub severity: String,
//     pub message: String,
//     pub metadata: HashMap<String, String>,
//     pub component: String,
//     pub action: String,
//     pub details: HashMap<String, String>,
// }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamEvent {
    pub stream_id: String,
    pub event_type: String,
    pub timestamp: Option<Timestamp>,
    pub data: HashMap<String, String>,
    pub action: String,
    pub target_address: String,
    pub stats: Option<StreamStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceEvent {
    pub metric_name: String,
    pub value: f64,
    pub timestamp: Option<Timestamp>,
    pub tags: HashMap<String, String>,
}

// Configuration management types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigUpdate {
    pub section: String,
    pub key: String,
    pub value: String,
    pub settings: HashMap<String, String>,
}

// Additional response validation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

// Top-level configuration request (restored after duplicate removal)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigRequest {
    pub key: String,
    pub value: Option<String>,
    pub operation: String, // "get", "set", "delete", "list"
    pub scope: String,     // "global", "local", "temporary"
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigResponse {
    pub success: bool,
    pub status: String,
    pub message: String,
    pub current_value: Option<String>,
    pub details: HashMap<String, String>,
    pub validation_errors: Vec<ValidationError>,
}

// Statistics types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatsRequest {
    pub metric_names: Vec<String>,
    pub time_range_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatsUpdate {
    pub metrics: HashMap<String, f64>,
    pub timestamp: Option<Timestamp>,
    pub node_info: Option<NodeInfo>,    // Added for compatibility
    pub stream_stats: Vec<StreamStats>, // Added for compatibility
    pub custom_metrics: HashMap<String, f64>, // Added for compatibility
}

// Event system types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub timestamp: Option<Timestamp>,
    pub event_type: String,
    pub data: HashMap<String, String>,
    pub event_data: Option<event::EventData>,
    pub r#type: String,
    pub detail: String,
    pub severity: String,
    pub attributes: HashMap<String, String>,
}

// Note: The top-level SystemEvent is kept for backward compatibility with some modules
// but internal event_data should use event::SystemEvent via event::EventData.
// Call sites in daemon should construct crate::proto::event::SystemEvent, not this one.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemEvent {
    pub timestamp: Option<Timestamp>,
    pub event_type: String,
    pub severity: String,
    pub message: String,
    pub metadata: HashMap<String, String>,
    pub component: String,
    pub action: String,
    pub details: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventFilter {
    pub event_types: Vec<String>,
    pub types: Vec<String>, // Alias for event_types
    pub severity_levels: Vec<String>,
    pub severity: String, // Single severity filter
    pub time_range_seconds: Option<u64>,
    pub stream_ids: Vec<u32>, // Stream ID filters
}

// Configuration types
// NOTE: Use existing ConfigUpdate definition above
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
// pub struct ConfigUpdate {
//     pub settings: HashMap<String, String>,
// }

// Health check types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthCheck {
    pub name: String,
    pub status: String,
    pub message: String,
    pub checked_at: Option<Timestamp>,
    pub response_time_ms: f64, // Add missing field
}

// NOTE: Unified HealthResponse definition above
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
// pub struct HealthResponse {
//     pub status: String,
//     pub checks: Vec<HealthCheck>,
//     pub checked_at: Option<Timestamp>,
// }

// Stream information types

// Event system for streaming capabilities
pub mod event {
    use super::*;

    // Event types for daemon events
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub enum EventType {
        StreamOpened,
        StreamClosed,
        PeerConnected,
        PeerDisconnected,
        LayerStatusChanged,
        ConfigurationChanged,
        HealthStatusChanged,
        Error,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct Event {
        pub event_type: String,
        pub timestamp: Timestamp,
        pub data: HashMap<String, String>,
        pub severity: EventSeverity,
        pub event_data: Option<event::EventData>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub enum EventSeverity {
        Info,
        Warning,
        Error,
        Critical,
    }

    // EventData enum for different event types
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub enum EventData {
        StreamEvent(StreamEvent),
        LayerEvent(LayerEvent),
        ConfigEvent(ConfigEvent),
        HealthEvent(HealthEvent),
        PerformanceEvent(PerformanceEvent),
        NetworkEvent(NetworkEvent),
        SystemEvent(SystemEvent),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct StreamEvent {
        pub stream_id: String,
        pub event_type: String,
        pub action: String,
        pub target_address: String,
        pub stats: Option<super::StreamStats>,
        pub timestamp: Option<Timestamp>,
        pub data: HashMap<String, String>,
        pub details: String, // Backward compatible detail/summary
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct LayerEvent {
        pub layer_name: String,
        pub status: String,
        pub message: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct ConfigEvent {
        pub config_key: String,
        pub old_value: String,
        pub new_value: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct HealthEvent {
        pub component: String,
        pub status: String,
        pub message: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct PerformanceEvent {
        pub metric: String,
        pub value: f64,
        pub threshold: f64,
        pub description: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct NetworkEvent {
        pub peer_id: String,
        pub action: String,
        pub address: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    pub struct SystemEvent {
        pub event_type: String,
        pub message: String,
        pub severity: String,
        pub component: String,
        pub metadata: HashMap<String, String>,
    }

    // NOTE: Removed duplicate ConfigRequest and PerformanceMetrics definitions to avoid type conflicts.

    #[allow(dead_code)] // fluent builders primarily used in diagnostic-heavy builds
    impl Event {
        pub fn new(event_type: String, severity: EventSeverity) -> Self {
            Self {
                event_type,
                timestamp: Timestamp::now(),
                data: HashMap::new(),
                severity,
                event_data: None,
            }
        }

        pub fn with_data(mut self, key: &str, value: &str) -> Self {
            self.data.insert(key.to_string(), value.to_string());
            self
        }
    }
}

// Additional response validation - NOTE: Using ValidationError definition above
// #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
// pub struct ValidationError {
//     pub field: String,
//     pub message: String,
// }

// Update config response to include validation errors
impl ConfigResponse {
    pub fn validation_errors(&self) -> Vec<ValidationError> {
        // Return access to the validation_errors field directly
        self.validation_errors.clone()
    }
}

// Pure Rust trait for control service implementation
#[async_trait::async_trait]
pub trait NyxControl: Send + Sync + 'static {
    async fn get_info(&self, request: Empty) -> Result<NodeInfo, String>;
    async fn get_health(&self, request: HealthRequest) -> Result<HealthResponse, String>;
    async fn open_stream(&self, request: OpenRequest) -> Result<StreamResponse, String>;
    async fn close_stream(&self, request: StreamId) -> Result<Empty, String>;
    async fn get_stream_stats(&self, request: StreamId) -> Result<StreamStats, String>;

    // Additional methods for compatibility with main.rs implementation
    async fn list_streams(&self, request: Empty) -> Result<Vec<StreamStats>, String>;
    async fn subscribe_events(&self, request: EventFilter) -> Result<Vec<Event>, String>;
    async fn subscribe_stats(&self, request: Empty) -> Result<Vec<StatsUpdate>, String>;
    async fn update_config(&self, request: ConfigRequest) -> Result<ConfigResponse, String>;
    async fn reload_config(&self, request: Empty) -> Result<ConfigResponse, String>;
    async fn receive_data(&self, request: StreamId) -> Result<ReceiveResponse, String>;
    async fn send_data(&self, request: DataRequest) -> Result<DataResponse, String>;
}

/// gRPC エラーコード変換ポリシー
/// 現行: 内部 Close コード (例: 0x07 UNSUPPORTED_CAP) を gRPC 相当の抽象分類へマップする。
///  - 0x07 (UNSUPPORTED_CAP) => FailedPrecondition (client が capability 拡張で再試行可能)
///  - 0x10–0x1F (認証/権限系: 予約想定) => PermissionDenied / Unauthenticated
///  - 0x20–0x2F (一時的資源枯渇) => ResourceExhausted
///  - その他未分類 => Unknown
#[allow(dead_code)]
pub fn map_close_code_to_grpc(code: u16) -> &'static str {
    match code {
        0x07 => "FailedPrecondition",
        0x10..=0x1F => "PermissionDenied",
        0x20..=0x2F => "ResourceExhausted",
        _ => "Unknown",
    }
}

// Server types for gRPC replacement
#[derive(Debug, Clone)]
#[allow(dead_code)] // server wrapper kept for potential gRPC/IPC backends
pub struct NyxControlServer<T>
where
    T: NyxControl,
{
    // Pure Rust server implementation
    pub service: T,
    pub address: String,
}

#[allow(dead_code)] // constructor used in alternate deployment modes
impl<T> NyxControlServer<T>
where
    T: NyxControl,
{
    pub fn new(service: T) -> Self {
        Self {
            service,
            address: "0.0.0.0:50051".to_string(),
        }
    }
}

#[cfg(feature = "experimental-p2p")]
pub mod nyx_control_server {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    /// Minimal pure-Rust control service implementation
    pub struct NyxControlService<S: NyxControl> {
        inner: Arc<S>,
        _addr: String,
    }

    impl<S: NyxControl> NyxControlService<S> {
        pub fn new(inner: S, addr: impl Into<String>) -> Self {
            Self {
                inner: Arc::new(inner),
                _addr: addr.into(),
            }
        }

        pub fn address(&self) -> &str {
            &self._addr
        }
    }
}
