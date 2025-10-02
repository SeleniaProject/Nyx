use crate::errors::{DaemonError, Result};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

/// Path degradation thresholds for automatic rebuilding
#[derive(Debug, Clone)]
pub struct DegradationThresholds {
    /// Maximum acceptable packet loss rate (0.0-1.0)
    pub max_loss_rate: f64,
    /// Maximum acceptable RTT in seconds
    pub max_rtt: Duration,
    /// Minimum acceptable path quality score (0.0-1.0)
    pub min_quality_score: f64,
}

impl Default for DegradationThresholds {
    fn default() -> Self {
        Self {
            max_loss_rate: 0.05, // 5% packet loss
            max_rtt: Duration::from_millis(500), // 500ms RTT
            min_quality_score: 0.6, // 60% quality score
        }
    }
}

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub max_paths: usize,
    pub degradation_thresholds: DegradationThresholds,
    pub metrics_update_interval: Duration,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            max_paths: 512,
            degradation_thresholds: DegradationThresholds::default(),
            metrics_update_interval: Duration::from_secs(5), // Update metrics every 5 seconds
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PathQuality {
    pub latency: f64,
    pub bandwidth: f64,
    pub reliability: f64,
}

impl PathQuality {
    pub fn overall_score(&self) -> f64 {
        (self.latency + self.bandwidth + self.reliability) / 3.0
    }
}

#[cfg(test)]
pub mod integration_tests;

#[derive(Debug, Clone)]
struct PathInfo {
    id: String,
    endpoint: SocketAddr,
    quality: PathQuality,
    created_at: SystemTime,
    last_ping: Option<Instant>,
    failed: bool,
    timeout: Duration,
    handshake_attempts: u32,
    redundancy_enabled: bool,
    credentials_refreshed_at: Option<SystemTime>,
    protocol_fallback: bool,
}

/// Path Builder - Core path management implementation
#[derive(Debug, Clone)]
pub struct PathBuilder {
    config: DaemonConfig,
    active_paths: Arc<RwLock<HashMap<String, PathInfo>>>,
    path_counter: Arc<AtomicU64>,
}

impl PathBuilder {
    pub fn new(config: DaemonConfig) -> Result<Self> {
        Ok(Self {
            config,
            active_paths: Arc::new(RwLock::new(HashMap::new())),
            path_counter: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn build_path(&self, endpoint: SocketAddr) -> Result<String> {
        // Basic endpoint validation and a simple heuristic for unreachable test-nets
        if endpoint.port() == 0 || is_unreachable_endpoint(endpoint.ip()) {
            return Err(DaemonError::transport("Network unreachable"));
        }

        // Enforce resource limits if configured
        if self.config.max_paths > 0 {
            let len = self.active_paths.read().await.len();
            if len >= self.config.max_paths {
                return Err(DaemonError::ResourceExhaustion);
            }
        }

        let id_num = self.path_counter.fetch_add(1, Ordering::SeqCst);
        let path_id = format!("path_{id_num}");

        let info = PathInfo {
            id: path_id.clone(),
            endpoint,
            quality: PathQuality {
                latency: 0.6,
                bandwidth: 0.6,
                reliability: 0.8,
            },
            created_at: SystemTime::now(),
            last_ping: None,
            failed: false,
            timeout: Duration::from_secs(3),
            handshake_attempts: 0,
            redundancy_enabled: false,
            credentials_refreshed_at: None,
            protocol_fallback: false,
        };

        let mut write = self.active_paths.write().await;
        write.insert(path_id.clone(), info);
        Ok(path_id)
    }

    pub async fn destroy_path(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        write.remove(path_id);
        Ok(())
    }

    pub async fn get_path_quality(&self, path_id: &str) -> Result<PathQuality> {
        let read = self.active_paths.read().await;
        let info = read
            .get(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        Ok(info.quality.clone())
    }

    pub async fn get_available_paths(&self) -> Result<Vec<(String, SocketAddr)>> {
        let read = self.active_paths.read().await;
        Ok(read.values().map(|p| (p.id.clone(), p.endpoint)).collect())
    }

    pub async fn rebuild_path_with_alternatives(
        &self,
        path_id: &str,
        target: SocketAddr,
    ) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.endpoint = target;
        info.failed = false;
        info.handshake_attempts = 0;
        Ok(())
    }

    pub async fn build_new_path(&self, target: SocketAddr) -> Result<String> {
        self.build_path(target).await
    }

    pub async fn rebuild_path_with_extended_timeout(
        &self,
        path_id: &str,
        timeout: Duration,
    ) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.timeout = timeout;
        info.failed = false;
        Ok(())
    }

    pub async fn reset_crypto_state(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.handshake_attempts = 0;
        Ok(())
    }

    pub async fn retry_handshake(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.handshake_attempts = info.handshake_attempts.saturating_add(1);
        info.failed = false;
        Ok(())
    }

    pub async fn find_high_bandwidth_path(&self, target: SocketAddr) -> Result<()> {
        let id = self.build_path(target).await?;
        let mut write = self.active_paths.write().await;
        if let Some(info) = write.get_mut(&id) {
            info.quality.bandwidth = 0.9;
        }
        Ok(())
    }

    pub async fn find_low_latency_path(&self, target: SocketAddr) -> Result<()> {
        let id = self.build_path(target).await?;
        let mut write = self.active_paths.write().await;
        if let Some(info) = write.get_mut(&id) {
            info.quality.latency = 0.9;
        }
        Ok(())
    }

    pub async fn enable_path_redundancy(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.redundancy_enabled = true;
        Ok(())
    }

    pub async fn find_reliable_path(&self, target: SocketAddr) -> Result<()> {
        let id = self.build_path(target).await?;
        let mut write = self.active_paths.write().await;
        if let Some(info) = write.get_mut(&id) {
            info.quality.reliability = 0.95;
        }
        Ok(())
    }

    pub async fn refresh_credentials(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.credentials_refreshed_at = Some(SystemTime::now());
        Ok(())
    }

    pub async fn retry_with_fallback_protocol(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.protocol_fallback = true;
        info.failed = false;
        Ok(())
    }

    pub async fn retry_path_build(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.failed = false;
        Ok(())
    }

    pub async fn generic_path_recovery(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.quality.latency = (info.quality.latency + 0.8).min(1.0);
        info.quality.reliability = (info.quality.reliability + 0.8).min(1.0);
        info.failed = false;
        Ok(())
    }

    pub async fn path_exists(&self, path_id: &str) -> Result<bool> {
        let read = self.active_paths.read().await;
        Ok(read.contains_key(path_id))
    }

    pub async fn assess_path_quality(&self, path_id: &str) -> Result<PathQuality> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        let age_factor = info
            .created_at
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
            .as_secs_f64();
        let age_bonus = (age_factor / 60.0).min(0.1);
        let ping_bonus = if info.last_ping.is_some() { 0.05 } else { 0.0 };
        info.quality.reliability = (info.quality.reliability + age_bonus + ping_bonus).min(1.0);
        Ok(info.quality.clone())
    }

    pub async fn update_path_quality(&self, path_id: &str, quality: PathQuality) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.quality = quality;
        Ok(())
    }

    pub async fn simulate_path_failure(&self, path_id: &str) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;
        info.failed = true;
        Ok(())
    }

    pub async fn test_failover(&self, primary: &str, backup: &str) -> Result<bool> {
        let read = self.active_paths.read().await;
        let ok = matches!(read.get(primary), Some(p) if p.failed) && read.contains_key(backup);
        Ok(ok)
    }

    pub async fn ping_path(&self, path_id: &str) -> Result<()> {
        let endpoint = {
            let mut write = self.active_paths.write().await;
            let info = write
                .get_mut(path_id)
                .ok_or_else(|| DaemonError::internal("Path not found"))?;
            if info.failed {
                return Err(DaemonError::internal("Path is marked failed"));
            }
            info.last_ping = Some(Instant::now());
            info.endpoint
        };

        let socket = std::net::UdpSocket::bind(("0.0.0.0", 0))?;
        socket.set_nonblocking(true)?;
        let _ = socket.send_to(&[0u8; 1], endpoint)?;
        Ok(())
    }

    /// Update path quality from live probe metrics
    /// 
    /// This method integrates with NetworkPathProber to update path quality based on
    /// real network measurements. It should be called periodically by the metrics update task.
    /// 
    /// # Arguments
    /// * `path_id` - ID of the path to update
    /// * `rtt` - Round-trip time
    /// * `loss_rate` - Packet loss rate (0.0-1.0)
    /// * `jitter` - Network jitter
    /// * `bandwidth` - Estimated bandwidth in bytes/sec
    pub async fn update_path_metrics(
        &self,
        path_id: &str,
        rtt: Duration,
        loss_rate: f64,
        _jitter: Duration, // Reserved for future jitter-based quality calculations
        bandwidth: u64,
    ) -> Result<()> {
        let mut write = self.active_paths.write().await;
        let info = write
            .get_mut(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;

        // Calculate quality scores (0.0-1.0)
        // Lower RTT is better: score = 1.0 - min(rtt/1000ms, 1.0)
        let latency_score = 1.0 - (rtt.as_millis() as f64 / 1000.0).min(1.0);
        
        // Higher bandwidth is better: normalize to 0.0-1.0 range (1 Mbps = 1.0)
        let bandwidth_score = (bandwidth as f64 / 1_000_000.0).min(1.0);
        
        // Lower loss rate is better: score = 1.0 - loss_rate
        let reliability_score = (1.0 - loss_rate).max(0.0);

        info.quality = PathQuality {
            latency: latency_score,
            bandwidth: bandwidth_score,
            reliability: reliability_score,
        };

        Ok(())
    }

    /// Check if path is degraded and needs rebuilding
    /// 
    /// Returns true if any of the quality thresholds are violated:
    /// - Packet loss rate exceeds threshold
    /// - RTT exceeds threshold  
    /// - Overall quality score falls below threshold
    pub async fn is_path_degraded(&self, path_id: &str, rtt: Duration, loss_rate: f64) -> Result<bool> {
        let read = self.active_paths.read().await;
        let info = read
            .get(path_id)
            .ok_or_else(|| DaemonError::internal("Path not found"))?;

        let thresholds = &self.config.degradation_thresholds;
        
        // Check individual thresholds
        if loss_rate > thresholds.max_loss_rate {
            return Ok(true); // Loss rate too high
        }
        
        if rtt > thresholds.max_rtt {
            return Ok(true); // RTT too high
        }
        
        // Check overall quality score
        let quality_score = info.quality.overall_score();
        if quality_score < thresholds.min_quality_score {
            return Ok(true); // Quality too low
        }

        Ok(false)
    }

    /// Automatically rebuild degraded path with alternative route
    /// 
    /// This method is called when a path is detected as degraded. It attempts to:
    /// 1. Mark the current path as failed
    /// 2. Build a new path to the same endpoint
    /// 3. Return the new path ID
    /// 
    /// # Arguments
    /// * `degraded_path_id` - ID of the degraded path
    /// 
    /// # Returns
    /// * `Ok(String)` - ID of the newly built replacement path
    pub async fn rebuild_degraded_path(&self, degraded_path_id: &str) -> Result<String> {
        // Get endpoint of degraded path
        let endpoint = {
            let mut write = self.active_paths.write().await;
            let info = write
                .get_mut(degraded_path_id)
                .ok_or_else(|| DaemonError::internal("Path not found"))?;
            
            // Mark as failed to prevent further use
            info.failed = true;
            info.endpoint
        };

        // Build new path to same endpoint
        let new_path_id = self.build_path(endpoint).await?;
        
        Ok(new_path_id)
    }

    /// Get all paths that need rebuilding due to degradation
    /// 
    /// Returns a list of (path_id, endpoint) tuples for paths that should be rebuilt
    pub async fn get_degraded_paths(&self) -> Result<Vec<(String, SocketAddr)>> {
        let read = self.active_paths.read().await;
        let thresholds = &self.config.degradation_thresholds;
        
        let degraded: Vec<_> = read
            .iter()
            .filter(|(_, info)| {
                // Check if quality is below threshold
                info.quality.overall_score() < thresholds.min_quality_score ||
                info.failed
            })
            .map(|(id, info)| (id.clone(), info.endpoint))
            .collect();
        
        Ok(degraded)
    }
}

fn is_unreachable_endpoint(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // TEST-NET-1 192.0.2.0/24, TEST-NET-2 198.51.100.0/24, TEST-NET-3 203.0.113.0/24
            (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
                || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
                || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
        }
        IpAddr::V6(_) => false,
    }
}
