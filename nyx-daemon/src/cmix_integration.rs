//! cMix Integration Manager
//!
//! This module integrates the cMix batch processing system into the daemon's
//! session pipeline as specified in `spec/Nyx_Protocol_v1.0_Spec_EN.md` §4.
//!
//! # Responsibilities
//! - Initialize and manage cMix batcher lifecycle
//! - Batch packet processing with VDF delays
//! - Adaptive cover traffic integration
//! - Configuration management via nyx.toml

use nyx_mix::cmix::{Batcher, BatchStats, VerifiedBatch};
use nyx_mix::adaptive::{AdaptiveMixConfig, AdaptiveMixEngine, NetworkConditions};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, trace, warn};

/// cMix integration errors
#[derive(Debug, Error)]
pub enum CmixIntegrationError {
    #[error("Batcher error: {0}")]
    BatcherError(String),
    #[error("Channel send error: {0}")]
    ChannelError(String),
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// cMix configuration from nyx.toml [mix] section
#[derive(Debug, Clone)]
pub struct CmixConfig {
    /// Enable cMix batch processing
    pub enabled: bool,
    /// Batch size (number of packets)
    pub batch_size: usize,
    /// VDF delay in milliseconds
    pub vdf_delay_ms: u32,
    /// Batch timeout (if not filled)
    pub batch_timeout: Duration,
    /// Target network utilization [0.0, 1.0]
    pub target_utilization: f64,
    /// Enable adaptive cover traffic
    pub enable_cover_traffic: bool,
}

impl Default for CmixConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default
            batch_size: 100,
            vdf_delay_ms: 100,
            batch_timeout: Duration::from_secs(1),
            target_utilization: 0.4, // 40% default
            enable_cover_traffic: true,
        }
    }
}

/// Packet for cMix processing
#[derive(Debug, Clone)]
pub struct CmixPacket {
    pub data: Vec<u8>,
    pub is_cover: bool, // True if this is cover traffic
}

/// cMix Integration Manager
///
/// Manages the cMix batch processing pipeline and adaptive cover traffic.
pub struct CmixIntegrationManager {
    config: CmixConfig,
    batcher: Arc<RwLock<Batcher>>,
    adaptive_engine: Option<Arc<RwLock<AdaptiveMixEngine>>>,
    packet_tx: mpsc::UnboundedSender<CmixPacket>,
    packet_rx: Arc<RwLock<mpsc::UnboundedReceiver<CmixPacket>>>,
    batch_output_tx: mpsc::UnboundedSender<VerifiedBatch>,
    stats: Arc<RwLock<CmixStats>>,
}

/// cMix statistics
#[derive(Debug, Default, Clone)]
pub struct CmixStats {
    pub total_packets: u64,
    pub cover_packets: u64,
    pub real_packets: u64,
    pub batches_emitted: u64,
    pub current_utilization: f64,
}

impl CmixIntegrationManager {
    /// Create new cMix integration manager
    pub fn new(
        config: CmixConfig,
        batch_output_tx: mpsc::UnboundedSender<VerifiedBatch>,
    ) -> Self {
        // Create batcher
        let batcher = Batcher::with_vdf_delay(
            config.batch_size,
            config.batch_timeout,
            config.vdf_delay_ms,
        );

        // Create adaptive engine if cover traffic enabled
        let adaptive_engine = if config.enable_cover_traffic {
            let adaptive_config = AdaptiveMixConfig {
                base_interval: Duration::from_millis(100),
                min_interval: Duration::from_millis(50),
                max_interval: Duration::from_millis(500),
                target_anonymity_set: 8,
                adaptation_factor: config.target_utilization,
                monitoring_interval: Duration::from_secs(10),
                cover_traffic_ratio: 0.3,
            };
            match AdaptiveMixEngine::new(adaptive_config) {
                Ok(engine) => Some(Arc::new(RwLock::new(engine))),
                Err(e) => {
                    error!("Failed to create adaptive engine: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Create packet channel
        let (packet_tx, packet_rx) = mpsc::unbounded_channel();

        Self {
            config,
            batcher: Arc::new(RwLock::new(batcher)),
            adaptive_engine,
            packet_tx,
            packet_rx: Arc::new(RwLock::new(packet_rx)),
            batch_output_tx,
            stats: Arc::new(RwLock::new(CmixStats::default())),
        }
    }

    /// Get packet sender for submitting packets to cMix
    pub fn get_packet_sender(&self) -> mpsc::UnboundedSender<CmixPacket> {
        self.packet_tx.clone()
    }

    /// Start the cMix processing loop
    ///
    /// This should be spawned as a background task.
    pub async fn start_processing_loop(self: Arc<Self>) {
        info!("Starting cMix processing loop");

        // Spawn batch processing task
        let manager = self.clone();
        tokio::spawn(async move {
            manager.batch_processing_loop().await;
        });

        // Spawn cover traffic injection if enabled
        if self.config.enable_cover_traffic {
            let manager = self.clone();
            tokio::spawn(async move {
                manager.cover_traffic_loop().await;
            });
        }

        // Spawn stats logging
        let manager = self.clone();
        tokio::spawn(async move {
            manager.stats_logging_loop().await;
        });
    }

    /// Batch processing loop
    async fn batch_processing_loop(&self) {
        let mut rx = self.packet_rx.write().await;

        while let Some(packet) = rx.recv().await {
            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.total_packets += 1;
                if packet.is_cover {
                    stats.cover_packets += 1;
                } else {
                    stats.real_packets += 1;
                }
            }

            // Add packet to batcher
            let mut batcher = self.batcher.write().await;
            match batcher.push(packet.data) {
                Ok(Some(batch)) => {
                    // Batch is ready
                    debug!("Batch {} ready with {} packets", batch.id, batch.packets.len());

                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.batches_emitted += 1;
                    }

                    // Send to output
                    if let Err(e) = self.batch_output_tx.send(batch) {
                        error!("Failed to send batch: {}", e);
                    }
                }
                Ok(None) => {
                    // Batch not ready yet
                    trace!("Packet added to batch");
                }
                Err(e) => {
                    error!("Failed to add packet to batch: {}", e);
                }
            }
        }

        warn!("cMix batch processing loop terminated");
    }

    /// Cover traffic injection loop
    ///
    /// Injects cover packets to maintain target utilization rate.
    async fn cover_traffic_loop(&self) {
        if self.adaptive_engine.is_none() {
            warn!("Cover traffic loop started but adaptive engine not initialized");
            return;
        }

        let mut interval_timer = interval(Duration::from_millis(100)); // 10 Hz

        loop {
            interval_timer.tick().await;

            // Get current cover rate
            let cover_rate = {
                let stats = self.stats.read().await;
                if stats.total_packets > 0 {
                    stats.cover_packets as f64 / stats.total_packets as f64
                } else {
                    0.0
                }
            };

            // Calculate target utilization
            let target_utilization = self.config.target_utilization;
            let current_utilization = cover_rate + (1.0 - cover_rate);

            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.current_utilization = current_utilization;
            }

            // Inject cover packet if below target
            if current_utilization < target_utilization {
                // Generate cover packet (dummy data)
                let cover_packet = CmixPacket {
                    data: vec![0u8; 1200], // Typical packet size
                    is_cover: true,
                };

                if let Err(e) = self.packet_tx.send(cover_packet) {
                    error!("Failed to inject cover packet: {}", e);
                }
            }
        }
    }

    /// Stats logging loop
    async fn stats_logging_loop(&self) {
        let mut interval_timer = interval(Duration::from_secs(10));

        loop {
            interval_timer.tick().await;

            let stats = self.stats.read().await;
            info!(
                "cMix Stats - Total: {}, Cover: {}, Real: {}, Batches: {}, Utilization: {:.2}%",
                stats.total_packets,
                stats.cover_packets,
                stats.real_packets,
                stats.batches_emitted,
                stats.current_utilization * 100.0
            );

            // Log batcher stats
            let batcher = self.batcher.read().await;
            debug!(
                "Batcher Stats - Emitted: {}, Errors: {}, VDF: {}",
                batcher.stats.emitted, batcher.stats.errors, batcher.stats.vdf_computations
            );
        }
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> CmixStats {
        self.stats.read().await.clone()
    }

    /// Get batcher statistics
    pub async fn get_batcher_stats(&self) -> BatchStats {
        let batcher = self.batcher.read().await;
        batcher.stats.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let (batch_tx, _batch_rx) = mpsc::unbounded_channel();
        let config = CmixConfig {
            enabled: true,
            batch_size: 10,
            ..Default::default()
        };

        let manager = CmixIntegrationManager::new(config, batch_tx);
        let packet_tx = manager.get_packet_sender();

        // Should be able to send packets
        let packet = CmixPacket {
            data: vec![1, 2, 3],
            is_cover: false,
        };
        assert!(packet_tx.send(packet).is_ok());
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let (batch_tx, _batch_rx) = mpsc::unbounded_channel();
        let config = CmixConfig {
            enabled: true,
            batch_size: 100,
            ..Default::default()
        };

        let manager = Arc::new(CmixIntegrationManager::new(config, batch_tx));
        
        // Start processing loop
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone.batch_processing_loop().await;
        });

        let packet_tx = manager.get_packet_sender();

        // Send some packets
        for i in 0..5 {
            let packet = CmixPacket {
                data: vec![i],
                is_cover: i % 2 == 0,
            };
            packet_tx.send(packet).unwrap();
        }

        // Give time for processing
        tokio::time::sleep(Duration::from_millis(200)).await;

        let stats = manager.get_stats().await;
        assert!(stats.total_packets >= 5, "Expected at least 5 packets, got {}", stats.total_packets);
    }

    #[tokio::test]
    async fn test_cover_traffic_disabled() {
        let (batch_tx, _batch_rx) = mpsc::unbounded_channel();
        let config = CmixConfig {
            enabled: true,
            enable_cover_traffic: false,
            ..Default::default()
        };

        let manager = CmixIntegrationManager::new(config, batch_tx);
        assert!(manager.adaptive_engine.is_none());
    }

    #[tokio::test]
    async fn test_cover_traffic_enabled() {
        let (batch_tx, _batch_rx) = mpsc::unbounded_channel();
        let config = CmixConfig {
            enabled: true,
            enable_cover_traffic: true,
            ..Default::default()
        };

        let manager = CmixIntegrationManager::new(config, batch_tx);
        assert!(manager.adaptive_engine.is_some());
    }

    #[tokio::test]
    async fn test_default_config() {
        let config = CmixConfig::default();
        assert!(!config.enabled); // Disabled by default
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.vdf_delay_ms, 100);
        assert_eq!(config.target_utilization, 0.4);
        assert!(config.enable_cover_traffic);
    }
}
