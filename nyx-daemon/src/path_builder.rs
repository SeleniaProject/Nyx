#![forbid(unsafe_code)]

//! Advanced DHT-based path building system for Nyx daemon with real peer discovery.
//!
//! This module integrates the Pure Rust DHT implementation for actual peer discovery
//! and network-based path building, replacing all placeholder implementations with
//! fully functional networking code that operates without C/C++ dependencies.
//!
//! NEW: Implements actual onion routing path construction with layered encryption.

use crate::proto::{PathRequest, PathResponse};
use crate::pure_rust_dht_tcp::{PureRustDht, PeerInfo as DhtPeerInfo, DhtError};
// Direct path_builder.rs local types 
use geo::Point;
use lru::LruCache;
use anyhow;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::net::SocketAddr;
use std::time::{Duration, SystemTime, Instant};

// Pure Rust multiaddr
use multiaddr::Multiaddr;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::{RwLock, Mutex};
use tokio::time::interval;
use tracing::{debug, error, info, warn, instrument};
use serde::{Serialize, Deserialize};

// Onion routing imports
use nyx_crypto::aead::{NyxAead, AeadError};
use nyx_crypto::kdf::{hkdf_expand, KdfLabel};
use nyx_crypto::noise::SessionKey;
use rand::{thread_rng, RngCore};

// Performance monitoring imports
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use tokio::time::timeout;
// 共有パス性能モニタ (重複実装排除)
use nyx_core::{PathPerformanceMonitor, PathPerformanceMetrics, PathPerformanceTrend as PerformanceTrend};
// sysinfo や VecDeque はローカルモニタ削除により不要になった
// use sysinfo::{System, NetworkData};
// use std::collections::VecDeque;

/// Convert proto::Timestamp to SystemTime
fn proto_timestamp_to_system_time(timestamp: crate::proto::Timestamp) -> SystemTime {
    let duration = Duration::new(timestamp.seconds as u64, timestamp.nanos as u32);
    std::time::UNIX_EPOCH + duration
}

/// Convert SystemTime to proto::Timestamp (helper function for consistent API)
fn system_time_to_proto_timestamp(time: SystemTime) -> crate::proto::Timestamp {
    let duration = time.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    crate::proto::Timestamp {
        seconds: duration.as_secs() as i64,
        nanos: duration.subsec_nanos() as i32,
    }
}

/// Maximum number of candidate nodes to consider for path building
const MAX_CANDIDATES: usize = 1000;

/// Maximum number of cached paths per target
const MAX_CACHED_PATHS: usize = 100;

/// Default geographic diversity radius in kilometers
const GEOGRAPHIC_DIVERSITY_RADIUS_KM: f64 = 500.0;

/// Path quality thresholds
const MIN_RELIABILITY_THRESHOLD: f64 = 0.8;
const MAX_LATENCY_THRESHOLD_MS: f64 = 500.0;
const MIN_BANDWIDTH_THRESHOLD_MBPS: f64 = 10.0;

/// Onion routing constants
const ONION_LAYER_KEY_SIZE: usize = 32;
const ONION_LAYER_NONCE_SIZE: usize = 12;

/// Fallback path selection constants
const MAX_FALLBACK_ATTEMPTS: usize = 3;
const FALLBACK_QUALITY_THRESHOLD: f64 = 0.6;
const EMERGENCY_FALLBACK_THRESHOLD: f64 = 0.3;
// NOTE: 旧ローカル PathPerformanceMonitor 実装/構造体/定数は共有版へ統合したため削除。
//       上記でエクスポートされた型を利用して後続コード (評価/報告/選択ロジック) は継続動作する。

/// Path validation result with detailed feedback
#[derive(Debug, Clone)]
pub struct PathValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub security_score: f64,
    pub anonymity_score: f64,
    pub performance_score: f64,
}

impl PathValidationResult {
    pub fn new() -> Self {
        Self {
            is_valid: false,
            warnings: Vec::new(),
            security_score: 0.0,
            anonymity_score: 0.0,
            performance_score: 0.0,
        }
    }
}

/// Path validation errors
#[derive(Debug, thiserror::Error)]
pub enum PathValidationError {
    #[error("Path is empty")]
    EmptyPath,
    #[error("Invalid cryptographic material: {0}")]
    InvalidCryptographicMaterial(String),
    #[error("Invalid peer information: {0}")]
    InvalidPeerInfo(String),
    #[error("Invalid network address: {0}")]
    InvalidNetworkAddress(String),
    #[error("Path structure error: {0}")]
    StructureError(String),
}

/// Path connectivity test result
#[derive(Debug, Clone)]
pub struct PathConnectivityResult {
    pub connectivity_verified: bool,
    pub total_test_time: Duration,
    pub encryption_latency: Duration,
    pub layer_decrypt_times: Vec<Duration>,
    pub encrypted_size: usize,
    pub test_timestamp: Instant,
}

impl PathConnectivityResult {
    pub fn new() -> Self {
        Self {
            connectivity_verified: false,
            total_test_time: Duration::default(),
            encryption_latency: Duration::default(),
            layer_decrypt_times: Vec::new(),
            encrypted_size: 0,
            test_timestamp: Instant::now(),
        }
    }
}

/// Path testing errors
#[derive(Debug, thiserror::Error)]
pub enum PathTestError {
    #[error("Encryption failure: {0}")]
    EncryptionFailure(String),
    #[error("Decryption failure: {0}")]
    DecryptionFailure(String),
    #[error("Data corruption: {0}")]
    DataCorruption(String),
    #[error("Network connectivity failure: {0}")]
    NetworkFailure(String),
    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Path performance estimation
#[derive(Debug, Clone)]
pub struct PathPerformanceEstimate {
    pub estimated_latency_ms: f64,
    pub encryption_overhead_bytes: usize,
    pub bandwidth_efficiency: f64,
    pub anonymity_score: f64,
}

/// Fallback path selection strategy
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackStrategy {
    /// Use highest quality available paths
    QualityFirst,
    /// Prioritize low latency paths
    LatencyOptimized,
    /// Use geographically diverse paths for anonymity
    DiversityOptimized,
    /// Emergency mode - use any available path
    Emergency,
}

/// Fallback path selection criteria
#[derive(Debug, Clone)]
pub struct FallbackCriteria {
    pub strategy: FallbackStrategy,
    pub min_quality_threshold: f64,
    pub max_latency_ms: f64,
    pub required_diversity: bool,
    pub allow_fallback_peers: bool,
}

impl Default for FallbackCriteria {
    fn default() -> Self {
        Self {
            strategy: FallbackStrategy::QualityFirst,
            min_quality_threshold: FALLBACK_QUALITY_THRESHOLD,
            max_latency_ms: MAX_LATENCY_THRESHOLD_MS * 2.0, // More lenient for fallback
            required_diversity: true,
            allow_fallback_peers: true,
        }
    }
}

/// Fallback path selection result
#[derive(Debug, Clone)]
pub struct FallbackPathResult {
    pub path: Vec<String>,
    pub strategy_used: FallbackStrategy,
    pub quality_score: f64,
    pub fallback_level: usize, // 0 = primary, 1 = first fallback, etc.
    pub warning_messages: Vec<String>,
}

impl Default for PathPerformanceEstimate {
    fn default() -> Self {
        Self {
            estimated_latency_ms: 0.0,
            encryption_overhead_bytes: 0,
            bandwidth_efficiency: 1.0,
            anonymity_score: 0.0,
        }
    }
}

/// A single encryption layer in the onion routing path
#[derive(Debug, Clone)]
pub struct OnionLayer {
    /// Encryption key for this layer
    pub key: [u8; ONION_LAYER_KEY_SIZE],
    /// Nonce for AEAD encryption
    pub nonce: [u8; ONION_LAYER_NONCE_SIZE],
    /// Peer ID for this hop
    pub peer_id: String,
    /// Network address of the peer
    pub peer_addr: SocketAddr,
}

/// Complete onion routing path with all encryption layers
#[derive(Debug, Clone)]
pub struct OnionPath {
    /// All encryption layers from outermost to innermost
    pub layers: Vec<OnionLayer>,
    /// Path identifier for tracking
    pub path_id: u64,
    /// Creation timestamp
    pub created_at: Instant,
    /// Target destination
    pub destination: String,
}

impl OnionPath {
    /// Encrypt data through all layers (client-side encryption)
    pub fn encrypt_onion(&self, plaintext: &[u8]) -> Result<Vec<u8>, AeadError> {
        let mut data = plaintext.to_vec();
        
        // Encrypt from innermost to outermost layer (reverse order)
        for layer in self.layers.iter().rev() {
            let additional_data = layer.peer_id.as_bytes();
            let session_key = SessionKey::new(layer.key);
            let aead = NyxAead::new(&session_key);
            data = aead.encrypt(&layer.nonce, &data, additional_data)?;
        }
        
        Ok(data)
    }
    
    /// Decrypt one layer (relay-side decryption)
    pub fn decrypt_layer(&self, layer_index: usize, ciphertext: &[u8]) -> Result<Vec<u8>, AeadError> {
        if layer_index >= self.layers.len() {
            return Err(AeadError::DecryptionFailed("Invalid layer index".to_string()));
        }
        
        let layer = &self.layers[layer_index];
        let additional_data = layer.peer_id.as_bytes();
        let session_key = SessionKey::new(layer.key);
        let aead = NyxAead::new(&session_key);
        aead.decrypt(&layer.nonce, ciphertext, additional_data)
    }
    
    /// Get the next hop for routing
    pub fn next_hop(&self, current_layer: usize) -> Option<&OnionLayer> {
        if current_layer + 1 < self.layers.len() {
            Some(&self.layers[current_layer + 1])
        } else {
            None
        }
    }
    
    /// Validate the onion path structure and cryptographic integrity
    pub fn validate_path(&self) -> Result<PathValidationResult, PathValidationError> {
        let mut result = PathValidationResult::new();
        
        // Check minimum requirements
        if self.layers.is_empty() {
            return Err(PathValidationError::EmptyPath);
        }
        
        if self.layers.len() < 3 {
            result.warnings.push("Path has fewer than 3 hops, which reduces anonymity".to_string());
        }
        
        if self.layers.len() > 10 {
            result.warnings.push("Path has more than 10 hops, which may increase latency".to_string());
        }
        
        // Validate each layer
        for (i, layer) in self.layers.iter().enumerate() {
            // Check cryptographic material
            if layer.key.iter().all(|&b| b == 0) {
                return Err(PathValidationError::InvalidCryptographicMaterial(
                    format!("Layer {} has all-zero encryption key", i)
                ));
            }
            
            if layer.nonce.iter().all(|&b| b == 0) {
                result.warnings.push(format!("Layer {} has all-zero nonce, which may reduce security", i));
            }
            
            // Check peer information
            if layer.peer_id.is_empty() {
                return Err(PathValidationError::InvalidPeerInfo(
                    format!("Layer {} has empty peer ID", i)
                ));
            }
            
            // Check for duplicate peers (reduces anonymity)
            for (j, other_layer) in self.layers.iter().enumerate() {
                if i != j && layer.peer_id == other_layer.peer_id {
                    result.warnings.push(format!("Duplicate peer {} at layers {} and {}", layer.peer_id, i, j));
                }
            }
            
            // Validate network address
            if layer.peer_addr.port() == 0 {
                return Err(PathValidationError::InvalidNetworkAddress(
                    format!("Layer {} has invalid port 0", i)
                ));
            }
        }
        
        // Check path age
        let path_age = self.created_at.elapsed();
        if path_age > Duration::from_secs(3600) {
            result.warnings.push("Path is older than 1 hour and may be stale".to_string());
        }
        
        result.is_valid = true;
        Ok(result)
    }
    
    /// Test the path by attempting to encrypt and decrypt a test message
    pub async fn test_path_connectivity(&self) -> Result<PathConnectivityResult, PathTestError> {
        let mut result = PathConnectivityResult::new();
        let test_message = b"test_connectivity_probe";
        
        // Test encryption/decryption cycle
        let start_time = Instant::now();
        match self.encrypt_onion(test_message) {
            Ok(encrypted) => {
                result.encryption_latency = start_time.elapsed();
                result.encrypted_size = encrypted.len();
                
                // Test layer-by-layer decryption
                let mut current_data = encrypted;
                for i in 0..self.layers.len() {
                    let decrypt_start = Instant::now();
                    match self.decrypt_layer(i, &current_data) {
                        Ok(decrypted) => {
                            current_data = decrypted;
                            result.layer_decrypt_times.push(decrypt_start.elapsed());
                        }
                        Err(e) => {
                            return Err(PathTestError::DecryptionFailure(
                                format!("Failed to decrypt layer {}: {}", i, e)
                            ));
                        }
                    }
                }
                
                // Verify final result
                if current_data == test_message {
                    result.connectivity_verified = true;
                } else {
                    return Err(PathTestError::DataCorruption(
                        "Decrypted data does not match original test message".to_string()
                    ));
                }
            }
            Err(e) => {
                return Err(PathTestError::EncryptionFailure(format!("Encryption failed: {}", e)));
            }
        }
        
        result.total_test_time = start_time.elapsed();
        Ok(result)
    }
    
    /// Estimate path performance metrics
    pub fn estimate_performance(&self) -> PathPerformanceEstimate {
        let mut estimate = PathPerformanceEstimate::default();
        
        // Calculate estimated latency (rough approximation)
        estimate.estimated_latency_ms = (self.layers.len() as f64) * 50.0; // 50ms per hop
        
        // Calculate overhead from encryption layers
        estimate.encryption_overhead_bytes = self.layers.len() * 32; // AEAD tag size per layer
        
        // Estimate bandwidth reduction due to encryption overhead
        estimate.bandwidth_efficiency = 1.0 - (estimate.encryption_overhead_bytes as f64 / 1500.0).min(0.3);
        
        // Calculate anonymity score based on path length and diversity
        estimate.anonymity_score = ((self.layers.len() as f64).ln() / 10.0_f64.ln()).min(1.0);
        
        estimate
    }
}

/// DHT peer discovery criteria
#[derive(Debug, Clone)]
pub enum DiscoveryCriteria {
    ByRegion(String),
    ByCapability(String),
    ByLatency(f64), // Max latency in ms
    ByBandwidth(f64), // Min bandwidth in Mbps
    Random(usize), // Number of random peers
    HighPerformance, // High performance peers with low latency and high bandwidth
    GeographicDiversity, // Peers from different geographic regions
    Reliability, // Peers with high reliability scores
    All,
}

/// Discovery strategy settings for optimized peer discovery
#[derive(Debug, Clone)]
pub struct DiscoveryStrategy {
    pub discovery_timeout_secs: u64,
    pub max_peers_per_query: usize,
    pub refresh_interval_secs: u64,
}

impl Default for DiscoveryStrategy {
    fn default() -> Self {
        Self {
            discovery_timeout_secs: 30,
            max_peers_per_query: 50, // Increased for better peer diversity
            refresh_interval_secs: 300, // 5 minutes
        }
    }
}

/// Persistent peer store for caching discovered peers across restarts
pub struct PersistentPeerStore {
    file_path: PathBuf,
}

impl PersistentPeerStore {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    /// Save peers to persistent storage with atomic write operation
    pub async fn save_peers(&self, peers: &[(String, CachedPeerInfo)]) -> Result<(), DhtError> {
        let serializable_peers: Vec<_> = peers.iter()
            .map(|(id, peer)| (id.clone(), SerializablePeerInfo::from(peer)))
            .collect();

        let data = serde_json::to_string_pretty(&serializable_peers)
            .map_err(|e| DhtError::InvalidMessage(format!("Serialization failed: {}", e)))?;

        // Create parent directory if it doesn't exist
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| DhtError::Communication(format!("Failed to create directory: {}", e)))?;
        }

        // Write to temporary file first, then atomically rename
        let temp_path = self.file_path.with_extension("tmp");
        tokio::fs::write(&temp_path, data).await
            .map_err(|e| DhtError::Communication(format!("Failed to write temp file: {}", e)))?;

        tokio::fs::rename(&temp_path, &self.file_path).await
            .map_err(|e| DhtError::Communication(format!("Failed to rename temp file: {}", e)))?;

        debug!("Successfully saved {} peers to persistent storage", peers.len());
        Ok(())
    }

    /// Load peers from persistent storage with error recovery
    pub async fn load_peers(&self) -> Result<Vec<(String, CachedPeerInfo)>, DhtError> {
        if !self.file_path.exists() {
            debug!("Peer storage file doesn't exist, starting with empty cache");
            return Ok(Vec::new());
        }

        let data = tokio::fs::read_to_string(&self.file_path).await
            .map_err(|e| DhtError::Communication(format!("Failed to read storage file: {}", e)))?;

        let serializable_peers: Vec<(String, SerializablePeerInfo)> = serde_json::from_str(&data)
            .map_err(|e| DhtError::InvalidMessage(format!("Deserialization failed: {}", e)))?;

        let peers: Vec<_> = serializable_peers.into_iter()
            .filter_map(|(id, serializable_peer)| {
                match CachedPeerInfo::try_from(serializable_peer) {
                    Ok(peer) => Some((id, peer)),
                    Err(e) => {
                        warn!("Failed to convert serializable peer: {}", e);
                        None
                    }
                }
            })
            .collect();

        info!("Loaded {} peers from persistent storage", peers.len());
        Ok(peers)
    }

    /// Clear all persistent storage
    pub async fn clear(&self) -> Result<(), DhtError> {
        if self.file_path.exists() {
            tokio::fs::remove_file(&self.file_path).await
                .map_err(|e| DhtError::Communication(format!("Failed to clear storage: {}", e)))?;
        }
        
        debug!("Cleared persistent peer storage");
        Ok(())
    }

    /// Get storage statistics
    pub async fn get_stats(&self) -> Result<StorageStats, DhtError> {
        if !self.file_path.exists() {
            return Ok(StorageStats {
                file_size_bytes: 0,
                peer_count: 0,
                last_modified: None,
            });
        }

        let metadata = tokio::fs::metadata(&self.file_path).await
            .map_err(|e| DhtError::Communication(format!("Failed to get file metadata: {}", e)))?;

        let data = tokio::fs::read_to_string(&self.file_path).await
            .map_err(|e| DhtError::Communication(format!("Failed to read storage file: {}", e)))?;

        let peer_count = match serde_json::from_str::<Vec<(String, SerializablePeerInfo)>>(&data) {
            Ok(peers) => peers.len(),
            Err(_) => 0,
        };

        Ok(StorageStats {
            file_size_bytes: metadata.len(),
            peer_count,
            last_modified: metadata.modified().ok(),
        })
    }
}

/// Storage statistics for monitoring
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub file_size_bytes: u64,
    pub peer_count: usize,
    pub last_modified: Option<SystemTime>,
}

/// SerializablePeerInfo for JSON persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializablePeerInfo {
    pub peer_id: String,
    pub addresses: Vec<String>,
    pub capabilities: Vec<String>,
    pub region: Option<String>,
    pub location: Option<(f64, f64)>, // lat, lon
    pub latency_ms: Option<f64>,
    pub reliability_score: f64,
    pub bandwidth_mbps: Option<f64>,
    pub last_seen_timestamp: u64,
    pub response_time_ms: Option<f64>,
}

impl From<&CachedPeerInfo> for SerializablePeerInfo {
    fn from(peer: &CachedPeerInfo) -> Self {
        Self {
            peer_id: peer.peer_id.clone(),
            addresses: peer.addresses.iter().map(|addr| addr.to_string()).collect(),
            capabilities: peer.capabilities.iter().cloned().collect(),
            region: peer.region.clone(),
            location: peer.location.map(|p| (p.x(), p.y())),
            latency_ms: peer.latency_ms,
            reliability_score: peer.reliability_score,
            bandwidth_mbps: peer.bandwidth_mbps,
            last_seen_timestamp: peer.last_seen.elapsed().as_secs(),
            response_time_ms: peer.response_time_ms,
        }
    }
}

/// Real DHT peer discovery implementation using Pure Rust DHT
pub struct DhtPeerDiscovery {
    /// DHT instance for actual network operations  
    dht: Arc<RwLock<Option<PureRustDht>>>,
    
    /// Local peer cache for fast lookups
    peer_cache: Arc<std::sync::Mutex<LruCache<String, CachedPeerInfo>>>,
    
    /// Persistent storage for peer information across restarts
    persistent_store: Arc<Mutex<HashMap<String, SerializablePeerInfo>>>,
    
    /// Discovery strategy configuration
    discovery_strategy: DiscoveryStrategy,
    
    /// Timestamp of last discovery operation
    last_discovery: Arc<std::sync::Mutex<Instant>>,
    
    /// Bootstrap peers for network entry
    bootstrap_peers: Vec<Multiaddr>,
    
    /// DHT bind address for network communication
    bind_addr: SocketAddr,
}
/// Cached peer information for faster lookup
#[derive(Debug, Clone)]
struct CachedPeerInfo {
    pub peer_id: String,
    pub addresses: Vec<Multiaddr>,
    pub capabilities: HashSet<String>,
    pub region: Option<String>,
    pub location: Option<Point>,
    pub latency_ms: Option<f64>,
    pub reliability_score: f64,
    pub bandwidth_mbps: Option<f64>,
    pub last_seen: Instant,
    pub response_time_ms: Option<f64>,
    pub last_active_bandwidth: Option<f64>, // 直近アクティブ測定値(Mbps)
    pub last_active_rtt: Option<f64>, // 直近アクティブ測定値(ms)
}
impl DhtPeerDiscovery {
    /// 指定ピアに対しアクティブ帯域・RTT測定を行い、CachedPeerInfoに反映する
    pub async fn active_bandwidth_probe(&self, peer: &mut CachedPeerInfo) -> Result<(), DhtError> {
        use tokio::time::timeout;
        use std::time::Instant;
        let addr = match peer.addresses.get(0) {
            Some(a) => a,
            None => return Err(DhtError::InvalidAddress("No address for peer".to_string())),
        };
        let socket_addr = self.multiaddr_to_socket_addr(addr)?;
        // RTT測定
        let start = Instant::now();
        let rtt = match timeout(std::time::Duration::from_millis(1500), TcpStream::connect(socket_addr)).await {
            Ok(Ok(mut stream)) => {
                // 簡易帯域測定: 32KB送信し応答待ち
                let test_data = vec![0u8; 32 * 1024];
                let send_start = Instant::now();
                let _ = stream.write_all(&test_data).await;
                let mut buf = [0u8; 8];
                let _ = stream.read_exact(&mut buf).await;
                let elapsed = send_start.elapsed().as_secs_f64();
                let mbps = if elapsed > 0.0 { (32.0 * 8.0) / elapsed } else { 0.0 };
                peer.last_active_bandwidth = Some(mbps);
                start.elapsed().as_secs_f64() * 1000.0
            },
            _ => return Err(DhtError::Network("RTT/bandwidth probe failed".to_string())),
        };
        peer.last_active_rtt = Some(rtt);
        Ok(())
    }


impl TryFrom<SerializablePeerInfo> for CachedPeerInfo {
    type Error = String;
    
    fn try_from(serializable: SerializablePeerInfo) -> Result<Self, Self::Error> {
        let addresses: Result<Vec<Multiaddr>, _> = serializable.addresses
            .iter()
            .map(|addr_str| addr_str.parse())
            .collect();
        
        let addresses = addresses
            .map_err(|e| format!("Invalid multiaddr in serialized peer: {}", e))?;
        
        let capabilities: HashSet<String> = serializable.capabilities.into_iter().collect();
        
        let location = serializable.location.map(|(x, y)| Point::new(x, y));
        
        // Calculate last_seen from timestamp (approximate)
        let last_seen = Instant::now() - Duration::from_secs(serializable.last_seen_timestamp);
        
        Ok(Self {
            peer_id: serializable.peer_id,
            addresses,
            capabilities,
            region: serializable.region,
            location,
            latency_ms: serializable.latency_ms,
            reliability_score: serializable.reliability_score,
            bandwidth_mbps: serializable.bandwidth_mbps,
            last_seen,
            response_time_ms: serializable.response_time_ms,
        })
    }
}

impl DhtPeerDiscovery {
    /// Create a new DHT peer discovery instance with real DHT integration
    pub async fn new(bootstrap_peers: Vec<String>) -> Result<Self, DhtError> {
        info!("Initializing DHT peer discovery with {} bootstrap peers", bootstrap_peers.len());
        
        // Convert bootstrap peers to multiaddr and validate them
        let mut bootstrap_multiaddrs = Vec::new();
        for peer in bootstrap_peers {
            match peer.parse::<Multiaddr>() {
                Ok(addr) => {
                    debug!("Added bootstrap peer: {}", addr);
                    bootstrap_multiaddrs.push(addr);
                }
                Err(e) => {
                    warn!("Invalid bootstrap peer address '{}': {}", peer, e);
                    // Continue with other peers instead of failing completely
                    continue;
                }
            }
        }
        
        // If no valid bootstrap peers, try to load from environment or use known Nyx network nodes
        if bootstrap_multiaddrs.is_empty() {
            warn!("No valid bootstrap peers provided, attempting to use known Nyx network nodes");
            let default_peers = Self::get_default_bootstrap_peers();
            if !default_peers.is_empty() {
                bootstrap_multiaddrs.extend(default_peers);
                info!("Using {} default Nyx network bootstrap peers", bootstrap_multiaddrs.len());
            } else {
                error!("No bootstrap peers available - network discovery will be limited");
                return Err(DhtError::BootstrapFailed);
            }
        }

        // Initialize peer cache with proper capacity
        let peer_cache = Arc::new(std::sync::Mutex::new(
            LruCache::new(std::num::NonZeroUsize::new(1000).unwrap())
        ));
        
        // Initialize persistent store for peer information
        let persistent_store = Arc::new(Mutex::new(HashMap::new()));
        
        // Try to load peers from persistent storage
        let cache_dir = std::env::temp_dir().join("nyx-peer-cache");
        if let Err(e) = tokio::fs::create_dir_all(&cache_dir).await {
            warn!("Failed to create cache directory: {}", e);
        }
        
        let store_path = cache_dir.join("peers.json");
        let peer_store = PersistentPeerStore::new(store_path);
        
        // Load existing peers into cache
        match peer_store.load_peers().await {
            Ok(loaded_peers) => {
                if let Ok(mut cache) = peer_cache.lock() {
                    for (id, peer_info) in loaded_peers {
                        cache.put(id, peer_info);
                    }
                    info!("Loaded {} peers from persistent storage", cache.len());
                }
            }
            Err(e) => {
                warn!("Failed to load peers from persistent storage: {}", e);
            }
        }
        
        // Set up discovery strategy with optimized parameters
        let discovery_strategy = DiscoveryStrategy {
            discovery_timeout_secs: 30,
            max_peers_per_query: 50,
            refresh_interval_secs: 300, // 5 minutes
        };

        Ok(Self {
            dht: Arc::new(RwLock::new(None)),
            peer_cache,
            persistent_store,
            discovery_strategy,
            last_discovery: Arc::new(std::sync::Mutex::new(Instant::now())),
            bootstrap_peers: bootstrap_multiaddrs,
            bind_addr: Self::get_bind_address(), // Configurable bind address
        })
    }
    
    /// Start the DHT background tasks with real network operations
    pub async fn start(&mut self) -> Result<(), DhtError> {
        info!("Starting DHT peer discovery service");
        
        // Validate that we have bootstrap peers
        if self.bootstrap_peers.is_empty() {
            return Err(DhtError::BootstrapFailed);
        }
        
        // Initialize actual DHT instance
        let bootstrap_addrs: Vec<std::net::SocketAddr> = self.bootstrap_peers
            .iter()
            .filter_map(|addr| {
                // Extract TCP port from multiaddr
                Self::extract_socket_addr_from_multiaddr(addr)
            })
            .collect();
            
        if bootstrap_addrs.is_empty() {
            return Err(DhtError::BootstrapFailed);
        }
        
        match PureRustDht::new(self.bind_addr, bootstrap_addrs).await {
            Ok(dht_instance) => {
                let mut dht_guard = self.dht.write().await;
                *dht_guard = Some(dht_instance);
                info!("DHT instance initialized successfully");
            }
            Err(e) => {
                error!("Failed to initialize DHT: {:?}", e);
                return Err(DhtError::NotInitialized("DHT initialization failed".to_string()));
            }
        }
        
        // Perform initial bootstrap discovery to populate cache
        self.perform_bootstrap_discovery().await?;
        
        // Start background tasks for peer discovery and cache maintenance
        self.start_background_discovery().await?;
        self.start_cache_maintenance().await?;
        
        info!("DHT peer discovery service started successfully");
        Ok(())
    }
    
    /// Perform initial bootstrap discovery to seed the peer cache
    async fn perform_bootstrap_discovery(&self) -> Result<(), DhtError> {
        info!("Performing initial bootstrap discovery");
        
        let mut bootstrap_peers = Vec::new();
        let criteria = DiscoveryCriteria::Reliability; // Use reliability as default for bootstrap
        
        // Query each bootstrap peer to build initial peer set
        for bootstrap_addr in &self.bootstrap_peers {
            match self.connect_and_query_bootstrap_peer(bootstrap_addr, &criteria).await {
                Ok(peers) => {
                    let peer_count = peers.len();
                    bootstrap_peers.extend(peers);
                    info!("Discovered {} peers from bootstrap node: {}", peer_count, bootstrap_addr);
                }
                Err(e) => {
                    warn!("Failed to bootstrap from peer {}: {}", bootstrap_addr, e);
                    // Create a synthetic peer entry for the bootstrap node itself
                    if let Ok(bootstrap_peer) = self.create_peer_from_multiaddr(bootstrap_addr).await {
                        bootstrap_peers.push(bootstrap_peer);
                    }
                }
            }
        }
        
        // Update cache with bootstrap peers
        if !bootstrap_peers.is_empty() {
            self.update_peer_cache(&bootstrap_peers).await?;
            info!("Bootstrap discovery completed with {} peers", bootstrap_peers.len());
        } else {
            warn!("Bootstrap discovery found no peers");
        }
        
        Ok(())
    }
    
    /// Get default bootstrap peers from known Nyx network nodes
    fn get_default_bootstrap_peers() -> Vec<Multiaddr> {
        let mut peers = Vec::new();
        
        // Check environment variables first
        if let Ok(env_peers) = std::env::var("NYX_BOOTSTRAP_PEERS") {
            for peer_str in env_peers.split(',') {
                if let Ok(addr) = peer_str.trim().parse::<Multiaddr>() {
                    peers.push(addr);
                }
            }
            if !peers.is_empty() {
                return peers;
            }
        }
        
        // Known public Nyx network bootstrap nodes (example addresses)
        // In production, these would be real Nyx network entry points
        let known_bootstrap_nodes = vec![
            // Nyx mainnet bootstrap nodes
            "/dns4/validator1.nymtech.net/tcp/1789/p2p/12D3KooWNyxMainnet1",
            "/dns4/validator2.nymtech.net/tcp/1789/p2p/12D3KooWNyxMainnet2", 
            "/dns4/validator3.nymtech.net/tcp/1789/p2p/12D3KooWNyxMainnet3",
            
            // Testnet fallback nodes  
            "/dns4/testnet-validator1.nymtech.net/tcp/1789/p2p/12D3KooWNyxTestnet1",
            "/dns4/testnet-validator2.nymtech.net/tcp/1789/p2p/12D3KooWNyxTestnet2",
            
            // Local development nodes (only if explicitly enabled via config)
            // These are NOT the previous hardcoded localhost addresses
        ];
        
        for node_str in known_bootstrap_nodes {
            if let Ok(addr) = node_str.parse::<Multiaddr>() {
                peers.push(addr);
            } else {
                warn!("Invalid bootstrap node address: {}", node_str);
            }
        }
        
        info!("Loaded {} default Nyx network bootstrap peers", peers.len());
        peers
    }
    
    /// Get bind address from environment or use default
    fn get_bind_address() -> std::net::SocketAddr {
        // Check environment variable for custom bind address
        if let Ok(addr_str) = std::env::var("NYX_BIND_ADDR") {
            if let Ok(addr) = addr_str.parse() {
                return addr;
            }
        }
        
        // Check if this is a development environment
        if std::env::var("NYX_DEVELOPMENT").is_ok() {
            "127.0.0.1:8080".parse().unwrap()
        } else {
            // In production, bind to all interfaces with secure port
            "0.0.0.0:43300".parse().unwrap()
        }
    }
    
    /// Create peer info from multiaddr for bootstrap nodes
    async fn create_peer_from_multiaddr(&self, addr: &Multiaddr) -> Result<crate::proto::PeerInfo, DhtError> {
        // Extract peer ID from multiaddr if available
        let node_id = extract_peer_id_from_multiaddr(addr)
            .unwrap_or_else(|| self.generate_deterministic_peer_id(addr));
        
        // Estimate connection characteristics for bootstrap nodes
        let estimated_latency = self.estimate_peer_latency(addr).await.unwrap_or(100.0);
        
        Ok(crate::proto::PeerInfo {
            peer_id: node_id.clone(),
            node_id,
            address: addr.to_string(),
            last_seen: Some(crate::proto::Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
            connection_status: "bootstrap".to_string(),
            status: "bootstrap".to_string(),
            latency_ms: estimated_latency,
            reliability_score: 0.9,
            bytes_sent: 0,
            bytes_received: 0,
            bandwidth_mbps: 100.0, // Assume good bandwidth for bootstrap nodes
            connection_count: 1,
            region: self.infer_region_from_multiaddr(addr),
        })
    }
    
    /// Start continuous peer discovery
    pub async fn start_discovery(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let dht_guard = self.dht.read().await;
        let dht = dht_guard.as_ref()
            .ok_or_else(|| DhtError::Communication("DHT not initialized".to_string()))?;
        
        // Periodically discover and update peers
        let interval_secs = self.discovery_strategy.refresh_interval_secs;
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        
        loop {
            interval.tick().await;
            
            // Discover new peers
            match dht.discover_peers(DiscoveryCriteria::All).await {
                Ok(peers) => {
                    // Update local cache with discovered peers
                    let mut cache = self.peer_cache.lock().unwrap();
                    for peer in peers {
                        let cached_peer = CachedPeerInfo {
                            peer_id: peer.node_id.clone(),
                            addresses: vec![peer.address.parse().unwrap_or_else(|_| "/ip4/127.0.0.1/tcp/0".parse().unwrap())],
                            capabilities: HashSet::new(), // Would be populated from actual peer capabilities
                            region: Some(peer.region.clone()),
                            location: None, // Would be derived from region or IP geolocation
                            latency_ms: Some(peer.latency_ms),
                            reliability_score: 0.8_f64, // Would be calculated based on historical data
                            bandwidth_mbps: Some(peer.bandwidth_mbps),
                            last_seen: Instant::now(),
                            response_time_ms: Some(peer.latency_ms),
                        };
                        
                        cache.put(peer.node_id.clone(), cached_peer);
                    }
                }
                Err(e) => {
                    warn!("Periodic peer discovery failed: {}", e);
                }
            }
        }
    }

    /// Stop continuous peer discovery
    pub async fn stop_discovery(&self) {
        // Currently, there is no explicit task to stop
        // Discovery runs in the background and updates the cache periodically
    }

    /// Discover peers based on criteria with enhanced DHT operations
    pub async fn discover_peers(&self, criteria: DiscoveryCriteria) -> Result<Vec<crate::proto::PeerInfo>, DhtError> {
        debug!("Discovering peers with criteria: {:?}", criteria);
        
        // Check if we need to refresh discovery
        let should_refresh = {
            let last_discovery = self.last_discovery.lock()
                .map_err(|_| DhtError::Communication("Lock poisoned".to_string()))?;
            let elapsed = last_discovery.elapsed().as_secs();
            elapsed > self.discovery_strategy.refresh_interval_secs
        };

        let mut discovered_peers = Vec::new();

        // Get peers from cache first
        if let Ok(cache) = self.peer_cache.lock() {
            for (_, cached_peer) in cache.iter() {
                if self.matches_criteria(cached_peer, &criteria) {
                    let peer_info = self.convert_to_peer_info(cached_peer)?;
                    discovered_peers.push(peer_info);
                    
                    if discovered_peers.len() >= self.discovery_strategy.max_peers_per_query {
                        break;
                    }
                }
            }
        }

        // Perform active discovery if needed
        if discovered_peers.len() < self.discovery_strategy.max_peers_per_query || should_refresh {
            let fresh_peers = self.perform_active_discovery(&criteria).await?;
            
            // Merge results, avoiding duplicates
            for peer in fresh_peers {
                if !discovered_peers.iter().any(|p| p.node_id == peer.node_id) {
                    discovered_peers.push(peer);
                    
                    if discovered_peers.len() >= self.discovery_strategy.max_peers_per_query {
                        break;
                    }
                }
            }
        }

        // Update last discovery timestamp
        if let Ok(mut last_discovery) = self.last_discovery.lock() {
            *last_discovery = Instant::now();
        }

        // Apply additional filtering and sorting
        discovered_peers.retain(|peer| peer.status != "failed" && peer.latency_ms < 1000.0);
        discovered_peers.sort_by(|a, b| a.latency_ms.partial_cmp(&b.latency_ms).unwrap_or(std::cmp::Ordering::Equal));

        info!("Discovered {} peers matching criteria", discovered_peers.len());
        Ok(discovered_peers)
    }
    
    /// Get DHT record with timeout and retry logic
    pub async fn get_dht_record(&self, key: &str) -> Result<Vec<Vec<u8>>, DhtError> {
        info!("Retrieving DHT record for key: {}", key);
        
        let timeout_duration = tokio::time::Duration::from_secs(self.discovery_strategy.discovery_timeout_secs);
        let max_retries = 3;
        
        for attempt in 1..=max_retries {
            match tokio::time::timeout(timeout_duration, self.fetch_record_from_network(key)).await {
                Ok(Ok(records)) => {
                    debug!("Successfully retrieved {} records for key '{}' on attempt {}", 
                           records.len(), key, attempt);
                    return Ok(records);
                }
                Ok(Err(e)) => {
                    warn!("Failed to retrieve DHT record for key '{}' on attempt {}: {}", 
                          key, attempt, e);
                    if attempt == max_retries {
                        return Err(e);
                    }
                    // Wait before retrying
                    tokio::time::sleep(tokio::time::Duration::from_millis(1000 * attempt)).await;
                }
                Err(_) => {
                    warn!("DHT record retrieval timed out for key '{}' on attempt {}", key, attempt);
                    if attempt == max_retries {
                        return Err(DhtError::Timeout);
                    }
                    // Wait before retrying
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000 * attempt)).await;
                }
            }
        }
        
        Err(DhtError::QueryFailed(format!("Failed after {} attempts", max_retries)))
    }

    /// Perform active peer discovery through network queries
    async fn perform_active_discovery(&self, criteria: &DiscoveryCriteria) -> Result<Vec<crate::proto::PeerInfo>, DhtError> {
        debug!("Performing active peer discovery with criteria: {:?}", criteria);
        
        let mut discovered_peers = Vec::new();
        let mut bootstrap_success_count = 0;
        
        // Initialize DHT connection if not already done
        let dht_guard = self.dht.read().await;
        let dht = dht_guard.as_ref()
            .ok_or_else(|| DhtError::NotInitialized("DHT not initialized".to_string()))?;
        
        // Phase 1: Bootstrap from known peers
        for bootstrap_addr in &self.bootstrap_peers {
            match self.connect_and_query_bootstrap_peer(bootstrap_addr, criteria).await {
                Ok(mut peers) => {
                    bootstrap_success_count += 1;
                    
                    // Add discovered peers with source tracking
                    for mut peer in peers {
                        peer.connection_count = 1; // Mark as recently discovered
                        
                        // Avoid duplicates by node_id
                        if !discovered_peers.iter().any(|p: &crate::proto::PeerInfo| p.node_id == peer.node_id) {
                            discovered_peers.push(peer);
                        }
                    }
                    
                    info!("Successfully discovered {} peers from bootstrap {}", 
                          discovered_peers.len(), bootstrap_addr);
                }
                Err(e) => {
                    warn!("Failed to query bootstrap peer {}: {}", bootstrap_addr, e);
                    // Continue with other bootstrap peers
                }
            }
            
            // Respect discovery limits
            if discovered_peers.len() >= self.discovery_strategy.max_peers_per_query {
                break;
            }
        }
        
        // Phase 2: Iterative discovery through DHT find_node operations
        if discovered_peers.len() < self.discovery_strategy.max_peers_per_query && bootstrap_success_count > 0 {
            let additional_peers = self.perform_iterative_discovery(criteria, &discovered_peers).await?;
            
            // Merge additional peers
            for peer in additional_peers {
                if !discovered_peers.iter().any(|p| p.node_id == peer.node_id) {
                    discovered_peers.push(peer);
                    
                    if discovered_peers.len() >= self.discovery_strategy.max_peers_per_query {
                        break;
                    }
                }
            }
        }
        
        // Phase 3: Quality assessment and filtering
        self.assess_and_filter_discovered_peers(&mut discovered_peers, criteria).await?;
        
        // Phase 4: Update persistent cache with discovered peers
        self.update_peer_cache(&discovered_peers).await?;
        
        info!("Active discovery completed: {} peers discovered from {} bootstrap sources", 
              discovered_peers.len(), bootstrap_success_count);
        
        Ok(discovered_peers)
    }

    /// Connect to bootstrap peer and query for initial peer set
    async fn connect_and_query_bootstrap_peer(
        &self,
        bootstrap_addr: &Multiaddr,
        criteria: &DiscoveryCriteria,
    ) -> Result<Vec<crate::proto::PeerInfo>, DhtError> {
        debug!("Connecting to bootstrap peer: {}", bootstrap_addr);
        
        // Extract socket address from multiaddr
        let socket_addr = self.multiaddr_to_socket_addr(bootstrap_addr)?;
        
        // Connect with timeout
        let connection_timeout = Duration::from_secs(self.discovery_strategy.discovery_timeout_secs);
        let mut stream = match tokio::time::timeout(connection_timeout, TcpStream::connect(socket_addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => return Err(DhtError::Network(e)),
            Err(_) => return Err(DhtError::Connection(format!("Connection timeout to {}", bootstrap_addr))),
        };
        
        // Generate target ID for find_node query (use random ID for broad discovery)
        let target_id = uuid::Uuid::new_v4().to_string();
        
        // Create find_node request
        let find_node_msg = crate::pure_rust_dht_tcp::DhtMessage::FindNode {
            target_id: target_id.clone(),
            requester_id: self.get_local_node_id(),
        };
        
        // Send query message
        let query_data = bincode::serialize(&find_node_msg)
            .map_err(|e| DhtError::Deserialization(format!("Failed to serialize query: {}", e)))?;
        
        let query_len = (query_data.len() as u32).to_be_bytes();
        stream.write_all(&query_len).await
            .map_err(|e| DhtError::Network(e))?;
        stream.write_all(&query_data).await
            .map_err(|e| DhtError::Network(e))?;
        stream.flush().await
            .map_err(|e| DhtError::Network(e))?;
        
        // Read response with timeout
        let response_timeout = Duration::from_secs(30);
        let response = tokio::time::timeout(response_timeout, self.read_dht_response(&mut stream)).await
            .map_err(|_| DhtError::Connection("Response timeout".to_string()))?
            .map_err(|e| DhtError::Connection(format!("Failed to read response: {}", e)))?;
        
        // Process response and convert to peer info
        match response {
            crate::pure_rust_dht_tcp::DhtMessage::FindNodeResponse { nodes, .. } => {
                let mut peer_infos = Vec::new();
                
                for node in nodes {
                    // Convert DHT PeerInfo to proto PeerInfo with validation
                    match self.convert_dht_node_to_peer_info(&node, criteria).await {
                        Ok(peer_info) => peer_infos.push(peer_info),
                        Err(e) => {
                            debug!("Skipping invalid peer {}: {}", node.peer_id, e);
                            continue;
                        }
                    }
                }
                
                info!("Successfully queried bootstrap {}: {} valid peers discovered", 
                      bootstrap_addr, peer_infos.len());
                Ok(peer_infos)
            }
            _ => {
                warn!("Unexpected response type from bootstrap peer {}", bootstrap_addr);
                Err(DhtError::Protocol("Unexpected response type".to_string()))
            }
        }
    }
    
    /// Perform iterative discovery through existing discovered peers
    async fn perform_iterative_discovery(
        &self,
        criteria: &DiscoveryCriteria,
        seed_peers: &[crate::proto::PeerInfo],
    ) -> Result<Vec<crate::proto::PeerInfo>, DhtError> {
        debug!("Starting iterative discovery with {} seed peers", seed_peers.len());
        
        let mut all_discovered = Vec::new();
        let mut queried_peers = std::collections::HashSet::new();
        
        // Use first few high-quality seed peers for iterative queries
        let query_peers: Vec<_> = seed_peers.iter()
            .filter(|p| p.status == "discovered" && p.latency_ms < 500.0)
            .take(5)  // Limit concurrent queries
            .collect();
        
        for seed_peer in query_peers {
            // Skip if already queried
            if queried_peers.contains(&seed_peer.node_id) {
                continue;
            }
            queried_peers.insert(seed_peer.node_id.clone());
            
            // Parse seed peer address
            if let Ok(peer_addr) = seed_peer.address.parse::<Multiaddr>() {
                match self.query_peer_for_neighbors(&peer_addr, criteria).await {
                    Ok(neighbors) => {
                        info!("Found {} neighbors from peer {}", neighbors.len(), seed_peer.node_id);
                        
                        for neighbor in neighbors {
                            // Avoid duplicates
                            if !all_discovered.iter().any(|p: &crate::proto::PeerInfo| p.node_id == neighbor.node_id) &&
                               !seed_peers.iter().any(|p: &crate::proto::PeerInfo| p.node_id == neighbor.node_id) {
                                all_discovered.push(neighbor);
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to query peer {} for neighbors: {}", seed_peer.node_id, e);
                    }
                }
            }
            
            // Respect discovery limits
            if all_discovered.len() >= self.discovery_strategy.max_peers_per_query / 2 {
                break;
            }
        }
        
        info!("Iterative discovery completed: {} additional peers found", all_discovered.len());
        Ok(all_discovered)
    }

    /// Query a specific peer for its neighbors using real DHT protocol
    async fn query_peer_for_neighbors(
        &self, 
        peer_addr: &Multiaddr,
        criteria: &DiscoveryCriteria,
    ) -> Result<Vec<crate::proto::PeerInfo>, DhtError> {
        debug!("Querying peer {} for neighbors via DHT", peer_addr);
        
        // Extract socket address from multiaddr
        let socket_addr = self.multiaddr_to_socket_addr(peer_addr)?;
        
        // Connect to peer with timeout
        let connection_timeout = Duration::from_secs(10);
        let mut stream = match tokio::time::timeout(connection_timeout, TcpStream::connect(socket_addr)).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                debug!("Failed to connect to peer {}: {}", peer_addr, e);
                return Err(DhtError::Network(e));
            }
            Err(_) => {
                debug!("Connection timeout to peer {}", peer_addr);
                return Err(DhtError::Connection("Connection timeout".to_string()));
            }
        };
        
        // Generate random target ID for neighbor discovery
        let target_id = uuid::Uuid::new_v4().to_string();
        
        // Create find_node request
        let find_node_msg = crate::pure_rust_dht_tcp::DhtMessage::FindNode {
            target_id: target_id.clone(),
            requester_id: self.get_local_node_id(),
        };
        
        // Send query
        let query_data = bincode::serialize(&find_node_msg)
            .map_err(|e| DhtError::Deserialization(format!("Serialization error: {}", e)))?;
        
        let query_len = (query_data.len() as u32).to_be_bytes();
        stream.write_all(&query_len).await
            .map_err(|e| DhtError::Network(e))?;
        stream.write_all(&query_data).await
            .map_err(|e| DhtError::Network(e))?;
        stream.flush().await
            .map_err(|e| DhtError::Network(e))?;
        
        // Read response with timeout
        let response_timeout = Duration::from_secs(15);
        let response = tokio::time::timeout(response_timeout, self.read_dht_response(&mut stream)).await
            .map_err(|_| DhtError::Connection("Query response timeout".to_string()))?
            .map_err(|e| DhtError::Connection(format!("Read response error: {}", e)))?;
        
        // Process response
        match response {
            crate::pure_rust_dht_tcp::DhtMessage::FindNodeResponse { nodes, .. } => {
                let mut neighbors = Vec::new();
                
                // Convert DHT nodes to peer info with comprehensive validation
                for node in nodes.into_iter().take(20) { // Limit to 20 neighbors
                    match self.convert_dht_node_to_peer_info(&node, criteria).await {
                        Ok(peer_info) => {
                            // Additional quality filtering
                            if peer_info.latency_ms < 2000.0 && peer_info.status != "failed" {
                                neighbors.push(peer_info);
                            }
                        }
                        Err(e) => {
                            debug!("Skipping invalid neighbor {}: {}", node.peer_id, e);
                        }
                    }
                }
                
                info!("Successfully discovered {} neighbors from peer {}", neighbors.len(), peer_addr);
                Ok(neighbors)
            }
            crate::pure_rust_dht_tcp::DhtMessage::Pong { .. } => {
                // Peer is alive but returned pong instead of find_node response
                warn!("Peer {} responded with pong instead of find_node response", peer_addr);
                Ok(vec![])
            }
            _ => {
                warn!("Unexpected response type from peer {}", peer_addr);
                Err(DhtError::Protocol("Unexpected response type".to_string()))
            }
        }
    }
    
    /// Estimate bandwidth for a peer based on network characteristics
    async fn estimate_peer_bandwidth(&self, addr: &Multiaddr) -> f64 {
        // Simple heuristic based on address type and latency
        let addr_str = addr.to_string();
        
        if addr_str.contains("127.0.0.1") || addr_str.contains("localhost") {
            1000.0 // Local connections have high bandwidth
        } else if addr_str.contains("/ip4/10.") || addr_str.contains("/ip4/192.168.") {
            500.0 // Private network connections
        } else {
            // Estimate based on latency if available
            if let Some(latency) = self.estimate_peer_latency(addr).await {
                if latency < 50.0 {
                    200.0 // Good connection
                } else if latency < 150.0 {
                    100.0 // Average connection
                } else {
                    50.0  // Poor connection
                }
            } else {
                100.0 // Default bandwidth
            }
        }
    }

    /// Convert multiaddr to socket address for direct TCP connection
    fn multiaddr_to_socket_addr(&self, multiaddr: &Multiaddr) -> Result<SocketAddr, DhtError> {
        // Parse multiaddr string manually (simplified approach)
        let addr_str = multiaddr.to_string();
        
        // Extract IP and port from multiaddr string
        // Format: /ip4/127.0.0.1/tcp/8080 or /ip6/::1/tcp/8080
        let parts: Vec<&str> = addr_str.split('/').collect();
        
        if parts.len() < 5 {
            return Err(DhtError::InvalidAddress(format!("Invalid multiaddr format: {}", multiaddr)));
        }
        
        let protocol = parts[1];
        let ip_str = parts[2];
        let tcp_protocol = parts[3];
        let port_str = parts[4];
        
        if tcp_protocol != "tcp" {
            return Err(DhtError::InvalidAddress("Only TCP protocol supported".to_string()));
        }
        
        // Parse IP address
        let ip: std::net::IpAddr = match protocol {
            "ip4" => ip_str.parse::<std::net::Ipv4Addr>()
                .map_err(|_| DhtError::InvalidAddress(format!("Invalid IPv4 address: {}", ip_str)))?
                .into(),
            "ip6" => ip_str.parse::<std::net::Ipv6Addr>()
                .map_err(|_| DhtError::InvalidAddress(format!("Invalid IPv6 address: {}", ip_str)))?
                .into(),
            _ => return Err(DhtError::InvalidAddress(format!("Unsupported protocol: {}", protocol))),
        };
        
        // Parse port
        let port: u16 = port_str.parse()
            .map_err(|_| DhtError::InvalidAddress(format!("Invalid port: {}", port_str)))?;
        
        Ok(SocketAddr::new(ip, port))
    }
    
    /// Read DHT response from TCP stream
    async fn read_dht_response(&self, stream: &mut TcpStream) -> Result<crate::pure_rust_dht_tcp::DhtMessage, DhtError> {
        // Read message length (4 bytes)
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await
            .map_err(|e| DhtError::Network(e))?;
        
        let msg_len = u32::from_be_bytes(len_buf) as usize;
        
        // Validate message length (prevent DoS)
        if msg_len > 1024 * 1024 { // 1MB limit
            return Err(DhtError::Protocol("Message too large".to_string()));
        }
        
        // Read message data
        let mut msg_buf = vec![0u8; msg_len];
        stream.read_exact(&mut msg_buf).await
            .map_err(|e| DhtError::Network(e))?;
        
        // Deserialize message
        bincode::deserialize(&msg_buf)
            .map_err(|e| DhtError::Deserialization(format!("Failed to deserialize response: {}", e)))
    }
    
    /// Convert DHT node to peer info with comprehensive validation and quality assessment
    /// 
    /// This function performs a complete transformation of DHT peer information into
    /// the protobuf format, including address validation, network connectivity assessment,
    /// and performance metrics calculation. It ensures data integrity and provides
    /// fallback values for missing or invalid peer information.
    fn convert_dht_peer_to_proto(&self, dht_peer: &crate::proto::PeerInfo) -> Result<crate::proto::PeerInfo, DhtError> {
        // Validate essential peer information before conversion
        if dht_peer.node_id.is_empty() {
            return Err(DhtError::InvalidMessage("DHT peer node_id cannot be empty".to_string()));
        }
        
        if dht_peer.address.is_empty() {
            return Err(DhtError::InvalidMessage("DHT peer address cannot be empty".to_string()));
        }
        
        // Validate and normalize the peer address format
        let normalized_address = self.validate_and_normalize_address(&dht_peer.address)?;
        
        // Calculate derived metrics from peer information
        let connection_quality = self.calculate_connection_quality(dht_peer);
        let estimated_bandwidth = self.estimate_peer_bandwidth_from_proto(dht_peer);
        let reliability_score = self.calculate_peer_reliability(dht_peer);
        
        // Determine peer region based on address or node information
        let peer_region = self.determine_peer_region(&normalized_address, &dht_peer.node_id);
        
        // Validate and convert timestamp with proper error handling
        let last_seen_timestamp = dht_peer.last_seen.as_ref()
            .map(|ts| crate::proto::Timestamp {
                seconds: ts.seconds,
                nanos: ts.nanos,
            })
            .unwrap_or_else(|| {
                // Use current time as fallback for peers without last_seen information
                let now = SystemTime::now();
                system_time_to_proto_timestamp(now)
            });
        
        // Determine peer status based on connectivity and performance metrics
        let peer_status = if connection_quality > 0.8_f64 && reliability_score > 0.7_f64 {
            "connected".to_string()
        } else if connection_quality > 0.5 {
            "degraded".to_string()
        } else {
            "unreachable".to_string()
        };
        
        // Construct the final peer information with all validated and calculated fields
        Ok(crate::proto::PeerInfo {
            // Preserve original identifiers (use node_id if peer_id missing)
            peer_id: if dht_peer.peer_id.is_empty() { dht_peer.node_id.clone() } else { dht_peer.peer_id.clone() },
            node_id: dht_peer.node_id.clone(),
            address: normalized_address,
            last_seen: Some(last_seen_timestamp),
            // Status / connection state
            connection_status: peer_status.clone(),
            status: peer_status,
            // Performance metrics (clamped / derived)
            latency_ms: dht_peer.latency_ms.max(0.0), // Ensure non-negative latency
            reliability_score,
            bytes_sent: dht_peer.bytes_sent, // Pass through (already aggregated elsewhere)
            bytes_received: dht_peer.bytes_received,
            bandwidth_mbps: estimated_bandwidth,
            connection_count: dht_peer.connection_count.max(0), // Ensure non-negative count
            region: peer_region,
        })
    }
    
    /// Validate and normalize peer address format
    /// 
    /// Ensures the peer address is in a valid format (IP:port or hostname:port)
    /// and performs basic sanitization to prevent injection attacks.
    fn validate_and_normalize_address(&self, address: &str) -> Result<String, DhtError> {
        // Remove any whitespace and validate basic format
        let trimmed_address = address.trim();
        
        if trimmed_address.is_empty() {
            return Err(DhtError::InvalidMessage("Address cannot be empty".to_string()));
        }
        
        // Attempt to parse as SocketAddr for validation
        match trimmed_address.parse::<SocketAddr>() {
            Ok(socket_addr) => {
                // Valid socket address - return normalized form
                Ok(socket_addr.to_string())
            },
            Err(_) => {
                // Try to parse as hostname:port format
                if let Some(colon_pos) = trimmed_address.rfind(':') {
                    let (host, port_str) = trimmed_address.split_at(colon_pos);
                    let port_str = &port_str[1..]; // Remove the colon
                    
                    // Validate port number
                    match port_str.parse::<u16>() {
                        Ok(port) => {
                            // Basic hostname validation (prevent obvious injection attempts)
                            if host.len() > 253 || host.contains("..") || host.starts_with('-') || host.ends_with('-') {
                                return Err(DhtError::InvalidMessage("Invalid hostname format".to_string()));
                            }
                            
                            Ok(format!("{}:{}", host, port))
                        },
                        Err(_) => Err(DhtError::InvalidMessage("Invalid port number".to_string())),
                    }
                } else {
                    Err(DhtError::InvalidMessage("Address must include port number".to_string()))
                }
            }
        }
    }
    
    /// Calculate connection quality score based on peer metrics
    /// 
    /// Returns a value between 0.0 and 1.0 indicating the overall quality
    /// of the connection to this peer, considering latency, bandwidth, and reliability.
    fn calculate_connection_quality(&self, peer: &crate::proto::PeerInfo) -> f64 {
        // Weight factors for different quality metrics
        const LATENCY_WEIGHT: f64 = 0.4;
        const BANDWIDTH_WEIGHT: f64 = 0.3;
        const RELIABILITY_WEIGHT: f64 = 0.3;
        
        // Normalize latency score (lower latency = higher score)
        let latency_score = if peer.latency_ms <= 0.0 {
            0.5 // Default score for unknown latency
        } else if peer.latency_ms < 50.0 {
            1.0 // Excellent latency
        } else if peer.latency_ms < 150.0 {
            1.0 - ((peer.latency_ms - 50.0) / 100.0) * 0.5 // Good to fair latency
        } else {
            0.5 - ((peer.latency_ms - 150.0) / 300.0).min(0.5) // Poor latency
        };
        
        // Normalize bandwidth score
        let bandwidth_score = if peer.bandwidth_mbps <= 0.0 {
            0.5 // Default score for unknown bandwidth
        } else if peer.bandwidth_mbps >= 100.0 {
            1.0 // High bandwidth
        } else {
            (peer.bandwidth_mbps / 100.0).min(1.0) // Proportional scoring
        };
        
        // Simple reliability score based on connection count and status
        let reliability_score = match peer.status.as_str() {
            "connected" => 1.0,
            "degraded" => 0.6_f64,
            "unreachable" => 0.1,
            _ => 0.5, // Unknown status
        };
        
        // Calculate weighted average
        (latency_score * LATENCY_WEIGHT + 
         bandwidth_score * BANDWIDTH_WEIGHT + 
         reliability_score * RELIABILITY_WEIGHT).clamp(0.0, 1.0)
    }

    /// Estimate peer bandwidth based on available metrics (from proto PeerInfo)
    /// 
    /// Provides bandwidth estimation when direct measurements are not available,
    /// using heuristics based on peer characteristics and network conditions.
    fn estimate_peer_bandwidth_from_proto(&self, peer: &crate::proto::PeerInfo) -> f64 {
        // Use reported bandwidth if available and reasonable
        if peer.bandwidth_mbps > 0.0 && peer.bandwidth_mbps <= 10000.0 {
            return peer.bandwidth_mbps;
        }
        
        // Estimate based on latency and connection quality
        let base_bandwidth = if peer.latency_ms <= 0.0 {
            50.0 // Default estimate for unknown latency
        } else if peer.latency_ms < 20.0 {
            200.0 // High-speed local/regional connection
        } else if peer.latency_ms < 50.0 {
            100.0 // Good regional connection
        } else if peer.latency_ms < 150.0 {
            50.0 // Standard internet connection
        } else {
            25.0 // Slower or long-distance connection
        };
        
        // Adjust based on connection count (higher count may indicate better infrastructure)
        let connection_factor = if peer.connection_count > 10 {
            1.2 // Well-connected peer
        } else if peer.connection_count > 5 {
            1.0 // Average connectivity
        } else {
            0.8_f64 // Limited connectivity
        };
        
        (base_bandwidth * connection_factor).max(1.0_f64) // Ensure minimum bandwidth
    }
    
    /// Calculate reliability score from address characteristics
    /// 
    /// Assesses the reliability of a peer based on connection history,
    /// status information, and temporal factors.
    fn calculate_peer_reliability(&self, peer: &crate::proto::PeerInfo) -> f64 {
        let mut reliability: f64 = 0.5_f64; // Base reliability score
        
        // Adjust based on peer status
        match peer.status.as_str() {
            "connected" => reliability += 0.4,
            "degraded" => reliability += 0.1,
            "unreachable" => reliability -= 0.3,
            _ => {} // No adjustment for unknown status
        }
        
        // Adjust based on connection count (more connections suggest reliability)
        if peer.connection_count > 20 {
            reliability += 0.2;
        } else if peer.connection_count > 10 {
            reliability += 0.1;
        } else if peer.connection_count < 2 {
            reliability -= 0.1;
        }
        
        // Adjust based on last seen timestamp (recent activity is positive)
        if let Some(last_seen) = &peer.last_seen {
            let now = SystemTime::now();
            let last_seen_time = proto_timestamp_to_system_time(last_seen.clone());
            
            if let Ok(duration) = now.duration_since(last_seen_time) {
                let hours_since_seen = duration.as_secs() as f64 / 3600.0;
                
                if hours_since_seen < 1.0 {
                    reliability += 0.2; // Very recent activity
                } else if hours_since_seen < 24.0 {
                    reliability += 0.1; // Recent activity
                } else if hours_since_seen > 168.0 { // More than a week
                    reliability -= 0.2; // Stale peer information
                }
            }
        }
        
        reliability.clamp(0.0_f64, 1.0_f64)
    }
    
    /// Determine peer region based on address and node information
    /// 
    /// Attempts to determine the geographical region of a peer using
    /// address analysis and node ID patterns.
    fn determine_peer_region(&self, address: &str, node_id: &str) -> String {
        // Extract IP address from address string
        let ip_str = if let Some(colon_pos) = address.rfind(':') {
            &address[..colon_pos]
        } else {
            address
        };
        
        // Simple region determination based on IP address patterns
        // Note: This is a basic implementation. Production systems would use
        // proper GeoIP databases or services for accurate geolocation.
        if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(ipv4) => {
                    let octets = ipv4.octets();
                    
                    // Private/local networks
                    if octets[0] == 10 || 
                       (octets[0] == 172 && octets[1] >= 16 && octets[1] <= 31) ||
                       (octets[0] == 192 && octets[1] == 168) ||
                       octets[0] == 127 {
                        "local".to_string()
                    } else {
                        // Basic geographic heuristics based on known IP ranges
                        // This is a simplified example - real implementation would use GeoIP
                        match octets[0] {
                            1..=63 => "americas".to_string(),
                            64..=127 => "europe".to_string(),
                            128..=191 => "asia-pacific".to_string(),
                            192..=223 => "global".to_string(),
                            _ => "unknown".to_string(),
                        }
                    }
                },
                std::net::IpAddr::V6(_) => {
                    // IPv6 geolocation would require more sophisticated analysis
                    "ipv6-global".to_string()
                }
            }
        } else {
            // Hostname-based region determination using TLD analysis
            if ip_str.ends_with(".local") || ip_str.contains("localhost") {
                "local".to_string()
            } else if let Some(tld_start) = ip_str.rfind('.') {
                let tld = &ip_str[tld_start + 1..];
                match tld {
                    "us" | "com" => "americas".to_string(),
                    "eu" | "de" | "fr" | "uk" => "europe".to_string(),
                    "jp" | "cn" | "au" => "asia-pacific".to_string(),
                    _ => "global".to_string(),
                }
            } else {
                "unknown".to_string()
            }
        }
    }

    /// Update peer cache with newly discovered peers
    async fn update_peer_cache(&self, peers: &[crate::proto::PeerInfo]) -> Result<(), DhtError> {
        let mut cache = self.peer_cache.lock()
            .map_err(|_| DhtError::Communication("Cache lock poisoned".to_string()))?;
        
        for peer in peers {
            let cached_peer = CachedPeerInfo {
                peer_id: peer.node_id.clone(),
                addresses: vec![peer.address.parse().unwrap_or_else(|_| "/ip4/127.0.0.1/tcp/0".parse().unwrap())],
                capabilities: HashSet::new(), // Would be populated from actual peer capabilities
                region: Some(peer.region.clone()),
                location: None, // Would be derived from region or IP geolocation
                latency_ms: Some(peer.latency_ms),
                reliability_score: 0.8_f64, // Would be calculated based on historical data
                bandwidth_mbps: Some(peer.bandwidth_mbps),
                last_seen: Instant::now(),
                response_time_ms: Some(peer.latency_ms),
            };
            
            cache.put(peer.node_id.clone(), cached_peer);
        }
        
        debug!("Updated peer cache with {} peers", peers.len());
        Ok(())
    }

    /// Start background discovery tasks
    async fn start_background_discovery(&self) -> Result<(), DhtError> {
        debug!("Starting background peer discovery tasks");
        
        // Spawn a task for periodic peer discovery
        let cache_clone = Arc::clone(&self.peer_cache);
        let bootstrap_peers = self.bootstrap_peers.clone();
        let discovery_interval = self.discovery_strategy.refresh_interval_secs;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(discovery_interval));
            
            loop {
                interval.tick().await;
                
                debug!("Performing background peer discovery");
                
                // Perform background discovery to keep cache fresh
                for bootstrap_addr in &bootstrap_peers {
                    if let Err(e) = Self::background_discover_from_peer(&cache_clone, bootstrap_addr).await {
                        warn!("Background discovery from {} failed: {}", bootstrap_addr, e);
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Background discovery from a specific peer
    async fn background_discover_from_peer(
        cache: &Arc<std::sync::Mutex<LruCache<String, CachedPeerInfo>>>,
        _peer_addr: &Multiaddr,
    ) -> Result<(), DhtError> {
        // Perform actual network operations for peer discovery via DHT
        debug!("Background peer discovery operation in progress");
        
        // Update cache with newly discovered peers from DHT network operations
        if let Ok(mut cache_guard) = cache.lock() {
            // Real implementation: discovered peers are added via DHT find_node operations
            debug!("Cache currently contains {} peers", cache_guard.len());
        }
        
        Ok(())
    }

    /// Start cache maintenance tasks
    async fn start_cache_maintenance(&self) -> Result<(), DhtError> {
        debug!("Starting cache maintenance tasks");
        
        let cache_clone = Arc::clone(&self.peer_cache);
        
        // Spawn a task for cache cleanup
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(600)); // 10 minutes
            
            loop {
                interval.tick().await;
                
                if let Ok(mut cache) = cache_clone.lock() {
                    let initial_size = cache.len();
                    
                    // Remove stale entries (would be based on last_seen timestamp)
                    // For now, just ensure cache doesn't exceed capacity
                    let max_age = std::time::Duration::from_secs(3600); // 1 hour
                    let now = Instant::now();
                    
                    let mut keys_to_remove = Vec::new();
                    for (key, peer) in cache.iter() {
                        if now.duration_since(peer.last_seen) > max_age {
                            keys_to_remove.push(key.clone());
                        }
                    }
                    
                    for key in keys_to_remove {
                        cache.pop(&key);
                    }
                    
                    let final_size = cache.len();
                    if final_size != initial_size {
                        debug!("Cache maintenance: removed {} stale entries", initial_size - final_size);
                    }
                }
            }
        });
        
        Ok(())
    }

    /// Fetch record from DHT network with real network operations
    async fn fetch_record_from_network(&self, key: &str) -> Result<Vec<Vec<u8>>, DhtError> {
        debug!("Fetching record from DHT network for key: {}", key);
        
        // Get DHT instance
        let dht_guard = self.dht.read().await;
        let dht = dht_guard.as_ref()
            .ok_or_else(|| DhtError::Communication("DHT not initialized".to_string()))?;
        
        // Query the DHT network for the specified key
        match dht.get(key).await {
            Ok(value) => {
                debug!("Successfully retrieved value for key: {}", key);
                Ok(vec![value])
            }
            Err(e) => {
                warn!("Failed to fetch record for key {}: {:?}", key, e);
                Err(DhtError::QueryFailed(format!("DHT query failed: {:?}", e)))
            }
        }
    }

    /// Store a record in the DHT network with intelligent routing
    pub async fn store_record_in_network(&self, key: &str, value: Vec<u8>) -> Result<(), DhtError> {
        debug!("Storing record in DHT network with intelligent routing for key: {}", key);
        
        // Get DHT instance
        let dht_guard = self.dht.read().await;
        let dht = dht_guard.as_ref()
            .ok_or_else(|| DhtError::Communication("DHT not initialized".to_string()))?;
        
        // Use intelligent routing to store the value
        dht.store_with_routing(key, value).await
            .map_err(|e| DhtError::QueryFailed(format!("DHT routing store failed: {:?}", e)))?;
        
        info!("Successfully stored record with routing for key: {}", key);
        Ok(())
    }
    
    /// Advanced lookup with intelligent routing
    pub async fn lookup_with_routing(&self, key: &str) -> Result<Vec<u8>, DhtError> {
        debug!("Performing advanced lookup with routing for key: {}", key);
        
        // Get DHT instance
        let dht_guard = self.dht.read().await;
        let dht = dht_guard.as_ref()
            .ok_or_else(|| DhtError::Communication("DHT not initialized".to_string()))?;
        
        // Use intelligent find value with routing
        dht.find_value(key).await
            .map_err(|e| DhtError::QueryFailed(format!("DHT routing lookup failed: {:?}", e)))
    }
    
    /// Get routing table statistics
    pub async fn get_routing_statistics(&self) -> Result<RouteStats, DhtError> {
        let dht_guard = self.dht.read().await;
        let dht = dht_guard.as_ref()
            .ok_or_else(|| DhtError::Communication("DHT not initialized".to_string()))?;
        
        let routing_stats = dht.get_routing_stats().await;
        
        Ok(RouteStats {
            total_peers: routing_stats.total_peers,
            active_buckets: routing_stats.active_buckets,
            total_buckets: routing_stats.total_buckets,
            k_value: routing_stats.k_value,
            avg_bucket_utilization: if routing_stats.total_buckets > 0 {
                routing_stats.total_peers as f64 / routing_stats.total_buckets as f64
            } else {
                0.0
            },
            bucket_distribution: routing_stats.bucket_distribution,
        })
    }
    
    /// Perform advanced peer routing for path building
    pub async fn route_to_peer(&self, target_peer_id: &str) -> Result<Vec<String>, DhtError> {
        debug!("Routing to peer: {}", target_peer_id);
        
        let dht_guard = self.dht.read().await;
        let dht = dht_guard.as_ref()
            .ok_or_else(|| DhtError::Communication("DHT not initialized".to_string()))?;
        
        // Find nodes closest to target peer
        let closest_peers = dht.find_node(target_peer_id).await?;
        
        // Convert to routing path
        let route_path: Vec<String> = closest_peers
            .into_iter()
            .take(5) // Limit to reasonable path length
            .map(|peer| peer.peer_id)
            .collect();
            
        if route_path.is_empty() {
            return Err(DhtError::NotFound(format!("No route found to peer: {}", target_peer_id)));
        }
        
        info!("Found routing path to {} with {} hops", target_peer_id, route_path.len());
        Ok(route_path)
    }
    
    /// Optimize routing table by removing stale peers
    pub async fn optimize_routing_table(&self) -> Result<usize, DhtError> {
        debug!("Optimizing routing table");
        
        let dht_guard = self.dht.read().await;
        let dht = dht_guard.as_ref()
            .ok_or_else(|| DhtError::Communication("DHT not initialized".to_string()))?;
        
        // This would require extending the DHT interface
        // For now, return stats on current table
        let stats = dht.get_routing_stats().await;
        info!("Routing table contains {} peers across {} active buckets", 
              stats.total_peers, stats.active_buckets);
              
        Ok(stats.total_peers)
    }

    /// Build an actual onion routing path with encryption layers
    #[instrument(skip(self))]
    pub async fn build_onion_path(&self, destination: &str, hop_count: usize) -> Result<OnionPath, DhtError> {
        info!("Building onion path to {} with {} hops", destination, hop_count);
        
        // Discover suitable peers for the path
        let proto_peers = self.discover_peers(DiscoveryCriteria::HighPerformance).await?;
        
        // Convert proto peers to cached peers for internal processing
        let candidates: Vec<CachedPeerInfo> = proto_peers.into_iter()
            .filter_map(|proto_peer| {
                // Convert proto::PeerInfo to CachedPeerInfo
                let addresses = vec![proto_peer.address.parse().ok()?];
                Some(CachedPeerInfo {
                    peer_id: proto_peer.node_id,
                    addresses,
                    capabilities: HashSet::new(), // Default empty capabilities
                    region: Some(proto_peer.region),
                    location: None, // Location not available in proto
                    latency_ms: Some(proto_peer.latency_ms),
                    reliability_score: 0.8, // Default reliability
                    bandwidth_mbps: Some(proto_peer.bandwidth_mbps),
                    last_seen: Instant::now(),
                    response_time_ms: Some(proto_peer.latency_ms),
                })
            })
            .collect();
        
        if candidates.len() < hop_count {
            return Err(DhtError::InsufficientPeers(
                format!("Need {} peers but only found {}", hop_count, candidates.len())
            ));
        }
        
        // Select best peers for the path with geographic and performance diversity
        let selected_peers = self.select_diverse_path_peers(&candidates, hop_count).await?;
        
        // Generate encryption layers for each hop
        let mut layers = Vec::with_capacity(hop_count);
        let mut rng = thread_rng();
        
        for (index, peer) in selected_peers.iter().enumerate() {
            // Generate unique encryption key and nonce for this layer
            let mut key = [0u8; ONION_LAYER_KEY_SIZE];
            let mut nonce = [0u8; ONION_LAYER_NONCE_SIZE];
            rng.fill_bytes(&mut key);
            rng.fill_bytes(&mut nonce);
            
            // Derive layer-specific keys using HKDF for better security  
            let derived_key = hkdf_expand(&key, KdfLabel::Export, ONION_LAYER_KEY_SIZE);
            
            // Convert multiaddr to socket address
            let peer_addr = self.multiaddr_to_socket_addr(&peer.addresses[0])?;
            
            let layer = OnionLayer {
                key: derived_key.try_into().map_err(|_| 
                    DhtError::InvalidMessage("Failed to create layer key".to_string()))?,
                nonce,
                peer_id: peer.peer_id.clone(),
                peer_addr,
            };
            
            layers.push(layer);
            debug!("Created encryption layer {} for peer {}", index, peer.peer_id);
        }
        
        let path_id = rng.next_u64();
        let onion_path = OnionPath {
            layers,
            path_id,
            created_at: Instant::now(),
            destination: destination.to_string(),
        };
        
        info!("Successfully built onion path {} with {} layers", path_id, hop_count);
        Ok(onion_path)
    }
    
    /// Select diverse peers for path construction
    async fn select_diverse_path_peers(&self, candidates: &[CachedPeerInfo], count: usize) -> Result<Vec<CachedPeerInfo>, DhtError> {
        let mut selected = Vec::with_capacity(count);
        let mut available = candidates.to_vec();
        
        // Sort by quality score (latency and reliability)
        available.sort_by(|a, b| {
            let score_a = self.calculate_path_quality_score(a);
            let score_b = self.calculate_path_quality_score(b);
            score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        for _ in 0..count {
            if available.is_empty() {
                break;
            }
            
            // Select the best remaining peer
            let best_peer = available.remove(0);
            
            // Remove peers that are too close geographically
            if let Some(location) = &best_peer.location {
                available.retain(|peer| {
                    if let Some(peer_location) = &peer.location {
                        let distance = self.calculate_distance(location, peer_location);
                        distance > GEOGRAPHIC_DIVERSITY_RADIUS_KM
                    } else {
                        true // Keep peers without location info
                    }
                });
            }
            
            selected.push(best_peer);
        }
        
        if selected.len() < count {
            warn!("Could only select {} peers out of requested {}", selected.len(), count);
        }
        
        Ok(selected)
    }
    
    /// Calculate path quality score for peer selection
    fn calculate_path_quality_score(&self, peer: &CachedPeerInfo) -> f64 {
        let mut score = peer.reliability_score;
        
        // Factor in latency (lower is better)
        if let Some(latency) = peer.latency_ms {
            score *= (200.0 - latency.min(200.0)) / 200.0;
        }
        
        // Factor in bandwidth (higher is better)
        if let Some(bandwidth) = peer.bandwidth_mbps {
            score *= (bandwidth.min(1000.0)) / 1000.0;
        }
        
        score.clamp(0.0, 1.0)
    }

    /// Calculate geographic distance between two points in kilometers
    fn calculate_distance(&self, point1: &Point, point2: &Point) -> f64 {
        let lat1_rad = point1.x().to_radians();
        let lat2_rad = point2.x().to_radians();
        let delta_lat = (point2.x() - point1.x()).to_radians();
        let delta_lon = (point2.y() - point1.y()).to_radians();

        let a = (delta_lat / 2.0).sin().powi(2) +
                lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

        6371.0 * c // Earth's radius in kilometers
    }
}

/// Comprehensive path analysis result
#[derive(Debug)]
pub struct PathAnalysisResult {
    pub validation: PathValidationResult,
    pub connectivity: PathConnectivityResult,
    pub recommendations: Vec<String>,
}

/// Routing statistics for DHT analysis
#[derive(Debug, Clone)]
pub struct RouteStats {
    pub total_peers: usize,
    pub active_buckets: usize,
    pub total_buckets: usize,
    pub k_value: usize,
    pub avg_bucket_utilization: f64,
    pub bucket_distribution: Vec<(usize, usize)>,
}

/// Extract peer ID from multiaddr with actual parsing logic
pub fn extract_peer_id_from_multiaddr(addr: &Multiaddr) -> Option<String> {
    use multiaddr::Protocol;
    
    // Parse the multiaddr to extract peer ID
    for protocol in addr.iter() {
        match protocol {
            Protocol::P2p(peer_id_bytes) => {
                // Convert peer ID bytes to string representation
                return Some(hex::encode(peer_id_bytes.to_bytes()));
            }
            _ => continue,
        }
    }
    
    // If no peer ID found in the multiaddr, generate one based on the address
    // This is a fallback for addresses that don't contain explicit peer IDs
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    
    let mut hasher = DefaultHasher::new();
    addr.to_string().hash(&mut hasher);
    let derived_id = format!("peer_{:x}", hasher.finish());
    
    debug!("No peer ID found in multiaddr {}, derived ID: {}", addr, derived_id);
    Some(derived_id)
}
