//! HPKE Rekey Scheduler
//!
//! Manages automatic key rotation based on data volume or time thresholds.
//! Triggers rekey when 1GB of data has been transferred OR 10 minutes have elapsed.
//! 
//! # Security Properties
//! - Forward secrecy through key rotation
//! - Atomic key transitions to prevent data loss
//! - Coordinated rekey between endpoints
//! - Anti-replay window reset on rekey
//!
//! # Telemetry
//! - Exposes `nyx.stream.rekey.count` counter for total rekey operations
//! - Tracks rekey failure rate via `nyx.stream.rekey.failures` counter
//! - Provides detailed metrics via RekeyMetrics structure

#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time;
use tracing::{debug, info, warn};

/// Rekey trigger thresholds
const REKEY_BYTES_THRESHOLD: u64 = 1_000_000_000; // 1 GB
const REKEY_TIME_THRESHOLD: Duration = Duration::from_secs(600); // 10 minutes

/// Rekey scheduler state
#[derive(Debug)]
pub struct RekeyScheduler {
    /// Bytes transferred since last rekey
    bytes_transferred: Arc<RwLock<u64>>,
    
    /// Time of last rekey
    last_rekey: Arc<RwLock<Instant>>,
    
    /// Configuration
    config: RekeyConfig,
    
    /// Metrics
    metrics: Arc<RwLock<RekeyMetrics>>,
}

/// Rekey configuration
#[derive(Debug, Clone)]
pub struct RekeyConfig {
    /// Bytes threshold for rekey trigger (default: 1GB)
    pub bytes_threshold: u64,
    
    /// Time threshold for rekey trigger (default: 10 min)
    pub time_threshold: Duration,
    
    /// Enable automatic rekey
    pub enabled: bool,
}

impl Default for RekeyConfig {
    fn default() -> Self {
        Self {
            bytes_threshold: REKEY_BYTES_THRESHOLD,
            time_threshold: REKEY_TIME_THRESHOLD,
            enabled: true,
        }
    }
}

/// Rekey metrics for telemetry
#[derive(Debug, Clone, Default)]
pub struct RekeyMetrics {
    /// Total rekey operations completed
    pub total_rekeys: u64,
    
    /// Rekeys triggered by bytes threshold
    pub rekeys_by_bytes: u64,
    
    /// Rekeys triggered by time threshold
    pub rekeys_by_time: u64,
    
    /// Failed rekey attempts
    pub rekey_failures: u64,
    
    /// Total bytes transferred across all rekeys
    pub total_bytes_transferred: u64,
    
    /// Average bytes per rekey interval
    pub avg_bytes_per_interval: u64,
}

impl RekeyScheduler {
    /// Create a new rekey scheduler with default configuration
    pub fn new() -> Self {
        Self::with_config(RekeyConfig::default())
    }
    
    /// Create a new rekey scheduler with custom configuration
    pub fn with_config(config: RekeyConfig) -> Self {
        Self {
            bytes_transferred: Arc::new(RwLock::new(0)),
            last_rekey: Arc::new(RwLock::new(Instant::now())),
            config,
            metrics: Arc::new(RwLock::new(RekeyMetrics::default())),
        }
    }
    
    /// Record bytes transferred
    pub async fn record_bytes(&self, bytes: u64) {
        let mut transferred = self.bytes_transferred.write().await;
        *transferred += bytes;
    }
    
    /// Check if rekey should be triggered
    pub async fn should_rekey(&self) -> (bool, RekeyTrigger) {
        if !self.config.enabled {
            return (false, RekeyTrigger::None);
        }
        
        let bytes = *self.bytes_transferred.read().await;
        let last = *self.last_rekey.read().await;
        let elapsed = last.elapsed();
        
        // Check bytes threshold first (more critical)
        if bytes >= self.config.bytes_threshold {
            debug!("Rekey triggered by bytes: {} >= {}", bytes, self.config.bytes_threshold);
            return (true, RekeyTrigger::Bytes(bytes));
        }
        
        // Check time threshold
        if elapsed >= self.config.time_threshold {
            debug!("Rekey triggered by time: {:?} >= {:?}", elapsed, self.config.time_threshold);
            return (true, RekeyTrigger::Time(elapsed));
        }
        
        (false, RekeyTrigger::None)
    }
    
    /// Mark rekey as completed and reset counters
    pub async fn complete_rekey(&self, trigger: RekeyTrigger) {
        let mut transferred = self.bytes_transferred.write().await;
        let mut last = self.last_rekey.write().await;
        let mut metrics = self.metrics.write().await;
        
        // Update metrics
        metrics.total_rekeys += 1;
        metrics.total_bytes_transferred += *transferred;
        
        match trigger {
            RekeyTrigger::Bytes(bytes) => {
                metrics.rekeys_by_bytes += 1;
                info!("Rekey completed (bytes trigger): {} bytes transferred", bytes);
            }
            RekeyTrigger::Time(elapsed) => {
                metrics.rekeys_by_time += 1;
                info!("Rekey completed (time trigger): {:?} elapsed", elapsed);
            }
            RekeyTrigger::None => {
                warn!("Rekey completed with no trigger");
            }
        }
        
        // Recalculate average
        if metrics.total_rekeys > 0 {
            metrics.avg_bytes_per_interval = 
                metrics.total_bytes_transferred / metrics.total_rekeys;
        }
        
        // Record rekey count to telemetry
        #[cfg(feature = "telemetry")]
        nyx_telemetry::metrics::record_counter("nyx.stream.rekey.count", 1);
        
        // Reset counters
        *transferred = 0;
        *last = Instant::now();
    }
    
    /// Record a failed rekey attempt
    pub async fn record_failure(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.rekey_failures += 1;
        
        // Record failure to telemetry
        #[cfg(feature = "telemetry")]
        nyx_telemetry::metrics::record_counter("nyx.stream.rekey.failures", 1);
        
        warn!("Rekey failure recorded (total failures: {})", metrics.rekey_failures);
    }
    
    /// Get current metrics
    pub async fn metrics(&self) -> RekeyMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Get current bytes transferred
    pub async fn bytes_transferred(&self) -> u64 {
        *self.bytes_transferred.read().await
    }
    
    /// Get time since last rekey
    pub async fn time_since_rekey(&self) -> Duration {
        self.last_rekey.read().await.elapsed()
    }
    
    /// Start automatic rekey monitoring task
    ///
    /// This spawns a background task that periodically checks rekey conditions
    /// and invokes the provided callback when rekey is needed.
    pub async fn start_monitoring<F>(
        self: Arc<Self>,
        mut rekey_callback: F,
    ) -> tokio::task::JoinHandle<()>
    where
        F: FnMut(RekeyTrigger) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
            + Send
            + 'static,
    {
        tokio::spawn(async move {
            let check_interval = Duration::from_secs(10); // Check every 10 seconds
            let mut interval = time::interval(check_interval);
            
            loop {
                interval.tick().await;
                
                let (should_rekey, trigger) = self.should_rekey().await;
                if should_rekey {
                    info!("Initiating rekey: {:?}", trigger);
                    
                    match rekey_callback(trigger).await {
                        Ok(()) => {
                            self.complete_rekey(trigger).await;
                        }
                        Err(e) => {
                            warn!("Rekey callback failed: {}", e);
                            self.record_failure().await;
                        }
                    }
                }
            }
        })
    }
}

impl Default for RekeyScheduler {
    fn default() -> Self {
        Self::new()
    }
}

/// Rekey trigger reason
#[derive(Debug, Clone, Copy)]
pub enum RekeyTrigger {
    /// No trigger (rekey not needed)
    None,
    
    /// Triggered by bytes threshold
    Bytes(u64),
    
    /// Triggered by time threshold
    Time(Duration),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_bytes_threshold_trigger() {
        let config = RekeyConfig {
            bytes_threshold: 1000,
            time_threshold: Duration::from_secs(3600),
            enabled: true,
        };
        let scheduler = RekeyScheduler::with_config(config);
        
        // Initially no rekey needed
        let (should, trigger) = scheduler.should_rekey().await;
        assert!(!should);
        assert!(matches!(trigger, RekeyTrigger::None));
        
        // Transfer 500 bytes - still below threshold
        scheduler.record_bytes(500).await;
        let (should, _) = scheduler.should_rekey().await;
        assert!(!should);
        
        // Transfer another 600 bytes - exceeds threshold
        scheduler.record_bytes(600).await;
        let (should, trigger) = scheduler.should_rekey().await;
        assert!(should);
        assert!(matches!(trigger, RekeyTrigger::Bytes(1100)));
        
        // Complete rekey
        scheduler.complete_rekey(trigger).await;
        
        // After rekey, counter resets
        assert_eq!(scheduler.bytes_transferred().await, 0);
    }
    
    #[tokio::test]
    async fn test_time_threshold_trigger() {
        let config = RekeyConfig {
            bytes_threshold: u64::MAX, // Disable bytes trigger
            time_threshold: Duration::from_millis(100),
            enabled: true,
        };
        let scheduler = RekeyScheduler::with_config(config);
        
        // Initially no rekey needed
        let (should, _) = scheduler.should_rekey().await;
        assert!(!should);
        
        // Wait for time threshold
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        let (should, trigger) = scheduler.should_rekey().await;
        assert!(should);
        assert!(matches!(trigger, RekeyTrigger::Time(_)));
        
        // Complete rekey
        scheduler.complete_rekey(trigger).await;
        
        // Time resets
        assert!(scheduler.time_since_rekey().await < Duration::from_millis(50));
    }
    
    #[tokio::test]
    async fn test_disabled_scheduler() {
        let config = RekeyConfig {
            bytes_threshold: 100,
            time_threshold: Duration::from_millis(10),
            enabled: false,
        };
        let scheduler = RekeyScheduler::with_config(config);
        
        // Transfer bytes and wait
        scheduler.record_bytes(200).await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        // Should not trigger when disabled
        let (should, _) = scheduler.should_rekey().await;
        assert!(!should);
    }
    
    #[tokio::test]
    async fn test_metrics_collection() {
        let config = RekeyConfig {
            bytes_threshold: 500,
            time_threshold: Duration::from_secs(3600),
            enabled: true,
        };
        let scheduler = RekeyScheduler::with_config(config);
        
        // First rekey cycle
        scheduler.record_bytes(600).await;
        let (_, trigger) = scheduler.should_rekey().await;
        scheduler.complete_rekey(trigger).await;
        
        // Second rekey cycle
        scheduler.record_bytes(700).await;
        let (_, trigger) = scheduler.should_rekey().await;
        scheduler.complete_rekey(trigger).await;
        
        // Check metrics
        let metrics = scheduler.metrics().await;
        assert_eq!(metrics.total_rekeys, 2);
        assert_eq!(metrics.rekeys_by_bytes, 2);
        assert_eq!(metrics.rekeys_by_time, 0);
        assert_eq!(metrics.total_bytes_transferred, 1300);
        assert_eq!(metrics.avg_bytes_per_interval, 650);
    }
    
    #[tokio::test]
    async fn test_failure_recording() {
        let scheduler = RekeyScheduler::new();
        
        scheduler.record_failure().await;
        scheduler.record_failure().await;
        
        let metrics = scheduler.metrics().await;
        assert_eq!(metrics.rekey_failures, 2);
    }
    
    #[tokio::test]
    async fn test_bytes_priority_over_time() {
        let config = RekeyConfig {
            bytes_threshold: 100,
            time_threshold: Duration::from_millis(50),
            enabled: true,
        };
        let scheduler = RekeyScheduler::with_config(config);
        
        // Transfer enough bytes to trigger bytes threshold
        scheduler.record_bytes(150).await;
        
        // Wait for time threshold to also pass
        tokio::time::sleep(Duration::from_millis(60)).await;
        
        // Should trigger by bytes (checked first)
        let (should, trigger) = scheduler.should_rekey().await;
        assert!(should);
        assert!(matches!(trigger, RekeyTrigger::Bytes(150)));
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn test_telemetry_integration() {
        // This test verifies that telemetry calls don't panic and internal metrics are updated
        let scheduler = RekeyScheduler::new();
        
        // Trigger a rekey by bytes
        scheduler.record_bytes(2_000_000_000).await; // 2GB
        let (should, trigger) = scheduler.should_rekey().await;
        assert!(should);
        
        // Get internal metrics before rekey
        let before_metrics = scheduler.metrics().await;
        let before_count = before_metrics.total_rekeys;
        
        // Complete rekey - should call record_counter and update internal metrics
        scheduler.complete_rekey(trigger).await;
        
        // Verify internal metrics incremented
        let after_metrics = scheduler.metrics().await;
        assert_eq!(
            after_metrics.total_rekeys,
            before_count + 1,
            "Rekey count should increment"
        );
        
        // Get internal metrics before failure
        let before_failures = after_metrics.rekey_failures;
        
        // Record a failure - should call record_counter and update internal metrics
        scheduler.record_failure().await;
        
        // Verify internal failure counter incremented
        let after_failures = scheduler.metrics().await.rekey_failures;
        assert_eq!(
            after_failures,
            before_failures + 1,
            "Failure count should increment"
        );
        
        // Note: We can't directly test Prometheus counter values here because REGISTRY is private,
        // but the record_counter calls are executed and logged. Integration tests at a higher
        // level (e.g., HTTP /metrics endpoint tests) can verify Prometheus exposition.
    }
}
