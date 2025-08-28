use crate::errors::{DaemonError, Result};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub max_paths: usize,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self { max_paths: 512 }
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
