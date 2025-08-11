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
            .unwrap();
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
    pub compliance_level: Option<String>,
    pub capabilities: Option<Vec<u32>>,
}

// Performance metrics for the daemon
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceMetrics {
    pub cover_traffic_rate: f64,        // packets/sec
    pub avg_latency_ms: f64,            // milliseconds
    pub packet_loss_rate: f64,          // 0.0-1.0
    pub bandwidth_utilization: f64,     // 0.0-1.0
    pub cpu_usage: f64,                 // 0.0-1.0
    pub memory_usage_mb: f64,           // megabytes
    pub total_packets_sent: u64,
    pub total_packets_received: u64,
    pub retransmissions: u64,
}

// System resource usage information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_bytes: u64,
    pub memory_percent: f64,
    pub disk_usage_bytes: u64,
    pub disk_total_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub file_descriptors: u32,
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
}

// Health check request and response
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct HealthRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
    pub active_connections: u32,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenRequest {
    pub destination: String,
    pub options: Option<StreamOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamResponse {
    pub stream_id: String,
    pub status: String,
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
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReceiveResponse {
    pub stream_id: String,
    pub data: Vec<u8>,
    pub more_data: bool,
    pub success: bool,
    pub error: String,
}

// Path management types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathRequest {
    pub destination: String,
    pub num_hops: u32,
    pub preferences: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathResponse {
    pub path_id: String,
    pub hops: Vec<String>,
    pub latency_ms: f64,
    pub bandwidth_estimate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PathInfo {
    pub path_id: String,
    pub hops: Vec<String>,
    pub latency_ms: f64,
    pub bandwidth_bps: f64,
    pub reliability_score: f64,
    pub last_used: Option<Timestamp>,
}

// Peer information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerInfo {
    pub peer_id: String,
    pub address: String,
    pub last_seen: Option<Timestamp>,
    pub connection_status: String,
    pub latency_ms: f64,
    pub reliability_score: f64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

// Event system types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub timestamp: Option<Timestamp>,
    pub event_type: String,
    pub data: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemEvent {
    pub timestamp: Option<Timestamp>,
    pub event_type: String,
    pub severity: String,
    pub message: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamEvent {
    pub stream_id: String,
    pub event_type: String,
    pub timestamp: Option<Timestamp>,
    pub data: HashMap<String, String>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigResponse {
    pub success: bool,
    pub message: String,
    pub current_value: Option<String>,
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
}

// Pure Rust gRPC client types to replace tonic
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub struct Status {
    pub code: StatusCode,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum StatusCode {
    Ok,
    Cancelled,
    Unknown,
    InvalidArgument,
    DeadlineExceeded,
    NotFound,
    AlreadyExists,
    PermissionDenied,
    ResourceExhausted,
    FailedPrecondition,
    Aborted,
    OutOfRange,
    Unimplemented,
    Internal,
    Unavailable,
    DataLoss,
    Unauthenticated,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Status: {:?} - {}", self.code, self.message)
    }
}

impl StdError for Status {}

pub type Result<T> = std::result::Result<T, Status>;

// Streaming response type to replace tonic::Streaming
pub struct Streaming<T> {
    pub inner: tokio::sync::mpsc::UnboundedReceiver<std::result::Result<T, Status>>,
}

impl<T> Streaming<T> {
    pub async fn message(&mut self) -> Option<std::result::Result<T, Status>> {
        self.inner.recv().await
    }
}

// Request type to replace tonic::Request
pub struct Request<T> {
    pub inner: T,
}

impl<T> Request<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
    
    pub fn into_inner(self) -> T {
        self.inner
    }
}

// Response type to replace tonic::Response
pub struct Response<T> {
    pub inner: T,
}

impl<T> Response<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
    
    pub fn into_inner(self) -> T {
        self.inner
    }
}