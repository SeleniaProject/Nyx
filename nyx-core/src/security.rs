//! Post-Compromise Recovery (PCR) Detection and Triggering
//!
//! Implements compromise detection triggers and PCR orchestration for Nyx Protocol.
//!
//! ## Detection Mechanisms
//! - Anomalous traffic pattern detection (pluggable)
//! - External signals via management API
//! - Manual administrator triggers
//!
//! ## PCR Actions
//! - Ephemeral key regeneration
//! - Session re-establishment
//! - Forward secrecy guarantee
//! - Audit logging

#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{error, info};

/// PCR trigger reason
#[derive(Debug, Clone)]
pub enum PcrTrigger {
    /// Anomalous traffic pattern detected
    AnomalousTraffic {
        description: String,
        severity: TriggerSeverity,
    },
    
    /// External signal from management API
    ExternalSignal {
        source: String,
        reason: String,
    },
    
    /// Manual administrator trigger
    ManualTrigger {
        operator: String,
        reason: String,
    },
    
    /// Automated periodic trigger (compliance/policy)
    PeriodicRotation {
        interval: Duration,
    },
}

/// Trigger severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TriggerSeverity {
    /// Low severity - informational
    Low,
    /// Medium severity - potential issue
    Medium,
    /// High severity - likely compromise
    High,
    /// Critical severity - confirmed compromise
    Critical,
}

/// PCR event for audit logging
#[derive(Debug, Clone)]
pub struct PcrEvent {
    /// Event timestamp
    pub timestamp: SystemTime,
    
    /// Trigger that caused this PCR
    pub trigger: PcrTrigger,
    
    /// Number of sessions affected
    pub sessions_affected: usize,
    
    /// Whether PCR succeeded
    pub success: bool,
    
    /// Error message if failed
    pub error: Option<String>,
    
    /// Duration of PCR operation
    pub duration: Duration,
}

/// PCR detector configuration
#[derive(Debug, Clone)]
pub struct PcrDetectorConfig {
    /// Enable anomaly detection
    pub enable_anomaly_detection: bool,
    
    /// Enable external signal reception
    pub enable_external_signals: bool,
    
    /// Enable periodic rotation
    pub enable_periodic_rotation: bool,
    
    /// Periodic rotation interval (default: 24 hours)
    pub rotation_interval: Duration,
    
    /// Anomaly detection threshold
    pub anomaly_threshold: f64,
}

impl Default for PcrDetectorConfig {
    fn default() -> Self {
        Self {
            enable_anomaly_detection: true,
            enable_external_signals: true,
            enable_periodic_rotation: false,
            rotation_interval: Duration::from_secs(86400), // 24 hours
            anomaly_threshold: 0.8, // 80% confidence threshold
        }
    }
}

/// PCR detector and trigger manager
pub struct PcrDetector {
    /// Configuration
    config: PcrDetectorConfig,
    
    /// Audit log of PCR events
    audit_log: Arc<RwLock<Vec<PcrEvent>>>,
    
    /// Last periodic rotation time
    last_rotation: Arc<RwLock<SystemTime>>,
    
    /// Metrics
    metrics: Arc<RwLock<PcrMetrics>>,
}

/// PCR metrics
#[derive(Debug, Clone, Default)]
pub struct PcrMetrics {
    /// Total PCR events triggered
    pub total_triggers: u64,
    
    /// PCR triggers by reason
    pub triggers_by_anomaly: u64,
    pub triggers_by_external: u64,
    pub triggers_by_manual: u64,
    pub triggers_by_periodic: u64,
    
    /// Successful PCR operations
    pub successful_pcr: u64,
    
    /// Failed PCR operations
    pub failed_pcr: u64,
    
    /// Total sessions recovered
    pub sessions_recovered: u64,
    
    /// Average PCR duration
    pub avg_pcr_duration: Duration,
}

impl PcrDetector {
    /// Create a new PCR detector with default configuration
    pub fn new() -> Self {
        Self::with_config(PcrDetectorConfig::default())
    }
    
    /// Create a new PCR detector with custom configuration
    pub fn with_config(config: PcrDetectorConfig) -> Self {
        Self {
            config,
            audit_log: Arc::new(RwLock::new(Vec::new())),
            last_rotation: Arc::new(RwLock::new(SystemTime::now())),
            metrics: Arc::new(RwLock::new(PcrMetrics::default())),
        }
    }
    
    /// Detect anomalous traffic patterns
    ///
    /// This is a pluggable interface - implement custom detection logic here
    pub async fn detect_anomaly(&self, traffic_stats: &TrafficStats) -> Option<PcrTrigger> {
        if !self.config.enable_anomaly_detection {
            return None;
        }
        
        // Example: Check for suspicious packet rate changes
        if traffic_stats.packet_rate_change > self.config.anomaly_threshold {
            let severity = if traffic_stats.packet_rate_change > 0.95 {
                TriggerSeverity::Critical
            } else if traffic_stats.packet_rate_change > 0.90 {
                TriggerSeverity::High
            } else {
                TriggerSeverity::Medium
            };
            
            return Some(PcrTrigger::AnomalousTraffic {
                description: format!(
                    "Abnormal packet rate change: {:.1}%",
                    traffic_stats.packet_rate_change * 100.0
                ),
                severity,
            });
        }
        
        // Example: Check for unusual connection patterns
        if traffic_stats.failed_handshakes > 10 {
            return Some(PcrTrigger::AnomalousTraffic {
                description: format!(
                    "High handshake failure rate: {}",
                    traffic_stats.failed_handshakes
                ),
                severity: TriggerSeverity::High,
            });
        }
        
        None
    }
    
    /// Receive external signal to trigger PCR
    pub async fn external_signal(&self, source: String, reason: String) -> Result<(), String> {
        if !self.config.enable_external_signals {
            return Err("External signals disabled".to_string());
        }
        
        let trigger = PcrTrigger::ExternalSignal { source, reason };
        
        let mut metrics = self.metrics.write().await;
        metrics.total_triggers += 1;
        metrics.triggers_by_external += 1;
        
        info!("PCR triggered by external signal: {:?}", trigger);
        Ok(())
    }
    
    /// Manual trigger by administrator
    pub async fn manual_trigger(&self, operator: String, reason: String) -> Result<(), String> {
        let trigger = PcrTrigger::ManualTrigger { operator, reason };
        
        let mut metrics = self.metrics.write().await;
        metrics.total_triggers += 1;
        metrics.triggers_by_manual += 1;
        
        info!("PCR manually triggered: {:?}", trigger);
        Ok(())
    }
    
    /// Check if periodic rotation is due
    pub async fn check_periodic_rotation(&self) -> Option<PcrTrigger> {
        if !self.config.enable_periodic_rotation {
            return None;
        }
        
        let last_rotation = *self.last_rotation.read().await;
        let elapsed = SystemTime::now()
            .duration_since(last_rotation)
            .unwrap_or(Duration::ZERO);
        
        if elapsed >= self.config.rotation_interval {
            return Some(PcrTrigger::PeriodicRotation {
                interval: self.config.rotation_interval,
            });
        }
        
        None
    }
    
    /// Record PCR event to audit log
    pub async fn record_event(&self, event: PcrEvent) {
        let mut log = self.audit_log.write().await;
        let mut metrics = self.metrics.write().await;
        
        // Update metrics
        if event.success {
            metrics.successful_pcr += 1;
            metrics.sessions_recovered += event.sessions_affected as u64;
        } else {
            metrics.failed_pcr += 1;
        }
        
        // Update average duration
        let total_pcr = metrics.successful_pcr + metrics.failed_pcr;
        if total_pcr > 0 {
            let total_duration = metrics.avg_pcr_duration.as_millis() * (total_pcr - 1) as u128
                + event.duration.as_millis();
            metrics.avg_pcr_duration = Duration::from_millis((total_duration / total_pcr as u128) as u64);
        }
        
        // Update trigger reason counters
        match &event.trigger {
            PcrTrigger::AnomalousTraffic { .. } => metrics.triggers_by_anomaly += 1,
            PcrTrigger::ExternalSignal { .. } => metrics.triggers_by_external += 1,
            PcrTrigger::ManualTrigger { .. } => metrics.triggers_by_manual += 1,
            PcrTrigger::PeriodicRotation { .. } => metrics.triggers_by_periodic += 1,
        }
        
        // Add to audit log
        log.push(event.clone());
        
        // Log to tracing
        if event.success {
            info!(
                "PCR event recorded: {:?}, {} sessions affected, duration: {:?}",
                event.trigger, event.sessions_affected, event.duration
            );
        } else {
            error!(
                "PCR event failed: {:?}, error: {:?}",
                event.trigger, event.error
            );
        }
    }
    
    /// Get audit log
    pub async fn get_audit_log(&self) -> Vec<PcrEvent> {
        self.audit_log.read().await.clone()
    }
    
    /// Get metrics
    pub async fn get_metrics(&self) -> PcrMetrics {
        self.metrics.read().await.clone()
    }
    
    /// Clear audit log (keep recent N events)
    pub async fn clear_audit_log(&self, keep_recent: usize) {
        let mut log = self.audit_log.write().await;
        if log.len() > keep_recent {
            *log = log.iter().rev().take(keep_recent).rev().cloned().collect();
        }
    }
}

impl Default for PcrDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Traffic statistics for anomaly detection
#[derive(Debug, Clone)]
pub struct TrafficStats {
    /// Packet rate change (0.0-1.0)
    pub packet_rate_change: f64,
    
    /// Failed handshake attempts
    pub failed_handshakes: u64,
    
    /// Connection drops
    pub connection_drops: u64,
    
    /// Unusual packet sizes
    pub unusual_packet_sizes: u64,
}

impl TrafficStats {
    /// Create new traffic stats with defaults
    pub fn new() -> Self {
        Self {
            packet_rate_change: 0.0,
            failed_handshakes: 0,
            connection_drops: 0,
            unusual_packet_sizes: 0,
        }
    }
}

impl Default for TrafficStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_anomaly_detection() {
        let detector = PcrDetector::new();
        
        // Normal traffic - no trigger
        let stats = TrafficStats {
            packet_rate_change: 0.5,
            failed_handshakes: 2,
            connection_drops: 1,
            unusual_packet_sizes: 0,
        };
        assert!(detector.detect_anomaly(&stats).await.is_none());
        
        // High packet rate change - should trigger High severity (0.90-0.95)
        let stats = TrafficStats {
            packet_rate_change: 0.92,
            ..Default::default()
        };
        let trigger = detector.detect_anomaly(&stats).await;
        assert!(trigger.is_some());
        if let Some(PcrTrigger::AnomalousTraffic { severity, .. }) = trigger {
            assert_eq!(severity, TriggerSeverity::High);
        }
        
        // Medium severity (0.80-0.90)
        let stats = TrafficStats {
            packet_rate_change: 0.85,
            ..Default::default()
        };
        let trigger = detector.detect_anomaly(&stats).await;
        assert!(trigger.is_some());
        if let Some(PcrTrigger::AnomalousTraffic { severity, .. }) = trigger {
            assert_eq!(severity, TriggerSeverity::Medium);
        }
    }
    
    #[tokio::test]
    async fn test_external_signal() {
        let detector = PcrDetector::new();
        
        let result = detector
            .external_signal("admin_api".to_string(), "suspected breach".to_string())
            .await;
        assert!(result.is_ok());
        
        let metrics = detector.get_metrics().await;
        assert_eq!(metrics.total_triggers, 1);
        assert_eq!(metrics.triggers_by_external, 1);
    }
    
    #[tokio::test]
    async fn test_manual_trigger() {
        let detector = PcrDetector::new();
        
        let result = detector
            .manual_trigger("alice".to_string(), "security audit".to_string())
            .await;
        assert!(result.is_ok());
        
        let metrics = detector.get_metrics().await;
        assert_eq!(metrics.total_triggers, 1);
        assert_eq!(metrics.triggers_by_manual, 1);
    }
    
    #[tokio::test]
    async fn test_periodic_rotation() {
        let config = PcrDetectorConfig {
            enable_periodic_rotation: true,
            rotation_interval: Duration::from_millis(10),
            ..Default::default()
        };
        let detector = PcrDetector::with_config(config);
        
        // Initially no rotation needed
        assert!(detector.check_periodic_rotation().await.is_none());
        
        // Wait for interval
        tokio::time::sleep(Duration::from_millis(15)).await;
        
        // Should trigger now
        let trigger = detector.check_periodic_rotation().await;
        assert!(trigger.is_some());
        assert!(matches!(trigger, Some(PcrTrigger::PeriodicRotation { .. })));
    }
    
    #[tokio::test]
    async fn test_audit_log() {
        let detector = PcrDetector::new();
        
        let event1 = PcrEvent {
            timestamp: SystemTime::now(),
            trigger: PcrTrigger::ManualTrigger {
                operator: "alice".to_string(),
                reason: "test".to_string(),
            },
            sessions_affected: 5,
            success: true,
            error: None,
            duration: Duration::from_millis(100),
        };
        
        detector.record_event(event1).await;
        
        let log = detector.get_audit_log().await;
        assert_eq!(log.len(), 1);
        
        let metrics = detector.get_metrics().await;
        assert_eq!(metrics.successful_pcr, 1);
        assert_eq!(metrics.sessions_recovered, 5);
    }
    
    #[tokio::test]
    async fn test_audit_log_cleanup() {
        let detector = PcrDetector::new();
        
        // Add 5 events
        for i in 0..5 {
            let event = PcrEvent {
                timestamp: SystemTime::now(),
                trigger: PcrTrigger::ManualTrigger {
                    operator: format!("op{}", i),
                    reason: "test".to_string(),
                },
                sessions_affected: 1,
                success: true,
                error: None,
                duration: Duration::from_millis(10),
            };
            detector.record_event(event).await;
        }
        
        // Keep only 2 most recent
        detector.clear_audit_log(2).await;
        
        let log = detector.get_audit_log().await;
        assert_eq!(log.len(), 2);
    }
}
