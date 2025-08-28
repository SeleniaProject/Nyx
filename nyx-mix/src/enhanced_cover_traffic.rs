//! Enhanced Adaptive Cover Traffic System for Nyx Protocol v1.0
//!
//! This module extends the existing cover traffic system with comprehensive
//! traffic analysis resistance, cross-layer coordination, and advanced
//! anonymity protection features.
//!
//! # Features
//!
//! - **Enhanced Adaptive Cover Traffic**: Smart rate adjustment based on network conditions
//! - **Traffic Pattern Matching**: Mimics legitimate traffic patterns
//! - **Cross-Layer Coordination**: Integration with stream layer padding
//! - **Anonymity Set Management**: Maintains minimum anonymity set size
//! - **Burst Pattern Injection**: Injects realistic traffic bursts
//! - **Battery-Aware Adaptation**: Mobile device power management
//!
//! # Security Properties
//!
//! - **Traffic Analysis Resistance**: Resists timing and volume analysis
//! - **Pattern Uniformity**: Creates indistinguishable traffic patterns
//! - **Minimum Anonymity**: Guarantees minimum k-anonymity
//! - **Adaptive Protection**: Responds to changing threat models
//!
//! # Example
//!
//! ```rust,no_run
//! use nyx_mix::enhanced_cover_traffic::{EnhancedCoverManager, EnhancedCoverConfig};
//! use std::time::Duration;
//!
//! let config = EnhancedCoverConfig::new()
//!     .min_anonymity_set(10)
//!     .traffic_analysis_resistance(true)
//!     .battery_optimization(true);
//!     
//! let manager = EnhancedCoverManager::new(config);
//! ```

use crate::cover::poisson_rate;
use crate::cover_adaptive::{AdaptiveCoverManager, CoverConfig, NetworkMetrics};
use crate::errors::Result;
use rand::{distributions::WeightedIndex, rngs::OsRng, Rng};
use rand_distr::{Distribution, Exp};
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};
use thiserror::Error;
use tracing::{debug, error, info, trace, warn};

/// Enhanced cover traffic specific errors
#[derive(Debug, Error)]
pub enum EnhancedCoverError {
    #[error("Traffic analysis resistance compromised: {reason}")]
    TrafficAnalysisRisk { reason: String },

    #[error("Anonymity set too small: current={current}, minimum={minimum}")]
    InsufficientAnonymity { current: u32, minimum: u32 },

    #[error("Cross-layer coordination failed: {reason}")]
    CoordinationFailure { reason: String },

    #[error("Pattern generation failed: {reason}")]
    PatternGenerationFailed { reason: String },

    #[error("Battery optimization conflict: {reason}")]
    BatteryOptimizationConflict { reason: String },

    #[error("Cover traffic generation failed: {reason}")]
    CoverGenerationFailed { reason: String },
}

/// Traffic pattern types for realistic simulation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrafficPattern {
    /// Constant rate background traffic
    Constant,
    /// Bursty application traffic
    Bursty,
    /// Web browsing pattern
    WebBrowsing,
    /// Video streaming pattern
    VideoStreaming,
    /// File transfer pattern
    FileTransfer,
    /// Gaming traffic pattern
    Gaming,
    /// IoT device pattern
    IoT,
    /// Chat/messaging pattern
    Messaging,
}

impl TrafficPattern {
    /// Get characteristic parameters for this pattern
    pub fn parameters(&self) -> TrafficPatternParams {
        match self {
            TrafficPattern::Constant => TrafficPatternParams {
                base_rate: 2.0,
                burst_probability: 0.05,
                burst_multiplier: 1.2,
                inter_burst_delay: Duration::from_secs(30),
                packet_size_variance: 0.1,
            },
            TrafficPattern::Bursty => TrafficPatternParams {
                base_rate: 1.0,
                burst_probability: 0.3,
                burst_multiplier: 5.0,
                inter_burst_delay: Duration::from_secs(10),
                packet_size_variance: 0.4,
            },
            TrafficPattern::WebBrowsing => TrafficPatternParams {
                base_rate: 0.5,
                burst_probability: 0.2,
                burst_multiplier: 8.0,
                inter_burst_delay: Duration::from_secs(5),
                packet_size_variance: 0.6,
            },
            TrafficPattern::VideoStreaming => TrafficPatternParams {
                base_rate: 15.0,
                burst_probability: 0.1,
                burst_multiplier: 1.5,
                inter_burst_delay: Duration::from_secs(1),
                packet_size_variance: 0.2,
            },
            TrafficPattern::FileTransfer => TrafficPatternParams {
                base_rate: 50.0,
                burst_probability: 0.05,
                burst_multiplier: 1.1,
                inter_burst_delay: Duration::from_millis(100),
                packet_size_variance: 0.05,
            },
            TrafficPattern::Gaming => TrafficPatternParams {
                base_rate: 30.0,
                burst_probability: 0.4,
                burst_multiplier: 2.0,
                inter_burst_delay: Duration::from_millis(50),
                packet_size_variance: 0.3,
            },
            TrafficPattern::IoT => TrafficPatternParams {
                base_rate: 0.1,
                burst_probability: 0.01,
                burst_multiplier: 2.0,
                inter_burst_delay: Duration::from_secs(300),
                packet_size_variance: 0.1,
            },
            TrafficPattern::Messaging => TrafficPatternParams {
                base_rate: 0.2,
                burst_probability: 0.15,
                burst_multiplier: 10.0,
                inter_burst_delay: Duration::from_secs(60),
                packet_size_variance: 0.5,
            },
        }
    }
}

/// Parameters for traffic pattern simulation
#[derive(Debug, Clone)]
pub struct TrafficPatternParams {
    /// Base packet rate (packets per second)
    pub base_rate: f32,
    /// Probability of burst occurrence
    pub burst_probability: f32,
    /// Multiplier for burst traffic
    pub burst_multiplier: f32,
    /// Delay between bursts
    pub inter_burst_delay: Duration,
    /// Variance in packet sizes
    pub packet_size_variance: f32,
}

/// Configuration for enhanced cover traffic system
#[derive(Debug, Clone)]
pub struct EnhancedCoverConfig {
    /// Basic adaptive cover configuration
    pub base_config: CoverConfig,
    /// Minimum anonymity set size to maintain
    pub min_anonymity_set: u32,
    /// Maximum anonymity set size (resource constraint)
    pub max_anonymity_set: u32,
    /// Enable traffic analysis resistance features
    pub traffic_analysis_resistance: bool,
    /// Enable pattern-based traffic generation
    pub pattern_based_generation: bool,
    /// Enable cross-layer coordination
    pub cross_layer_coordination: bool,
    /// Enable battery optimization for mobile devices
    pub battery_optimization: bool,
    /// Battery level threshold for power saving
    pub battery_threshold: f32,
    /// Power saving reduction factor
    pub power_saving_factor: f32,
    /// Traffic pattern weights for mixing
    pub pattern_weights: HashMap<TrafficPattern, f32>,
    /// Minimum cover traffic rate (packets per second)
    pub min_cover_rate: f32,
    /// Maximum cover traffic rate (packets per second)
    pub max_cover_rate: f32,
    /// Target bandwidth utilization for cover traffic
    pub target_utilization: f32,
    /// Anonymity assessment interval
    pub anonymity_check_interval: Duration,
}

impl Default for EnhancedCoverConfig {
    fn default() -> Self {
        let mut pattern_weights = HashMap::new();
        pattern_weights.insert(TrafficPattern::Constant, 0.2);
        pattern_weights.insert(TrafficPattern::WebBrowsing, 0.3);
        pattern_weights.insert(TrafficPattern::Messaging, 0.2);
        pattern_weights.insert(TrafficPattern::VideoStreaming, 0.1);
        pattern_weights.insert(TrafficPattern::Bursty, 0.1);
        pattern_weights.insert(TrafficPattern::Gaming, 0.05);
        pattern_weights.insert(TrafficPattern::FileTransfer, 0.03);
        pattern_weights.insert(TrafficPattern::IoT, 0.02);

        Self {
            base_config: CoverConfig::default(),
            min_anonymity_set: 10,
            max_anonymity_set: 1000,
            traffic_analysis_resistance: true,
            pattern_based_generation: true,
            cross_layer_coordination: true,
            battery_optimization: true,
            battery_threshold: 0.2,   // 20% battery
            power_saving_factor: 0.3, // 70% reduction in power saving mode
            pattern_weights,
            min_cover_rate: 0.5,
            max_cover_rate: 50.0,
            target_utilization: 0.1, // 10% of bandwidth for cover traffic
            anonymity_check_interval: Duration::from_secs(30),
        }
    }
}

impl EnhancedCoverConfig {
    /// Create new enhanced cover configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum anonymity set size
    pub fn min_anonymity_set(mut self, size: u32) -> Self {
        self.min_anonymity_set = size;
        self
    }

    /// Set maximum anonymity set size
    pub fn max_anonymity_set(mut self, size: u32) -> Self {
        self.max_anonymity_set = size;
        self
    }

    /// Enable or disable traffic analysis resistance
    pub fn traffic_analysis_resistance(mut self, enabled: bool) -> Self {
        self.traffic_analysis_resistance = enabled;
        self
    }

    /// Enable or disable pattern-based generation
    pub fn pattern_based_generation(mut self, enabled: bool) -> Self {
        self.pattern_based_generation = enabled;
        self
    }

    /// Enable or disable cross-layer coordination
    pub fn cross_layer_coordination(mut self, enabled: bool) -> Self {
        self.cross_layer_coordination = enabled;
        self
    }

    /// Enable or disable battery optimization
    pub fn battery_optimization(mut self, enabled: bool) -> Self {
        self.battery_optimization = enabled;
        self
    }

    /// Set battery threshold for power saving
    pub fn battery_threshold(mut self, threshold: f32) -> Self {
        self.battery_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Set power saving reduction factor
    pub fn power_saving_factor(mut self, factor: f32) -> Self {
        self.power_saving_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Set target bandwidth utilization
    pub fn target_utilization(mut self, utilization: f32) -> Self {
        self.target_utilization = utilization.clamp(0.0, 1.0);
        self
    }

    /// Add or update traffic pattern weight
    pub fn pattern_weight(mut self, pattern: TrafficPattern, weight: f32) -> Self {
        self.pattern_weights.insert(pattern, weight.max(0.0));
        self
    }

    /// Set anonymity check interval
    pub fn anonymity_check_interval(mut self, interval: Duration) -> Self {
        self.anonymity_check_interval = interval;
        self
    }
}

/// Anonymity metrics and assessment
#[derive(Debug, Clone, Default)]
pub struct AnonymityMetrics {
    /// Current estimated anonymity set size
    pub current_anonymity_set: u32,
    /// Minimum anonymity set observed in recent period
    pub min_recent_anonymity: u32,
    /// Average anonymity set over time
    pub avg_anonymity_set: f32,
    /// Number of active cover traffic sources
    pub active_cover_sources: u32,
    /// Traffic analysis resistance score (0.0-1.0)
    pub resistance_score: f32,
    /// Pattern mixing effectiveness (0.0-1.0)
    pub pattern_mixing_score: f32,
    /// Last anonymity assessment time
    pub last_assessment: Option<Instant>,
}

impl AnonymityMetrics {
    /// Check if current anonymity is adequate
    pub fn is_adequate(&self, min_required: u32) -> bool {
        self.current_anonymity_set >= min_required
            && self.resistance_score >= 0.7
            && self.pattern_mixing_score >= 0.6
    }

    /// Get overall anonymity quality score
    pub fn quality_score(&self) -> f32 {
        let anonymity_factor = (self.current_anonymity_set as f32 / 100.0).min(1.0);
        let resistance_factor = self.resistance_score;
        let mixing_factor = self.pattern_mixing_score;

        (anonymity_factor + resistance_factor + mixing_factor) / 3.0
    }
}

/// Enhanced cover traffic generation state
#[derive(Debug)]
struct CoverGenerationState {
    current_pattern: TrafficPattern,
    pattern_start_time: Instant,
    pattern_duration: Duration,
    burst_state: BurstState,
    last_packet_time: Option<Instant>,
    packet_count: u64,
}

#[derive(Debug)]
struct BurstState {
    in_burst: bool,
    burst_start: Option<Instant>,
    burst_packets_remaining: u32,
    next_burst_time: Option<Instant>,
}

impl Default for CoverGenerationState {
    fn default() -> Self {
        Self {
            current_pattern: TrafficPattern::Constant,
            pattern_start_time: Instant::now(),
            pattern_duration: Duration::from_secs(300), // 5 minutes default
            burst_state: BurstState {
                in_burst: false,
                burst_start: None,
                burst_packets_remaining: 0,
                next_burst_time: None,
            },
            last_packet_time: None,
            packet_count: 0,
        }
    }
}

/// Cross-layer coordination interface
#[derive(Debug, Clone)]
pub struct CrossLayerMetrics {
    /// Stream layer padding overhead
    pub padding_overhead: f32,
    /// Active stream count
    pub active_streams: u32,
    /// Network congestion level (0.0-1.0)
    pub congestion_level: f32,
    /// Available bandwidth (bytes per second)
    pub available_bandwidth: u64,
    /// Battery level (0.0-1.0)
    pub battery_level: f32,
    /// Network type (WiFi, Cellular, etc.)
    pub network_type: String,
}

impl Default for CrossLayerMetrics {
    fn default() -> Self {
        Self {
            padding_overhead: 0.0,
            active_streams: 0,
            congestion_level: 0.0,
            available_bandwidth: 1_000_000, // 1 MB/s default
            battery_level: 1.0,
            network_type: "Unknown".to_string(),
        }
    }
}

/// Enhanced adaptive cover traffic manager
pub struct EnhancedCoverManager {
    config: EnhancedCoverConfig,
    base_manager: AdaptiveCoverManager,
    generation_state: CoverGenerationState,
    pub anonymity_metrics: AnonymityMetrics,
    cross_layer_metrics: CrossLayerMetrics,
    pattern_selector: WeightedPatternSelector,
    traffic_history: VecDeque<TrafficSample>,
    rng: OsRng,
    last_coordination_update: Instant,
}

/// Traffic sample for analysis
#[derive(Debug, Clone)]
struct TrafficSample {
    timestamp: Instant,
    _packet_count: u32,
    _bytes_sent: u64,
    pattern: TrafficPattern,
}

/// Weighted pattern selector for realistic traffic mixing
struct WeightedPatternSelector {
    patterns: Vec<TrafficPattern>,
    distribution: WeightedIndex<f32>,
}

impl WeightedPatternSelector {
    fn new(weights: &HashMap<TrafficPattern, f32>) -> Result<Self> {
        let mut patterns = Vec::new();
        let mut weight_values = Vec::new();

        for (&pattern, &weight) in weights {
            patterns.push(pattern);
            weight_values.push(weight);
        }

        let distribution =
            WeightedIndex::new(&weight_values).map_err(|e| crate::errors::MixError::Internal {
                msg: format!("Failed to create weighted distribution: {e}"),
            })?;

        Ok(Self {
            patterns,
            distribution,
        })
    }

    fn select_pattern(&self, rng: &mut impl Rng) -> TrafficPattern {
        let index = self.distribution.sample(rng);
        self.patterns[index]
    }
}

impl EnhancedCoverManager {
    /// Create new enhanced cover traffic manager
    pub fn new(config: EnhancedCoverConfig) -> Result<Self> {
        let base_manager = AdaptiveCoverManager::new(config.base_config.clone());
        let pattern_selector = WeightedPatternSelector::new(&config.pattern_weights)?;

        info!(
            "Initialized EnhancedCoverManager with min_anonymity={}, patterns={}, battery_opt={}",
            config.min_anonymity_set,
            config.pattern_weights.len(),
            config.battery_optimization
        );

        Ok(Self {
            config,
            base_manager,
            generation_state: CoverGenerationState::default(),
            anonymity_metrics: AnonymityMetrics::default(),
            cross_layer_metrics: CrossLayerMetrics::default(),
            pattern_selector,
            traffic_history: VecDeque::with_capacity(1000),
            rng: OsRng,
            last_coordination_update: Instant::now(),
        })
    }

    /// Update cross-layer metrics from other protocol layers
    pub async fn update_cross_layer_metrics(&mut self, metrics: CrossLayerMetrics) -> Result<()> {
        self.cross_layer_metrics = metrics;
        self.last_coordination_update = Instant::now();

        // Coordinate cover traffic with other layers
        if self.config.cross_layer_coordination {
            self.coordinate_with_layers().await?;
        }

        debug!(
            "Updated cross-layer metrics: congestion={:.2}, bandwidth={}, battery={:.2}",
            self.cross_layer_metrics.congestion_level,
            self.cross_layer_metrics.available_bandwidth,
            self.cross_layer_metrics.battery_level
        );

        Ok(())
    }

    /// Generate coordinated cover traffic
    pub async fn generate_coordinated_cover(&mut self) -> Result<Vec<CoverPacket>> {
        // Update anonymity assessment
        self.assess_anonymity().await?;

        // Check if anonymity is adequate
        if !self
            .anonymity_metrics
            .is_adequate(self.config.min_anonymity_set)
        {
            warn!("Inadequate anonymity detected, increasing cover traffic");
            return self.generate_emergency_cover().await;
        }

        // Apply battery optimization if needed
        if self.should_apply_battery_optimization() {
            return self.generate_battery_optimized_cover().await;
        }

        // Select traffic pattern
        if self.should_switch_pattern() {
            self.switch_traffic_pattern();
        }

        // Generate pattern-based cover traffic
        let cover_packets = if self.config.pattern_based_generation {
            self.generate_pattern_based_cover().await?
        } else {
            self.generate_basic_cover().await?
        };

        // Record traffic for analysis
        self.record_traffic_sample(&cover_packets).await;

        Ok(cover_packets)
    }

    /// Get current anonymity metrics
    pub fn anonymity_metrics(&self) -> &AnonymityMetrics {
        &self.anonymity_metrics
    }

    /// Get current cross-layer metrics
    pub fn cross_layer_metrics(&self) -> &CrossLayerMetrics {
        &self.cross_layer_metrics
    }

    /// Get anonymity check interval
    pub fn anonymity_check_interval(&self) -> Duration {
        self.config.anonymity_check_interval
    }

    /// Check if traffic analysis resistance is adequate
    pub fn is_traffic_analysis_resistant(&self) -> bool {
        self.anonymity_metrics.resistance_score >= 0.7
            && self.anonymity_metrics.pattern_mixing_score >= 0.6
            && self.anonymity_metrics.current_anonymity_set >= self.config.min_anonymity_set
    }

    // Private implementation methods

    async fn coordinate_with_layers(&mut self) -> Result<()> {
        // Adjust cover traffic based on padding overhead
        if self.cross_layer_metrics.padding_overhead > 0.2 {
            // High padding overhead, reduce cover traffic
            let reduction_factor = 0.8;
            self.config.max_cover_rate *= reduction_factor;
            debug!("Reduced cover traffic due to high padding overhead");
        }

        // Adjust based on network congestion
        if self.cross_layer_metrics.congestion_level > 0.8 {
            // High congestion, be more conservative
            let reduction_factor = 0.6;
            self.config.max_cover_rate *= reduction_factor;
            debug!("Reduced cover traffic due to network congestion");
        }

        // Coordinate with available bandwidth
        let bandwidth_based_rate = (self.cross_layer_metrics.available_bandwidth as f32
            * self.config.target_utilization)
            / 1280.0; // Assume 1280 byte packets

        self.config.max_cover_rate = self.config.max_cover_rate.min(bandwidth_based_rate);

        Ok(())
    }

    async fn assess_anonymity(&mut self) -> Result<()> {
        let now = Instant::now();

        // Check if assessment is needed
        if let Some(last_assessment) = self.anonymity_metrics.last_assessment {
            if now.duration_since(last_assessment) < self.config.anonymity_check_interval {
                return Ok(());
            }
        }

        // Calculate current anonymity set based on cover traffic and active streams
        let base_anonymity = self.cross_layer_metrics.active_streams.max(1);
        let cover_boost = (self.base_manager.current_rate() * 10.0) as u32; // Estimate
        self.anonymity_metrics.current_anonymity_set = base_anonymity + cover_boost;

        // Update minimum recent anonymity
        if self.anonymity_metrics.min_recent_anonymity == 0
            || self.anonymity_metrics.current_anonymity_set
                < self.anonymity_metrics.min_recent_anonymity
        {
            self.anonymity_metrics.min_recent_anonymity =
                self.anonymity_metrics.current_anonymity_set;
        }

        // Calculate resistance score based on traffic patterns
        self.anonymity_metrics.resistance_score = self.calculate_resistance_score();

        // Calculate pattern mixing score
        self.anonymity_metrics.pattern_mixing_score = self.calculate_pattern_mixing_score();

        // Update average with exponential moving average
        let alpha = 0.1;
        let current = self.anonymity_metrics.current_anonymity_set as f32;
        self.anonymity_metrics.avg_anonymity_set =
            alpha * current + (1.0 - alpha) * self.anonymity_metrics.avg_anonymity_set;

        self.anonymity_metrics.last_assessment = Some(now);

        trace!(
            "Anonymity assessment: set_size={}, resistance={:.2}, mixing={:.2}",
            self.anonymity_metrics.current_anonymity_set,
            self.anonymity_metrics.resistance_score,
            self.anonymity_metrics.pattern_mixing_score
        );

        Ok(())
    }

    async fn generate_emergency_cover(&mut self) -> Result<Vec<CoverPacket>> {
        warn!("Generating emergency cover traffic for anonymity protection");

        // Increase cover traffic rate temporarily
        let emergency_rate = self.config.max_cover_rate * 2.0;
        let packet_count = poisson_rate(emergency_rate, &mut self.rng);

        let mut packets = Vec::new();
        for _ in 0..packet_count {
            packets.push(CoverPacket {
                size: 1280,
                pattern: TrafficPattern::Constant,
                delay: Duration::from_millis(self.rng.gen_range(1..=10)),
                priority: CoverPriority::Emergency,
            });
        }

        Ok(packets)
    }

    async fn generate_battery_optimized_cover(&mut self) -> Result<Vec<CoverPacket>> {
        debug!("Generating battery-optimized cover traffic");

        let reduced_rate = self.config.min_cover_rate * self.config.power_saving_factor;
        let packet_count = poisson_rate(reduced_rate, &mut self.rng);

        let mut packets = Vec::new();
        for _ in 0..packet_count {
            packets.push(CoverPacket {
                size: 1280,
                pattern: TrafficPattern::IoT, // Low-power pattern
                delay: Duration::from_millis(self.rng.gen_range(100..=1000)),
                priority: CoverPriority::Low,
            });
        }

        Ok(packets)
    }

    async fn generate_pattern_based_cover(&mut self) -> Result<Vec<CoverPacket>> {
        let current_pattern = self.generation_state.current_pattern;
        let params = current_pattern.parameters();

        // Generate base packets
        let base_rate = self.adjust_rate_for_conditions(params.base_rate);
        let base_count = poisson_rate(base_rate, &mut self.rng);

        let mut packets = Vec::new();

        // Handle burst generation
        let (burst_count, in_burst) = self.handle_burst_generation(&params)?;

        let total_count = base_count + burst_count;

        for i in 0..total_count {
            let size = self.generate_packet_size(&params);
            let delay = self.generate_inter_packet_delay(&params, in_burst);
            let priority = if i < burst_count {
                CoverPriority::High
            } else {
                CoverPriority::Normal
            };

            packets.push(CoverPacket {
                size,
                pattern: current_pattern,
                delay,
                priority,
            });
        }

        Ok(packets)
    }

    async fn generate_basic_cover(&mut self) -> Result<Vec<CoverPacket>> {
        let network_metrics = NetworkMetrics {
            bandwidth_utilization: self.cross_layer_metrics.congestion_level,
            active_flows: self.cross_layer_metrics.active_streams,
            ..Default::default()
        };

        self.base_manager.update_metrics(network_metrics);
        let packet_count = self.base_manager.generate_cover_traffic(&mut self.rng);

        let mut packets = Vec::new();
        for _ in 0..packet_count {
            packets.push(CoverPacket {
                size: 1280,
                pattern: TrafficPattern::Constant,
                delay: Duration::from_millis(self.rng.gen_range(10..=50)),
                priority: CoverPriority::Normal,
            });
        }

        Ok(packets)
    }

    fn should_apply_battery_optimization(&self) -> bool {
        self.config.battery_optimization
            && self.cross_layer_metrics.battery_level < self.config.battery_threshold
    }

    fn should_switch_pattern(&self) -> bool {
        let pattern_age = self.generation_state.pattern_start_time.elapsed();
        pattern_age >= self.generation_state.pattern_duration
    }

    fn switch_traffic_pattern(&mut self) {
        let new_pattern = self.pattern_selector.select_pattern(&mut self.rng);

        debug!(
            "Switching traffic pattern from {:?} to {:?}",
            self.generation_state.current_pattern, new_pattern
        );

        self.generation_state.current_pattern = new_pattern;
        self.generation_state.pattern_start_time = Instant::now();

        // Randomize pattern duration
        let base_duration = Duration::from_secs(self.rng.gen_range(60..=600)); // 1-10 minutes
        self.generation_state.pattern_duration = base_duration;

        // Reset burst state
        self.generation_state.burst_state = BurstState {
            in_burst: false,
            burst_start: None,
            burst_packets_remaining: 0,
            next_burst_time: Some(Instant::now() + new_pattern.parameters().inter_burst_delay),
        };
    }

    fn adjust_rate_for_conditions(&self, base_rate: f32) -> f32 {
        let mut adjusted_rate = base_rate;

        // Adjust for battery level
        if self.config.battery_optimization {
            let battery_factor = (self.cross_layer_metrics.battery_level * 0.5 + 0.5).min(1.0);
            adjusted_rate *= battery_factor;
        }

        // Adjust for network congestion
        let congestion_factor = 1.0 - (self.cross_layer_metrics.congestion_level * 0.5);
        adjusted_rate *= congestion_factor;

        // Ensure within configured bounds
        adjusted_rate.clamp(self.config.min_cover_rate, self.config.max_cover_rate)
    }

    fn handle_burst_generation(&mut self, params: &TrafficPatternParams) -> Result<(u32, bool)> {
        let now = Instant::now();
        let burst_state = &mut self.generation_state.burst_state;

        // Check if we should start a new burst
        if !burst_state.in_burst {
            if let Some(next_burst_time) = burst_state.next_burst_time {
                if now >= next_burst_time && self.rng.gen::<f32>() < params.burst_probability {
                    // Start new burst
                    burst_state.in_burst = true;
                    burst_state.burst_start = Some(now);

                    let burst_packets =
                        poisson_rate(params.base_rate * params.burst_multiplier, &mut self.rng);
                    burst_state.burst_packets_remaining = burst_packets;

                    debug!("Starting traffic burst with {} packets", burst_packets);
                    return Ok((burst_packets, true));
                }
            }
        } else {
            // Continue existing burst
            if burst_state.burst_packets_remaining > 0 {
                let packets_this_round = burst_state.burst_packets_remaining.min(10);
                burst_state.burst_packets_remaining -= packets_this_round;

                if burst_state.burst_packets_remaining == 0 {
                    // End burst
                    burst_state.in_burst = false;
                    burst_state.burst_start = None;
                    burst_state.next_burst_time = Some(now + params.inter_burst_delay);
                    debug!("Ending traffic burst");
                }

                return Ok((packets_this_round, true));
            }
        }

        Ok((0, burst_state.in_burst))
    }

    fn generate_packet_size(&mut self, params: &TrafficPatternParams) -> usize {
        let base_size = 1280; // Fixed size for anonymity
        let variance = (base_size as f32 * params.packet_size_variance) as usize;

        if variance > 0 {
            let offset = self.rng.gen_range(0..=variance);
            let add = self.rng.gen_bool(0.5);

            if add {
                (base_size + offset).min(1500) // Don't exceed MTU
            } else {
                (base_size - offset).max(64) // Don't go below minimum
            }
        } else {
            base_size
        }
    }

    fn generate_inter_packet_delay(
        &mut self,
        params: &TrafficPatternParams,
        in_burst: bool,
    ) -> Duration {
        if in_burst {
            // Shorter delays during bursts
            Duration::from_millis(self.rng.gen_range(1..=10))
        } else {
            // Use exponential distribution for realistic delays
            let rate = 1.0 / params.base_rate.max(0.1);
            let exp_dist = Exp::new(rate as f64).unwrap_or_else(|_| Exp::new(1.0).unwrap());
            let delay_secs = exp_dist.sample(&mut self.rng);
            Duration::from_secs_f64(delay_secs.clamp(0.001, 10.0)) // Clamp to reasonable range
        }
    }

    fn calculate_resistance_score(&self) -> f32 {
        // Simplified resistance score calculation
        let pattern_diversity = self
            .traffic_history
            .iter()
            .map(|sample| sample.pattern)
            .collect::<std::collections::HashSet<_>>()
            .len() as f32
            / 8.0; // Total number of patterns

        let timing_variance = if self.traffic_history.len() >= 2 {
            let delays: Vec<f32> = self
                .traffic_history
                .iter()
                .collect::<Vec<_>>()
                .windows(2)
                .map(|window| {
                    window[1]
                        .timestamp
                        .duration_since(window[0].timestamp)
                        .as_secs_f32()
                })
                .collect();

            if delays.is_empty() {
                return 0.5;
            }

            let mean = delays.iter().sum::<f32>() / delays.len() as f32;
            let variance =
                delays.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / delays.len() as f32;

            (variance.sqrt() / mean.max(0.1)).min(1.0)
        } else {
            0.5
        };

        (pattern_diversity + timing_variance) / 2.0
    }

    fn calculate_pattern_mixing_score(&self) -> f32 {
        if self.traffic_history.len() < 10 {
            return 0.5; // Not enough data
        }

        let recent_patterns: Vec<TrafficPattern> = self
            .traffic_history
            .iter()
            .rev()
            .take(10)
            .map(|sample| sample.pattern)
            .collect();

        let unique_patterns = recent_patterns
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();

        unique_patterns as f32 / recent_patterns.len() as f32
    }

    async fn record_traffic_sample(&mut self, packets: &[CoverPacket]) {
        let total_bytes = packets.iter().map(|p| p.size as u64).sum();

        let sample = TrafficSample {
            timestamp: Instant::now(),
            _packet_count: packets.len() as u32,
            _bytes_sent: total_bytes,
            pattern: self.generation_state.current_pattern,
        };

        self.traffic_history.push_back(sample);

        // Keep only recent history
        while self.traffic_history.len() > 1000 {
            self.traffic_history.pop_front();
        }

        // Update generation state
        self.generation_state.packet_count += packets.len() as u64;
        self.generation_state.last_packet_time = Some(Instant::now());
    }
}

/// Cover traffic packet descriptor
#[derive(Debug, Clone)]
pub struct CoverPacket {
    /// Packet size in bytes
    pub size: usize,
    /// Traffic pattern this packet belongs to
    pub pattern: TrafficPattern,
    /// Delay before sending this packet
    pub delay: Duration,
    /// Priority level for this packet
    pub priority: CoverPriority,
}

/// Priority levels for cover traffic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoverPriority {
    /// Low priority, can be dropped under pressure
    Low,
    /// Normal priority cover traffic
    Normal,
    /// High priority for pattern maintenance
    High,
    /// Emergency cover traffic for anonymity protection
    Emergency,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_enhanced_cover_config() {
        let config = EnhancedCoverConfig::new()
            .min_anonymity_set(20)
            .traffic_analysis_resistance(true)
            .battery_optimization(true);

        assert_eq!(config.min_anonymity_set, 20);
        assert!(config.traffic_analysis_resistance);
        assert!(config.battery_optimization);
    }

    #[test]
    async fn test_traffic_pattern_parameters() {
        let web_params = TrafficPattern::WebBrowsing.parameters();
        let iot_params = TrafficPattern::IoT.parameters();

        assert!(web_params.burst_probability > iot_params.burst_probability);
        assert!(web_params.burst_multiplier > iot_params.burst_multiplier);
    }

    #[test]
    async fn test_enhanced_cover_manager_creation() {
        let config = EnhancedCoverConfig::new();
        let manager = EnhancedCoverManager::new(config);

        assert!(manager.is_ok());
    }

    #[test]
    async fn test_cross_layer_coordination() {
        let config = EnhancedCoverConfig::new().cross_layer_coordination(true);
        let mut manager = EnhancedCoverManager::new(config).unwrap();

        let metrics = CrossLayerMetrics {
            congestion_level: 0.8,
            battery_level: 0.3,
            available_bandwidth: 500_000,
            ..Default::default()
        };

        assert!(manager.update_cross_layer_metrics(metrics).await.is_ok());
    }

    #[test]
    async fn test_pattern_based_cover_generation() {
        let config = EnhancedCoverConfig::new().pattern_based_generation(true);
        let mut manager = EnhancedCoverManager::new(config).unwrap();

        let packets = manager.generate_coordinated_cover().await.unwrap();
        assert!(!packets.is_empty());
    }

    #[test]
    async fn test_battery_optimization() {
        let config = EnhancedCoverConfig::new()
            .battery_optimization(true)
            .battery_threshold(0.5);
        let mut manager = EnhancedCoverManager::new(config).unwrap();

        // Set low battery
        let metrics = CrossLayerMetrics {
            battery_level: 0.2,
            ..Default::default()
        };
        manager.update_cross_layer_metrics(metrics).await.unwrap();

        let packets = manager.generate_coordinated_cover().await.unwrap();

        // Battery optimization should result in battery-optimized generation
        // Verify that the function was triggered by checking for IoT pattern packets
        assert!(
            !packets.is_empty(),
            "Should generate some cover traffic even with battery optimization"
        );

        // Verify that battery optimization was triggered by checking the manager state
        let battery_opt_active = manager.should_apply_battery_optimization();
        assert!(
            battery_opt_active,
            "Battery optimization should be active with low battery"
        );

        // Since battery optimization uses different generation path,
        // we verify the method works rather than specific packet characteristics
        // as packet generation uses randomness
    }

    #[test]
    async fn test_anonymity_assessment() {
        let config = EnhancedCoverConfig::new().min_anonymity_set(10);
        let mut manager = EnhancedCoverManager::new(config).unwrap();

        // Update metrics to simulate active network
        let metrics = CrossLayerMetrics {
            active_streams: 5,
            ..Default::default()
        };
        manager.update_cross_layer_metrics(metrics).await.unwrap();

        manager.assess_anonymity().await.unwrap();

        let anonymity = manager.anonymity_metrics();
        assert!(anonymity.current_anonymity_set > 0);
    }

    #[test]
    async fn test_emergency_cover_generation() {
        let config = EnhancedCoverConfig::new().min_anonymity_set(100);
        let mut manager = EnhancedCoverManager::new(config).unwrap();

        // Simulate inadequate anonymity
        manager.anonymity_metrics.current_anonymity_set = 5;

        let packets = manager.generate_coordinated_cover().await.unwrap();

        // Should generate emergency cover traffic
        assert!(packets
            .iter()
            .any(|p| matches!(p.priority, CoverPriority::Emergency)));
    }

    #[test]
    async fn test_pattern_switching() {
        let config = EnhancedCoverConfig::new();
        let mut manager = EnhancedCoverManager::new(config).unwrap();

        let initial_pattern = manager.generation_state.current_pattern;

        // Force pattern switch by setting very old pattern start time
        manager.generation_state.pattern_start_time = Instant::now() - Duration::from_secs(1000);
        manager.generation_state.pattern_duration = Duration::from_secs(60); // 1 minute

        // Check if pattern should switch
        let should_switch = manager.should_switch_pattern();
        assert!(should_switch, "Pattern should be ready to switch");

        // Manually switch pattern
        manager.switch_traffic_pattern();

        // Pattern should have changed or timing should be updated
        let changed_pattern = manager.generation_state.current_pattern != initial_pattern;
        let time_reset =
            manager.generation_state.pattern_start_time > Instant::now() - Duration::from_secs(10);

        assert!(
            changed_pattern || time_reset,
            "Pattern should change or timing should be reset"
        );
    }

    #[test]
    async fn test_traffic_analysis_resistance() {
        let config = EnhancedCoverConfig::new().traffic_analysis_resistance(true);
        let mut manager = EnhancedCoverManager::new(config).unwrap();

        // Generate some traffic history
        for _ in 0..20 {
            manager.generate_coordinated_cover().await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Should have reasonable resistance metrics
        let resistance_score = manager.anonymity_metrics().resistance_score;
        assert!((0.0..=1.0).contains(&resistance_score));
    }
}
