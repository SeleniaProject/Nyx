//! Path Recovery Module - Debugging and Recovery Utilities
//! Provides tools for diagnosing and recovering from path building failures
//! This module helps identify common issues and provides automated recovery mechanisms

use crate::errors::Result;
use crate::path_builder::PathBuilder;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

// Note: This DaemonConfig here is a local placeholder and independent of the one in path_builder.
#[derive(Debug, Clone, Default)]
pub struct DaemonConfig {
    pub max_paths: usize,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum PathFailureReason {
    NetworkUnreachable,
    ConnectionTimeout,
    HandshakeFailure,
    InsufficientBandwidth,
    HighLatency,
    PacketLoss,
    AuthenticationError,
    ProtocolMismatch,
    ResourceExhaustion,
    Unknown(String),
}

#[derive(Debug, Clone)]
pub struct PathFailureRecord {
    pub path_id: String,
    pub target_addr: SocketAddr,
    pub failure_reason: PathFailureReason,
    pub failure_time: SystemTime,
    pub retry_count: u32,
    pub last_working_time: Option<SystemTime>,
    pub error_details: String,
}

#[derive(Debug, Clone)]
pub struct PathRecoveryConfig {
    /// Maximum number of retry attempts
    pub max_retry_attempts: u32,
    /// Base delay between retry attempts
    pub base_retry_delay: Duration,
    /// Maximum retry delay (exponential backoff cap)
    pub max_retry_delay: Duration,
    /// Time to wait before marking a path as permanently failed
    pub permanent_failure_timeout: Duration,
    /// Minimum interval between recovery attempts
    pub recovery_interval: Duration,
    /// Enable automatic recovery
    pub auto_recovery_enabled: bool,
}

impl Default for PathRecoveryConfig {
    fn default() -> Self {
        Self {
            max_retry_attempts: 5,
            base_retry_delay: Duration::from_secs(1),
            max_retry_delay: Duration::from_secs(60),
            permanent_failure_timeout: Duration::from_secs(300),
            recovery_interval: Duration::from_secs(30),
            auto_recovery_enabled: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PathDiagnostics {
    pub connectivity_score: f64, // 0.0 - 1.0
    pub reliability_score: f64,  // 0.0 - 1.0
    pub performance_score: f64,  // 0.0 - 1.0
    pub overall_health: f64,     // 0.0 - 1.0
    pub issue_count: u32,
    pub last_diagnosis: SystemTime,
    pub recommended_actions: Vec<String>,
}

/// Path Recovery Manager for handling broken paths
#[derive(Debug)]
pub struct PathRecoveryManager {
    config: PathRecoveryConfig,
    path_builder: Arc<PathBuilder>,
    failure_records: Arc<RwLock<HashMap<String, PathFailureRecord>>>,
    recovery_tasks: Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    diagnostics: Arc<RwLock<HashMap<String, PathDiagnostics>>>,
}

impl PathRecoveryManager {
    pub fn new(config: PathRecoveryConfig, path_builder: Arc<PathBuilder>) -> Self {
        Self {
            config,
            path_builder,
            failure_records: Arc::new(RwLock::new(HashMap::new())),
            recovery_tasks: Arc::new(Mutex::new(HashMap::new())),
            diagnostics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the recovery manager
    pub async fn start(&self) -> Result<()> {
        if self.config.auto_recovery_enabled {
            let recovery_manager = self.clone();
            tokio::spawn(async move {
                recovery_manager.run_recovery_loop().await;
            });
        }

        info!("Path recovery manager started");
        Ok(())
    }

    /// Record a path failure
    pub async fn record_failure(
        &self,
        path_id: String,
        target_addr: SocketAddr,
        failure_reason: PathFailureReason,
        error_details: String,
    ) -> Result<()> {
        let mut failure_records = self.failure_records.write().await;

        let record = if let Some(existing) = failure_records.get(&path_id) {
            PathFailureRecord {
                retry_count: existing.retry_count + 1,
                failure_time: SystemTime::now(),
                failure_reason: failure_reason.clone(),
                error_details,
                ..existing.clone()
            }
        } else {
            PathFailureRecord {
                path_id: path_id.clone(),
                target_addr,
                failure_reason: failure_reason.clone(),
                failure_time: SystemTime::now(),
                retry_count: 1,
                last_working_time: None,
                error_details,
            }
        };

        failure_records.insert(path_id.clone(), record);

        warn!("Recorded path failure: {} -> {:?}", path_id, failure_reason);

        // Update diagnostics
        self.update_path_diagnostics(&path_id).await?;

        Ok(())
    }

    /// Attempt to recover a failed path
    pub async fn attempt_recovery(&self, path_id: &str) -> Result<bool> {
        let failure_record = {
            let records = self.failure_records.read().await;
            records.get(path_id).cloned()
        };

        let record = match failure_record {
            Some(record) => record,
            None => {
                debug!("No failure record found for path {}", path_id);
                return Ok(false);
            }
        };

        // Check if we've exceeded retry attempts
        if record.retry_count >= self.config.max_retry_attempts {
            warn!("Path {} has exceeded maximum retry attempts", path_id);
            return Ok(false);
        }

        // Check if it's too soon to retry
        let retry_delay = self.calculate_retry_delay(record.retry_count);
        if record
            .failure_time
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
            < retry_delay
        {
            debug!(
                "Too soon to retry path {}, waiting {:?}",
                path_id, retry_delay
            );
            return Ok(false);
        }

        info!(
            "Attempting recovery for path {} (attempt {})",
            path_id,
            record.retry_count + 1
        );

        // Perform recovery based on failure reason
        let recovery_result = match record.failure_reason {
            PathFailureReason::NetworkUnreachable => {
                self.recover_network_unreachable(&record).await
            }
            PathFailureReason::ConnectionTimeout => self.recover_connection_timeout(&record).await,
            PathFailureReason::HandshakeFailure => self.recover_handshake_failure(&record).await,
            PathFailureReason::InsufficientBandwidth => {
                self.recover_insufficient_bandwidth(&record).await
            }
            PathFailureReason::HighLatency => self.recover_high_latency(&record).await,
            PathFailureReason::PacketLoss => self.recover_packet_loss(&record).await,
            PathFailureReason::AuthenticationError => {
                self.recover_authentication_error(&record).await
            }
            PathFailureReason::ProtocolMismatch => self.recover_protocol_mismatch(&record).await,
            PathFailureReason::ResourceExhaustion => {
                self.recover_resource_exhaustion(&record).await
            }
            PathFailureReason::Unknown(_) => self.recover_unknown_failure(&record).await,
        };

        match recovery_result {
            Ok(true) => {
                info!("Successfully recovered path {}", path_id);
                // Remove from failure records
                {
                    let mut records = self.failure_records.write().await;
                    records.remove(path_id);
                }
                self.update_path_diagnostics(path_id).await?;
                Ok(true)
            }
            Ok(false) => {
                warn!("Recovery attempt failed for path {}", path_id);
                self.record_recovery_failure(path_id).await?;
                Ok(false)
            }
            Err(e) => {
                error!("Recovery error for path {}: {}", path_id, e);
                self.record_recovery_failure(path_id).await?;
                Err(e)
            }
        }
    }

    /// Calculate exponential backoff retry delay
    fn calculate_retry_delay(&self, retry_count: u32) -> Duration {
        let delay_ms = self.config.base_retry_delay.as_millis() as u64 * (2_u64.pow(retry_count));
        let delay = Duration::from_millis(delay_ms);

        if delay > self.config.max_retry_delay {
            self.config.max_retry_delay
        } else {
            delay
        }
    }

    /// Recovery strategies for different failure types
    async fn recover_network_unreachable(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!(
            "Attempting network unreachable recovery for {}",
            record.path_id
        );

        // Try to rebuild the path with alternative routes
        match self
            .path_builder
            .rebuild_path_with_alternatives(&record.path_id, record.target_addr)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => {
                // Try to find completely new path
                match self.path_builder.build_new_path(record.target_addr).await {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
        }
    }

    async fn recover_connection_timeout(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!(
            "Attempting connection timeout recovery for {}",
            record.path_id
        );

        // Increase timeout and retry
        match self
            .path_builder
            .rebuild_path_with_extended_timeout(&record.path_id, Duration::from_secs(10))
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn recover_handshake_failure(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!(
            "Attempting handshake failure recovery for {}",
            record.path_id
        );

        // Reset crypto state and retry
        match self.path_builder.reset_crypto_state(&record.path_id).await {
            Ok(_) => {
                // Retry handshake
                match self.path_builder.retry_handshake(&record.path_id).await {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            Err(_) => Ok(false),
        }
    }

    async fn recover_insufficient_bandwidth(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!("Attempting bandwidth recovery for {}", record.path_id);

        // Try to find a higher bandwidth path
        match self
            .path_builder
            .find_high_bandwidth_path(record.target_addr)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn recover_high_latency(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!("Attempting latency recovery for {}", record.path_id);

        // Try to find a lower latency path
        match self
            .path_builder
            .find_low_latency_path(record.target_addr)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn recover_packet_loss(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!("Attempting packet loss recovery for {}", record.path_id);

        // Enable redundancy or find more reliable path
        match self
            .path_builder
            .enable_path_redundancy(&record.path_id)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => {
                match self
                    .path_builder
                    .find_reliable_path(record.target_addr)
                    .await
                {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
        }
    }

    async fn recover_authentication_error(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!("Attempting authentication recovery for {}", record.path_id);

        // Refresh credentials and retry
        match self.path_builder.refresh_credentials(&record.path_id).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn recover_protocol_mismatch(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!(
            "Attempting protocol mismatch recovery for {}",
            record.path_id
        );

        // Try with different protocol version
        match self
            .path_builder
            .retry_with_fallback_protocol(&record.path_id)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn recover_resource_exhaustion(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!(
            "Attempting resource exhaustion recovery for {}",
            record.path_id
        );

        // Wait and then retry
        tokio::time::sleep(Duration::from_secs(5)).await;
        match self.path_builder.retry_path_build(&record.path_id).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    async fn recover_unknown_failure(&self, record: &PathFailureRecord) -> Result<bool> {
        debug!("Attempting unknown failure recovery for {}", record.path_id);

        // Generic retry approach
        match self
            .path_builder
            .generic_path_recovery(&record.path_id)
            .await
        {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Update path diagnostics
    async fn update_path_diagnostics(&self, path_id: &str) -> Result<()> {
        let diagnostics = self.calculate_path_diagnostics(path_id).await?;

        let mut diag_map = self.diagnostics.write().await;
        diag_map.insert(path_id.to_string(), diagnostics);

        Ok(())
    }

    /// Calculate comprehensive path diagnostics
    async fn calculate_path_diagnostics(&self, path_id: &str) -> Result<PathDiagnostics> {
        let failure_records = self.failure_records.read().await;
        let record = failure_records.get(path_id);

        let (connectivity_score, reliability_score, issue_count) = if let Some(record) = record {
            let connectivity =
                1.0 - (record.retry_count as f64 / self.config.max_retry_attempts as f64).min(1.0);
            let reliability = if record.last_working_time.is_some() {
                0.7
            } else {
                0.3
            };
            (connectivity, reliability, record.retry_count)
        } else {
            (1.0, 1.0, 0)
        };

        // Get performance metrics from path builder if available
        let performance_score = match self.path_builder.get_path_quality(path_id).await {
            Ok(quality) => quality.overall_score(),
            Err(_) => 0.5, // Default score
        };

        let overall_health = (connectivity_score + reliability_score + performance_score) / 3.0;

        let mut recommended_actions = Vec::new();
        if connectivity_score < 0.5 {
            recommended_actions.push("Check network connectivity".to_string());
        }
        if reliability_score < 0.5 {
            recommended_actions.push("Consider path redundancy".to_string());
        }
        if performance_score < 0.5 {
            recommended_actions.push("Find alternative routes".to_string());
        }

        Ok(PathDiagnostics {
            connectivity_score,
            reliability_score,
            performance_score,
            overall_health,
            issue_count,
            last_diagnosis: SystemTime::now(),
            recommended_actions,
        })
    }

    /// Record a recovery failure
    async fn record_recovery_failure(&self, path_id: &str) -> Result<()> {
        let mut failure_records = self.failure_records.write().await;
        if let Some(record) = failure_records.get_mut(path_id) {
            record.failure_time = SystemTime::now();
        }
        Ok(())
    }

    /// Main recovery loop
    async fn run_recovery_loop(&self) {
        let mut interval = tokio::time::interval(self.config.recovery_interval);

        loop {
            interval.tick().await;

            let failed_paths: Vec<String> = {
                let records = self.failure_records.read().await;
                records.keys().cloned().collect()
            };

            for path_id in failed_paths {
                if let Err(e) = self.attempt_recovery(&path_id).await {
                    error!("Recovery attempt failed for path {}: {}", path_id, e);
                }
            }
        }
    }

    /// Get failure statistics
    pub async fn get_failure_statistics(&self) -> HashMap<PathFailureReason, u32> {
        let records = self.failure_records.read().await;
        let mut stats = HashMap::new();

        for record in records.values() {
            let reason = record.failure_reason.clone();
            *stats.entry(reason).or_insert(0) += 1;
        }

        stats
    }

    /// Get all failed paths
    pub async fn get_failed_paths(&self) -> Vec<PathFailureRecord> {
        let records = self.failure_records.read().await;
        records.values().cloned().collect()
    }

    /// Get path diagnostics
    pub async fn get_path_diagnostics(&self, path_id: &str) -> Option<PathDiagnostics> {
        let diagnostics = self.diagnostics.read().await;
        diagnostics.get(path_id).cloned()
    }

    /// Clear failure history
    pub async fn clear_failure_history(&self) {
        let mut records = self.failure_records.write().await;
        records.clear();

        let mut diagnostics = self.diagnostics.write().await;
        diagnostics.clear();

        info!("Cleared path failure history");
    }
}

impl Clone for PathRecoveryManager {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            path_builder: self.path_builder.clone(),
            failure_records: self.failure_records.clone(),
            recovery_tasks: self.recovery_tasks.clone(),
            diagnostics: self.diagnostics.clone(),
        }
    }
}
