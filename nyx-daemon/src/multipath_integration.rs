//! Multipath Integration
//!
//! Integrates nyx-stream's PathScheduler and ReorderingBuffer into daemon-level
//! session management. Provides:
//! - Runtime PathScheduler integration
//! - Path selection logic for outgoing packets
//! - Path health metrics collection
//! - Reordering buffer for out-of-order packets
//!
//! Design decisions:
//! - Per-connection multipath state
//! - Automatic path failover on quality degradation
//! - Metrics-driven path selection (RTT, loss, bandwidth)

#![forbid(unsafe_code)]

use nyx_stream::multipath_dataplane::{
    MultipathConfig, PathId, PathInfo, PathMetrics, PathScheduler, PathState,
    ReorderingBuffer,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Connection identifier
pub type ConnectionId = u32;

/// Multipath manager for daemon-level integration
pub struct MultipathManager {
    connections: Arc<RwLock<HashMap<ConnectionId, ConnectionMultipath>>>,
    config: MultipathConfig,
}

impl MultipathManager {
    pub fn new(config: MultipathConfig) -> Self {
        info!("MultipathManager initialized with config: {:?}", config);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Register connection for multipath support
    pub async fn register_connection(&self, conn_id: ConnectionId) {
        let mut conns = self.connections.write().await;
        let multipath = ConnectionMultipath::new(self.config.clone());
        conns.insert(conn_id, multipath);
        info!("Registered connection {} for multipath", conn_id);
    }

    /// Unregister connection
    pub async fn unregister_connection(&self, conn_id: ConnectionId) -> Result<(), MultipathError> {
        let mut conns = self.connections.write().await;
        
        if conns.remove(&conn_id).is_some() {
            info!("Unregistered connection {} from multipath", conn_id);
            Ok(())
        } else {
            Err(MultipathError::ConnectionNotFound)
        }
    }

    /// Add path to connection
    pub async fn add_path(
        &self,
        conn_id: ConnectionId,
        path_id: PathId,
        path_info: PathInfo,
    ) -> Result<(), MultipathError> {
        let mut conns = self.connections.write().await;
        
        let multipath = conns
            .get_mut(&conn_id)
            .ok_or(MultipathError::ConnectionNotFound)?;

        multipath.add_path(path_id, path_info)?;
        debug!("Added path {} to connection {}", path_id, conn_id);
        Ok(())
    }

    /// Remove path from connection
    pub async fn remove_path(
        &self,
        conn_id: ConnectionId,
        path_id: PathId,
    ) -> Result<(), MultipathError> {
        let mut conns = self.connections.write().await;
        
        let multipath = conns
            .get_mut(&conn_id)
            .ok_or(MultipathError::ConnectionNotFound)?;

        multipath.remove_path(path_id)?;
        debug!("Removed path {} from connection {}", path_id, conn_id);
        Ok(())
    }

    /// Select path for sending (core scheduler integration)
    pub async fn select_path(&self, conn_id: ConnectionId) -> Result<PathId, MultipathError> {
        let mut conns = self.connections.write().await;
        
        let multipath = conns
            .get_mut(&conn_id)
            .ok_or(MultipathError::ConnectionNotFound)?;

        multipath.select_path().ok_or(MultipathError::NoActivePath)
    }

    /// Update path metrics (RTT, jitter, loss rate)
    pub async fn update_path_metrics(
        &self,
        conn_id: ConnectionId,
        path_id: PathId,
        metrics: PathMetrics,
    ) -> Result<(), MultipathError> {
        let mut conns = self.connections.write().await;
        
        let multipath = conns
            .get_mut(&conn_id)
            .ok_or(MultipathError::ConnectionNotFound)?;

        multipath.update_metrics(path_id, metrics)?;
        Ok(())
    }

    /// Get path metrics for monitoring
    pub async fn get_path_metrics(
        &self,
        conn_id: ConnectionId,
        path_id: PathId,
    ) -> Option<PathMetrics> {
        let conns = self.connections.read().await;
        
        conns.get(&conn_id).and_then(|multipath| {
            multipath.scheduler.get_path_info(path_id).map(|info| info.metrics.clone())
        })
    }

    /// List all paths for connection
    pub async fn list_paths(&self, conn_id: ConnectionId) -> Vec<PathId> {
        let conns = self.connections.read().await;
        
        conns
            .get(&conn_id)
            .map(|multipath| multipath.scheduler.get_all_paths().keys().copied().collect())
            .unwrap_or_default()
    }

    /// Get reordering buffer status
    pub async fn get_reorder_status(&self, conn_id: ConnectionId) -> Option<ReorderStatus> {
        let conns = self.connections.read().await;
        
        conns.get(&conn_id).map(|multipath| {
            let (buffered, next_seq, timeout) = multipath.reorder_buffer.get_stats();
            ReorderStatus {
                buffered_packets: buffered,
                next_sequence: next_seq,
                timeout_ms: timeout.as_millis() as u64,
            }
        })
    }
}

/// Per-connection multipath state
struct ConnectionMultipath {
    scheduler: PathScheduler,
    reorder_buffer: ReorderingBuffer,
    config: MultipathConfig,
    next_sequence: u64,
    last_probe: Instant,
}

impl ConnectionMultipath {
    fn new(config: MultipathConfig) -> Self {
        Self {
            scheduler: PathScheduler::new(config.clone()),
            reorder_buffer: ReorderingBuffer::new(config.reorder_timeout_ms, 1000), // max 1000 packets
            config,
            next_sequence: 0,
            last_probe: Instant::now(),
        }
    }

    fn add_path(&mut self, _path_id: PathId, path_info: PathInfo) -> Result<(), MultipathError> {
        self.scheduler
            .add_path(path_info)
            .map_err(|e| MultipathError::SchedulerError(e.to_string()))
    }

    fn remove_path(&mut self, path_id: PathId) -> Result<(), MultipathError> {
        if self.scheduler.remove_path(path_id) {
            Ok(())
        } else {
            Err(MultipathError::PathNotFound)
        }
    }

    fn select_path(&mut self) -> Option<PathId> {
        // Probe paths if needed
        if self.last_probe.elapsed() >= Duration::from_millis(self.config.probe_interval_ms) {
            self.probe_paths();
            self.last_probe = Instant::now();
        }

        self.scheduler.select_path()
    }

    fn update_metrics(&mut self, path_id: PathId, metrics: PathMetrics) -> Result<(), MultipathError> {
        self.scheduler
            .update_path_metrics(path_id, metrics)
            .map_err(|e| MultipathError::SchedulerError(e.to_string()))
    }

    fn probe_paths(&mut self) {
        // Automatic path health check
        let timeout = Duration::from_millis(self.config.failover_timeout_ms);

        // Get all path IDs first to avoid mutable borrow conflicts
        let path_ids: Vec<PathId> = self.scheduler.get_all_paths().keys().copied().collect();

        for path_id in path_ids {
            if let Some(path_info) = self.scheduler.get_all_paths().get(&path_id) {
                let needs_degradation = path_info.last_activity.elapsed() > timeout
                    && matches!(path_info.state, PathState::Active);
                
                let needs_failure = path_info.metrics.quality < self.config.min_path_quality
                    && matches!(path_info.state, PathState::Active | PathState::Degraded);

                // Update metrics to trigger state change
                if needs_degradation || needs_failure {
                    let mut new_metrics = path_info.metrics.clone();
                    if needs_failure {
                        new_metrics.quality = 0.0; // Force quality check failure
                    }
                    let _ = self.scheduler.update_path_metrics(path_id, new_metrics);
                }
            }
        }
    }
}

/// Reordering buffer status
#[derive(Debug, Clone)]
pub struct ReorderStatus {
    pub buffered_packets: usize,
    pub next_sequence: u64,
    pub timeout_ms: u64,
}

/// Multipath errors
#[derive(Debug, thiserror::Error)]
pub enum MultipathError {
    #[error("Connection not found")]
    ConnectionNotFound,

    #[error("Path not found")]
    PathNotFound,

    #[error("No active path available")]
    NoActivePath,

    #[error("Scheduler error: {0}")]
    SchedulerError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_path_info(path_id: PathId) -> PathInfo {
        PathInfo {
            path_id,
            connection_id: 1,
            state: PathState::Active,
            weight: 1.0,
            metrics: PathMetrics {
                rtt_ms: 50.0,
                jitter_ms: 5.0,
                loss_rate: 0.01,
                bandwidth_mbps: 100.0,
                quality: 0.9,
                hop_count: 3,
                last_measurement: Instant::now(),
                failed_probes: 0,
            },
            created_at: Instant::now(),
            last_activity: Instant::now(),
        }
    }

    #[tokio::test]
    async fn test_connection_lifecycle() {
        let manager = MultipathManager::new(MultipathConfig::default());
        let conn_id = 1;

        // Register connection
        manager.register_connection(conn_id).await;

        // Add path
        let path_id = 0;
        manager
            .add_path(conn_id, path_id, create_test_path_info(path_id))
            .await
            .unwrap();

        // List paths
        let paths = manager.list_paths(conn_id).await;
        assert_eq!(paths.len(), 1);
        assert!(paths.contains(&path_id));

        // Unregister connection
        manager.unregister_connection(conn_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_path_selection() {
        let manager = MultipathManager::new(MultipathConfig::default());
        let conn_id = 1;

        manager.register_connection(conn_id).await;

        // Add multiple paths
        manager
            .add_path(conn_id, 0, create_test_path_info(0))
            .await
            .unwrap();
        manager
            .add_path(conn_id, 1, create_test_path_info(1))
            .await
            .unwrap();

        // Select path (should succeed)
        let selected = manager.select_path(conn_id).await.unwrap();
        assert!(selected == 0 || selected == 1);
    }

    #[tokio::test]
    async fn test_metrics_update() {
        let manager = MultipathManager::new(MultipathConfig::default());
        let conn_id = 1;
        let path_id = 0;

        manager.register_connection(conn_id).await;
        manager
            .add_path(conn_id, path_id, create_test_path_info(path_id))
            .await
            .unwrap();

        // Update metrics
        let new_metrics = PathMetrics {
            rtt_ms: 100.0,
            jitter_ms: 10.0,
            loss_rate: 0.02,
            bandwidth_mbps: 50.0,
            quality: 0.8,
            hop_count: 4,
            last_measurement: Instant::now(),
            failed_probes: 0,
        };

        manager
            .update_path_metrics(conn_id, path_id, new_metrics.clone())
            .await
            .unwrap();

        // Verify update
        let metrics = manager.get_path_metrics(conn_id, path_id).await.unwrap();
        assert_eq!(metrics.rtt_ms, 100.0);
    }

    #[tokio::test]
    async fn test_path_removal() {
        let manager = MultipathManager::new(MultipathConfig::default());
        let conn_id = 1;
        let path_id = 0;

        manager.register_connection(conn_id).await;
        manager
            .add_path(conn_id, path_id, create_test_path_info(path_id))
            .await
            .unwrap();

        // Remove path
        manager.remove_path(conn_id, path_id).await.unwrap();

        // Should not be listed
        let paths = manager.list_paths(conn_id).await;
        assert_eq!(paths.len(), 0);
    }

    #[tokio::test]
    async fn test_no_active_path() {
        let manager = MultipathManager::new(MultipathConfig::default());
        let conn_id = 1;

        manager.register_connection(conn_id).await;

        // Try to select path without any paths
        let result = manager.select_path(conn_id).await;
        assert!(matches!(result, Err(MultipathError::NoActivePath)));
    }

    #[tokio::test]
    async fn test_reorder_status() {
        let manager = MultipathManager::new(MultipathConfig::default());
        let conn_id = 1;

        manager.register_connection(conn_id).await;

        let status = manager.get_reorder_status(conn_id).await.unwrap();
        assert_eq!(status.buffered_packets, 0);
        assert_eq!(status.next_sequence, 0);
    }
}
