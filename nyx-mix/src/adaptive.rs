/// Adaptive Mix Module for Nyx Protocol
/// Implements dynamic mixing strategies based on network conditions and traffic patterns
/// This module provides adaptive cover traffic generation and mixing parameter optimization

use crate::cover::{CoverTrafficConfig, CoverTrafficGenerator};
use crate::errors::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptiveMixConfig {
    /// Base mixing interval
    pub base_interval: Duration,
    /// Minimum mixing interval (safety bound)
    pub min_interval: Duration,
    /// Maximum mixing interval (efficiency bound)
    pub max_interval: Duration,
    /// Target anonymity set size
    pub target_anonymity_set: usize,
    /// Adaptation sensitivity factor (0.0 - 1.0)
    pub adaptation_factor: f64,
    /// Network condition monitoring interval
    pub monitoring_interval: Duration,
    /// Cover traffic baseline ratio
    pub cover_traffic_ratio: f64,
}

impl Default for AdaptiveMixConfig {
    fn default() -> Self {
        Self {
            base_interval: Duration::from_millis(100),
            min_interval: Duration::from_millis(50),
            max_interval: Duration::from_millis(500),
            target_anonymity_set: 8,
            adaptation_factor: 0.3,
            monitoring_interval: Duration::from_secs(5),
            cover_traffic_ratio: 0.3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkConditions {
    pub rtt: Duration,
    pub bandwidth_estimate: u64, // bytes per second
    pub packet_loss_rate: f64,   // 0.0 - 1.0
    pub jitter: Duration,
    pub last_updated: Instant,
}

impl Default for NetworkConditions {
    fn default() -> Self {
        Self {
            rtt: Duration::from_millis(50),
            bandwidth_estimate: 1_000_000, // 1 Mbps
            packet_loss_rate: 0.01,        // 1%
            jitter: Duration::from_millis(5),
            last_updated: Instant::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MixingMetrics {
    pub packets_mixed: u64,
    pub cover_packets_sent: u64,
    pub actual_anonymity_set: usize,
    pub current_interval: Duration,
    pub adaptation_score: f64, // Quality metric (0.0 - 1.0)
    pub last_reset: Instant,
}

impl Default for MixingMetrics {
    fn default() -> Self {
        Self {
            packets_mixed: 0,
            cover_packets_sent: 0,
            actual_anonymity_set: 0,
            current_interval: Duration::from_millis(100),
            adaptation_score: 0.5,
            last_reset: Instant::now(),
        }
    }
}

/// Adaptive Mix Engine that adjusts mixing parameters based on network conditions
pub struct AdaptiveMixEngine {
    config: AdaptiveMixConfig,
    network_conditions: Arc<RwLock<NetworkConditions>>,
    metrics: Arc<Mutex<MixingMetrics>>,
    cover_generator: Arc<Mutex<CoverTrafficGenerator>>,
    adaptation_history: Arc<Mutex<Vec<(Instant, f64)>>>, // (timestamp, adaptation_score)
}

impl AdaptiveMixEngine {
    pub fn new(config: AdaptiveMixConfig) -> Result<Self> {
        let cover_config = CoverTrafficConfig {
            target_bandwidth: (config.cover_traffic_ratio * 1_000_000.0) as u64, // 1Mbps * ratio
            poisson_lambda: config.cover_traffic_ratio,
            min_packet_size: 64,
            max_packet_size: 1280,
            burst_probability: 0.1,
        };

        let cover_generator = CoverTrafficGenerator::new(cover_config)?;

        Ok(Self {
            config,
            network_conditions: Arc::new(RwLock::new(NetworkConditions::default())),
            metrics: Arc::new(Mutex::new(MixingMetrics::default())),
            cover_generator: Arc::new(Mutex::new(cover_generator)),
            adaptation_history: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Start the adaptive mixing engine
    pub async fn start(&self) -> Result<()> {
        let metrics = Arc::clone(&self.metrics);
        let network_conditions = Arc::clone(&self.network_conditions);
        let cover_generator = Arc::clone(&self.cover_generator);
        let config = self.config.clone();
        let adaptation_history = Arc::clone(&self.adaptation_history);

        tokio::spawn(async move {
            let mut adaptation_interval = interval(config.monitoring_interval);
            
            loop {
                adaptation_interval.tick().await;
                
                if let Err(e) = Self::adapt_mixing_parameters(
                    &config,
                    &network_conditions,
                    &metrics,
                    &cover_generator,
                    &adaptation_history,
                ).await {
                    warn!("Failed to adapt mixing parameters: {:?}", e);
                }
            }
        });

        info!("Adaptive mix engine started");
        Ok(())
    }

    /// Update network conditions for adaptation
    pub async fn update_network_conditions(&self, conditions: NetworkConditions) {
        let mut current_conditions = self.network_conditions.write().await;
        debug!("Updating network conditions: RTT={:?}, BW={} bytes/s", 
               conditions.rtt, conditions.bandwidth_estimate);
        *current_conditions = conditions;
    }

    /// Report packet mixing event
    pub async fn report_packet_mixed(&self, anonymity_set_size: usize) -> Result<()> {
        let mut metrics = self.metrics.lock().await;
        metrics.packets_mixed += 1;
        metrics.actual_anonymity_set = anonymity_set_size;
        Ok(())
    }

    /// Get current mixing metrics
    pub async fn get_metrics(&self) -> MixingMetrics {
        self.metrics.lock().await.clone()
    }

    /// Calculate optimal mixing interval based on network conditions
    async fn calculate_optimal_interval(
        config: &AdaptiveMixConfig,
        conditions: &NetworkConditions,
        current_metrics: &MixingMetrics,
    ) -> Duration {
        // Base calculation considering RTT and packet loss
        let rtt_factor = (conditions.rtt.as_millis() as f64) / 100.0; // Normalize to ~100ms
        let loss_factor = 1.0 + conditions.packet_loss_rate * 5.0; // Increase interval on loss
        let jitter_factor = 1.0 + (conditions.jitter.as_millis() as f64) / 50.0;

        // Anonymity set consideration
        let anonymity_factor = if current_metrics.actual_anonymity_set < config.target_anonymity_set {
            0.8 // Decrease interval to increase mixing frequency
        } else {
            1.2 // Increase interval to save resources
        };

        let adjustment = rtt_factor * loss_factor * jitter_factor * anonymity_factor;
        let base_ms = config.base_interval.as_millis() as f64;
        let optimal_ms = base_ms * adjustment * config.adaptation_factor + 
                        base_ms * (1.0 - config.adaptation_factor);

        let optimal_duration = Duration::from_millis(optimal_ms as u64);

        // Enforce bounds
        if optimal_duration < config.min_interval {
            config.min_interval
        } else if optimal_duration > config.max_interval {
            config.max_interval
        } else {
            optimal_duration
        }
    }

    /// Adaptation logic for mixing parameters
    async fn adapt_mixing_parameters(
        config: &AdaptiveMixConfig,
        network_conditions: &Arc<RwLock<NetworkConditions>>,
        metrics: &Arc<Mutex<MixingMetrics>>,
        cover_generator: &Arc<Mutex<CoverTrafficGenerator>>,
        adaptation_history: &Arc<Mutex<Vec<(Instant, f64)>>>,
    ) -> Result<()> {
        let conditions = network_conditions.read().await;
        let mut current_metrics = metrics.lock().await;

        // Calculate optimal mixing interval
        let optimal_interval = Self::calculate_optimal_interval(
            config,
            &conditions,
            &current_metrics,
        ).await;

        // Update mixing interval
        current_metrics.current_interval = optimal_interval;

        // Calculate adaptation score
        let anonymity_score = if current_metrics.actual_anonymity_set > 0 {
            (current_metrics.actual_anonymity_set as f64) / (config.target_anonymity_set as f64)
        } else {
            0.0
        };

        let latency_score = 1.0 - (conditions.rtt.as_millis() as f64) / 1000.0; // Normalize to 1s
        let loss_score = 1.0 - conditions.packet_loss_rate;

        current_metrics.adaptation_score = (anonymity_score + latency_score + loss_score) / 3.0;

        // Update cover traffic generation parameters
        let mut cover_gen = cover_generator.lock().await;
        let target_bandwidth = (conditions.bandwidth_estimate as f64 * config.cover_traffic_ratio) as u64;
        cover_gen.update_target_bandwidth(target_bandwidth)?;

        // Store adaptation history
        let mut history = adaptation_history.lock().await;
        history.push((Instant::now(), current_metrics.adaptation_score));
        
        // Keep only recent history (last hour)
        let cutoff = Instant::now() - Duration::from_secs(3600);
        history.retain(|(timestamp, _)| *timestamp > cutoff);

        debug!(
            "Adapted mixing parameters: interval={:?}, score={:.3}, anonymity_set={}",
            optimal_interval, current_metrics.adaptation_score, current_metrics.actual_anonymity_set
        );

        Ok(())
    }

    /// Get adaptation quality over time
    pub async fn get_adaptation_history(&self) -> Vec<(Instant, f64)> {
        self.adaptation_history.lock().await.clone()
    }

    /// Reset metrics and start fresh adaptation cycle
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.lock().await;
        metrics.packets_mixed = 0;
        metrics.cover_packets_sent = 0;
        metrics.actual_anonymity_set = 0;
        metrics.last_reset = Instant::now();
        
        let mut history = self.adaptation_history.lock().await;
        history.clear();
        
        info!("Reset adaptive mix metrics");
    }

    /// Check if adaptation is working effectively
    pub async fn is_adaptation_effective(&self) -> bool {
        let history = self.adaptation_history.lock().await;
        
        if history.len() < 3 {
            return false; // Not enough data
        }

        // Check if adaptation score is improving over time
        let recent_scores: Vec<f64> = history.iter()
            .rev()
            .take(5)
            .map(|(_, score)| *score)
            .collect();

        let avg_recent = recent_scores.iter().sum::<f64>() / recent_scores.len() as f64;
        avg_recent > 0.6 // Threshold for "effective"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adaptive_mix_creation() -> Result<()> {
        let config = AdaptiveMixConfig::default();
        let engine = AdaptiveMixEngine::new(config)?;
        
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.packets_mixed, 0);
        assert_eq!(metrics.adaptation_score, 0.5);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_network_conditions_update() -> Result<()> {
        let config = AdaptiveMixConfig::default();
        let engine = AdaptiveMixEngine::new(config)?;
        
        let new_conditions = NetworkConditions {
            rtt: Duration::from_millis(100),
            bandwidth_estimate: 2_000_000,
            packet_loss_rate: 0.05,
            jitter: Duration::from_millis(10),
            last_updated: Instant::now(),
        };

        engine.update_network_conditions(new_conditions.clone()).await;
        
        let stored_conditions = engine.network_conditions.read().await;
        assert_eq!(stored_conditions.rtt, new_conditions.rtt);
        assert_eq!(stored_conditions.bandwidth_estimate, new_conditions.bandwidth_estimate);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_adaptation_interval_calculation() -> Result<()> {
        let config = AdaptiveMixConfig::default();
        let conditions = NetworkConditions::default();
        let metrics = MixingMetrics::default();

        let interval = AdaptiveMixEngine::calculate_optimal_interval(
            &config,
            &conditions,
            &metrics,
        ).await;

        assert!(interval >= config.min_interval);
        assert!(interval <= config.max_interval);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_packet_mixing_reporting() -> Result<()> {
        let config = AdaptiveMixConfig::default();
        let engine = AdaptiveMixEngine::new(config)?;
        
        engine.report_packet_mixed(5).await?;
        engine.report_packet_mixed(8).await?;
        
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.packets_mixed, 2);
        assert_eq!(metrics.actual_anonymity_set, 8);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_adaptation_effectiveness() -> Result<()> {
        let config = AdaptiveMixConfig::default();
        let engine = AdaptiveMixEngine::new(config)?;
        
        // Initially not effective due to lack of data
        assert!(!engine.is_adaptation_effective().await);
        
        // Add some history manually for testing
        {
            let mut history = engine.adaptation_history.lock().await;
            for i in 0..5 {
                history.push((Instant::now(), 0.7 + (i as f64) * 0.05));
            }
        }
        
        assert!(engine.is_adaptation_effective().await);
        
        Ok(())
    }

    #[tokio::test]
    async fn test_metrics_reset() -> Result<()> {
        let config = AdaptiveMixConfig::default();
        let engine = AdaptiveMixEngine::new(config)?;
        
        engine.report_packet_mixed(10).await?;
        engine.reset_metrics().await;
        
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.packets_mixed, 0);
        assert_eq!(metrics.actual_anonymity_set, 0);
        
        let history = engine.get_adaptation_history().await;
        assert!(history.is_empty());
        
        Ok(())
    }
}
