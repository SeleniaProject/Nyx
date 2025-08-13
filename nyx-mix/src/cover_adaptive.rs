//! Adaptive Cover Traffic Generator with Mobile Power State Integration
//!
//! This module implements an advanced cover traffic system that adapts to:
//! - Mobile device power states (battery level, charging status, screen state)
//! - Network conditions and bandwidth availability
//! - User activity patterns and application state
//! - Time-of-day and usage patterns
//!
//! The system uses Poisson distribution for traffic generation with dynamic
//! lambda (λ) parameter adjustment based on real-time conditions.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::collections::VecDeque;
use tokio::sync::{RwLock, broadcast, watch};
use tokio::time::{interval, sleep_until, Instant as TokioInstant};
use serde::{Serialize, Deserialize};
use tracing::{debug, info, trace};
use rand::Rng;
use crate::cover::CoverGenerator;

// Import mobile types
use nyx_core::mobile::{PowerProfile, NetworkState, AppState};

/// Configuration for adaptive cover traffic generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AdaptiveCoverConfig {
    /// Base lambda value for Poisson distribution (packets per second)
    pub base_lambda: f64,
    /// Maximum lambda value to prevent excessive traffic
    pub max_lambda: f64,
    /// Minimum lambda value to maintain basic cover
    pub min_lambda: f64,
    /// Target bandwidth utilization (0.0 to 1.0)
    pub target_utilization: f64,
    /// Adaptation speed (0.0 to 1.0, higher = faster adaptation)
    pub adaptation_speed: f64,
    /// Time window for traffic analysis (seconds)
    pub analysis_window: u64,
    /// Target cover ratio C/(C+R) in 0.0..=1.0 (e.g. 0.35 = 35% cover)
    pub target_cover_ratio: f64,
    /// Exponential moving average beta for ratio smoothing (0..1, higher = smoother)
    pub ema_beta: f64,
    /// Proportional gain for ratio error to lambda update (α in spec)
    pub alpha_gain: f64,
    /// Heartbeat bump interval (seconds) when long-term real traffic is nearly zero
    pub heartbeat_bump_sec: u64,
    /// Exponential decay factor k for low-power transitions
    pub low_power_decay_k: f64,
    /// Enable mobile power state adaptation
    pub mobile_adaptation: bool,
    /// Enable time-based adaptation
    pub time_based_adaptation: bool,
    /// Enable network condition adaptation
    pub network_adaptation: bool,
    /// (Test/advanced) Override multiplier applied to base lambda before other scaling
    pub manual_scale: Option<f64>,
    /// Adaptation interval in milliseconds (default 1000, can be shortened for tests)
    pub adaptation_interval_ms: u64,
}

impl Default for AdaptiveCoverConfig {
    fn default() -> Self {
        Self {
            base_lambda: 2.0,           // 2 packets/second base rate
            max_lambda: 10.0,           // Max 10 packets/second
            min_lambda: 0.1,            // Min 0.1 packets/second
            target_utilization: 0.35,   // deprecated semantic; kept for compat
            target_cover_ratio: 0.35,   // 35% cover share target
            adaptation_speed: 0.1,      // Moderate adaptation speed
            analysis_window: 60,        // 1 minute analysis window
            ema_beta: 0.3,              // Ratio smoothing beta
            alpha_gain: 0.5,            // Control proportional gain
            heartbeat_bump_sec: 60,     // Periodic micro bump when idle
            low_power_decay_k: 0.18,    // ~10-15s to 80% convergence
            mobile_adaptation: true,
            time_based_adaptation: true,
            network_adaptation: true,
            manual_scale: None,
            adaptation_interval_ms: 1000,
        }
    }
}

/// Real-time metrics for cover traffic generation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CoverTrafficMetrics {
    /// Current lambda value
    pub current_lambda: f64,
    /// Packets generated in last window
    pub packets_generated: u64,
    /// Actual traffic rate (packets/second)
    pub actual_rate: f64,
    /// Target traffic rate (packets/second)
    pub target_rate: f64,
    /// Power profile scaling factor
    pub power_scale: f64,
    /// Network condition scaling factor
    pub network_scale: f64,
    /// Time-based scaling factor
    pub time_scale: f64,
    /// Total bandwidth used (bytes/second)
    pub bandwidth_used: u64,
    /// Adaptation efficiency (0.0 to 1.0)
    pub efficiency: f64,
}

/// Historical data point for traffic analysis.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TrafficSample {
    timestamp: Instant,
    lambda: f64,
    packets_sent: u64,
    bandwidth_used: u64,
    power_state: Option<PowerProfile>,
}

/// Adaptive cover traffic generator with real-time lambda scaling.
pub struct AdaptiveCoverGenerator {
    /// Configuration
    config: Arc<RwLock<AdaptiveCoverConfig>>,
    /// Current lambda value
    current_lambda: Arc<RwLock<f64>>,
    /// Traffic generation metrics
    metrics: Arc<RwLock<CoverTrafficMetrics>>,
    /// Historical traffic samples
    history: Arc<RwLock<VecDeque<TrafficSample>>>,
    /// Metrics broadcast channel
    metrics_tx: broadcast::Sender<CoverTrafficMetrics>,
    /// Lambda change notifications
    lambda_tx: watch::Sender<f64>,
    /// Last packet generation time
    last_packet_time: Arc<RwLock<Instant>>,
    /// Running statistics
    stats: Arc<RwLock<TrafficStats>>,
    /// Sliding window of generated cover bytes
    cover_bytes_window: Arc<RwLock<SlidingWindow>>, 
    /// Sliding window of real bytes recorded by upper layers
    real_bytes_window: Arc<RwLock<SlidingWindow>>, 
    /// Smoothed cover ratio (EMA of C/(C+R))
    cover_ratio_ema: Arc<RwLock<f64>>,
    /// Last time real bytes were recorded (for heartbeat bump)
    last_real_activity: Arc<RwLock<Instant>>,
}

/// Running statistics for traffic analysis.
#[derive(Debug, Clone)]
pub struct TrafficStats {
    total_packets: u64,
    total_bytes: u64,
    total_runtime: Duration,
    average_lambda: f64,
    peak_lambda: f64,
    min_lambda: f64,
    adaptation_count: u64,
}

impl Default for TrafficStats {
    fn default() -> Self {
        Self {
            total_packets: 0,
            total_bytes: 0,
            total_runtime: Duration::ZERO,
            average_lambda: 0.0,
            peak_lambda: 0.0,
            min_lambda: f64::INFINITY,
            adaptation_count: 0,
        }
    }
}

impl AdaptiveCoverGenerator {
    /// Create a new adaptive cover traffic generator.
    pub fn new(config: AdaptiveCoverConfig) -> Self {
        let (metrics_tx, _) = broadcast::channel(32);
        let (lambda_tx, _) = watch::channel(config.base_lambda);

        let initial_metrics = CoverTrafficMetrics {
            current_lambda: config.base_lambda,
            packets_generated: 0,
            actual_rate: 0.0,
            target_rate: config.base_lambda,
            power_scale: 1.0,
            network_scale: 1.0,
            time_scale: 1.0,
            bandwidth_used: 0,
            efficiency: 1.0,
        };

        let base_lambda = config.base_lambda;
        let analysis_len = Duration::from_secs(config.analysis_window.max(1));
        Self {
            config: Arc::new(RwLock::new(config)),
            current_lambda: Arc::new(RwLock::new(base_lambda)),
            metrics: Arc::new(RwLock::new(initial_metrics)),
            history: Arc::new(RwLock::new(VecDeque::new())),
            metrics_tx,
            lambda_tx,
            last_packet_time: Arc::new(RwLock::new(Instant::now())),
            stats: Arc::new(RwLock::new(TrafficStats::default())),
            cover_bytes_window: Arc::new(RwLock::new(SlidingWindow::new(analysis_len))),
            real_bytes_window: Arc::new(RwLock::new(SlidingWindow::new(analysis_len))),
            cover_ratio_ema: Arc::new(RwLock::new(0.0)),
            last_real_activity: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Start the adaptive cover traffic generator.
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting adaptive cover traffic generator");

        // Start adaptation engine
        self.start_adaptation_engine().await;

        // Start metrics collection
        self.start_metrics_collection().await;

        // Start traffic generation
        self.start_traffic_generation().await;

        Ok(())
    }

    /// Record number of real (non-cover) bytes sent by upper layers.
    /// This updates the utilization estimator used by the control loop.
    pub async fn record_real_bytes(&self, bytes: usize) {
        {
            let mut w = self.real_bytes_window.write().await;
            w.record(bytes);
        }
        let mut last = self.last_real_activity.write().await;
        *last = Instant::now();
    }

    /// Get current traffic metrics.
    pub async fn metrics(&self) -> CoverTrafficMetrics {
        *self.metrics.read().await
    }

    /// Subscribe to metrics updates.
    pub fn subscribe_metrics(&self) -> broadcast::Receiver<CoverTrafficMetrics> {
        self.metrics_tx.subscribe()
    }

    /// Subscribe to lambda changes.
    pub fn subscribe_lambda(&self) -> watch::Receiver<f64> {
        self.lambda_tx.subscribe()
    }

    /// Update configuration at runtime.
    pub async fn update_config(&self, new_config: AdaptiveCoverConfig) {
        let mut config = self.config.write().await;
        *config = new_config;
        info!("Cover traffic configuration updated");
    }

    /// Get current lambda value.
    pub async fn current_lambda(&self) -> f64 {
        *self.current_lambda.read().await
    }

    /// Get traffic statistics.
    pub async fn statistics(&self) -> TrafficStats {
        self.stats.read().await.clone()
    }

    /// Start the adaptation engine that adjusts lambda based on conditions.
    async fn start_adaptation_engine(&self) {
        let config = Arc::clone(&self.config);
        let current_lambda = Arc::clone(&self.current_lambda);
        let metrics = Arc::clone(&self.metrics);
        let _history = Arc::clone(&self.history);
        let lambda_tx = self.lambda_tx.clone();
        let metrics_tx = self.metrics_tx.clone();
        let stats = Arc::clone(&self.stats);
        let cover_bytes_window = Arc::clone(&self.cover_bytes_window);
        let real_bytes_window = Arc::clone(&self.real_bytes_window);
        let cover_ratio_ema = Arc::clone(&self.cover_ratio_ema);
        let last_real_activity = Arc::clone(&self.last_real_activity);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_millis(config.read().await.adaptation_interval_ms));
            // Track low-power decay state and previous power scale to apply smooth transition
            let mut last_power_scale: f64 = 1.0;
            let mut decay_state: Option<(Instant, f64, f64)> = None; // (start, from, to)

            loop {
                interval.tick().await;

                let cfg = config.read().await.clone();
                let mut lambda = current_lambda.write().await;
                let mut metrics_guard = metrics.write().await;
                let mut stats_guard = stats.write().await;

                // Calculate scaling factors
                let power_scale = Self::calculate_power_scale(&cfg).await;
                let network_scale = Self::calculate_network_scale(&cfg).await;
                let time_scale = Self::calculate_time_scale(&cfg).await;

                // Base feedforward lambda before feedback (ratio control)
                let manual = cfg.manual_scale.unwrap_or(1.0);
                let mut ff_lambda = cfg.base_lambda * manual * power_scale * network_scale * time_scale;

                // Apply low-power exponential decay when power scale drops significantly
                if power_scale + 1e-6 < last_power_scale {
                    decay_state = Some((Instant::now(), *lambda, ff_lambda));
                } else if power_scale > last_power_scale + 1e-6 {
                    // Clear decay when exiting low power
                    decay_state = None;
                }
                last_power_scale = power_scale;

                if let Some((start, from, to)) = decay_state {
                    let t = Instant::now().saturating_duration_since(start).as_secs_f64();
                    let k = cfg.low_power_decay_k.max(0.01).min(1.0);
                    // λ_decay(t) = from*e^{-k t} + to*(1 - e^{-k t})
                    let e = (-k * t).exp();
                    ff_lambda = from * e + to * (1.0 - e);
                }

                // Ensure analysis window duration is respected by sliding windows
                {
                    let desired = Duration::from_secs(cfg.analysis_window.max(1));
                    cover_bytes_window.write().await.set_window_len(desired);
                    real_bytes_window.write().await.set_window_len(desired);
                }

                // Compute cover ratio over the window: C/(C+R)
                let c_bytes = cover_bytes_window.write().await.total_bytes();
                let r_bytes = real_bytes_window.write().await.total_bytes();
                let total_bytes = c_bytes + r_bytes;
                let target_cover = cfg.target_cover_ratio.clamp(0.0, 1.0);
                // If we have no observations yet, assume ratio at target to avoid biasing control
                let cover_ratio_raw = if total_bytes == 0 {
                    target_cover
                } else {
                    c_bytes as f64 / (total_bytes as f64)
                };

                // Update EMA of cover ratio
                {
                    let mut ema = cover_ratio_ema.write().await;
                    let beta = cfg.ema_beta.clamp(0.0, 1.0);
                    if *ema == 0.0 { *ema = cover_ratio_raw; }
                    else { *ema = *ema * (1.0 - beta) + cover_ratio_raw * beta; }
                }
                let cover_ratio_smoothed = *cover_ratio_ema.read().await;

                // Ratio error: target - achieved
                let e_ratio = target_cover - cover_ratio_smoothed;

                // Feedback update: λ_new = clamp(λ_old * (1 + α * e), min, max)
                let alpha = if e_ratio >= 0.0 { cfg.alpha_gain } else { cfg.alpha_gain * 0.7 };
                let fb_lambda = (*lambda * (1.0 + alpha * e_ratio)).clamp(cfg.min_lambda, cfg.max_lambda);

                // If caller requested immediate adaptation, converge to feedforward target directly.
                let mut new_lambda = if cfg.adaptation_speed >= 1.0 {
                    ff_lambda.clamp(cfg.min_lambda, cfg.max_lambda)
                } else {
                    // Blend feedforward and feedback for stability
                    // Move a fraction toward ff_lambda while respecting bounds
                    let toward_ff = *lambda + (ff_lambda - *lambda) * cfg.adaptation_speed.clamp(0.0, 1.0);
                    let blended = 0.5 * fb_lambda + 0.5 * toward_ff;
                    blended.clamp(cfg.min_lambda, cfg.max_lambda)
                };

                // Heartbeat bump if no real activity for prolonged time
                let idle_for = Instant::now().saturating_duration_since(*last_real_activity.read().await);
                if idle_for.as_secs() >= cfg.heartbeat_bump_sec && new_lambda < (cfg.min_lambda * 1.5) {
                    new_lambda = (cfg.min_lambda * 1.5).min(cfg.max_lambda);
                }

                if (*lambda - new_lambda).abs() > 0.01 {
                    *lambda = new_lambda;
                    stats_guard.adaptation_count += 1;
                    debug!(
                        "Lambda adapted: {:.3} (ff: {:.3}, power: {:.2}, network: {:.2}, time: {:.2}, cover_ratio_ema: {:.3}, e: {:.3})",
                        new_lambda, ff_lambda, power_scale, network_scale, time_scale, cover_ratio_smoothed, e_ratio
                    );
                    let _ = lambda_tx.send(new_lambda);
                }

                // Update metrics
                metrics_guard.current_lambda = *lambda;
                metrics_guard.target_rate = ff_lambda.clamp(cfg.min_lambda, cfg.max_lambda);
                metrics_guard.power_scale = power_scale;
                metrics_guard.network_scale = network_scale;
                metrics_guard.time_scale = time_scale;

                // Calculate efficiency
                let efficiency = if metrics_guard.target_rate > 0.0 {
                    (metrics_guard.actual_rate / metrics_guard.target_rate).min(1.0)
                } else {
                    1.0
                };
                metrics_guard.efficiency = efficiency;

                // Update statistics
                stats_guard.average_lambda = (stats_guard.average_lambda * (stats_guard.adaptation_count as f64 - 1.0) + *lambda) / stats_guard.adaptation_count as f64;
                stats_guard.peak_lambda = stats_guard.peak_lambda.max(*lambda);
                stats_guard.min_lambda = stats_guard.min_lambda.min(*lambda);

                let _ = metrics_tx.send(*metrics_guard);
            }
        });
    }

    /// Start metrics collection and analysis.
    async fn start_metrics_collection(&self) {
        let history = Arc::clone(&self.history);
        let metrics = Arc::clone(&self.metrics);
        let config = Arc::clone(&self.config);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                let cfg = config.read().await;
                let window_size = cfg.analysis_window;
                drop(cfg);

                let now = Instant::now();
                let mut history_guard = history.write().await;
                let mut metrics_guard = metrics.write().await;

                // Clean old samples
                while let Some(sample) = history_guard.front() {
                    if now.duration_since(sample.timestamp).as_secs() > window_size {
                        history_guard.pop_front();
                    } else {
                        break;
                    }
                }

                // Calculate metrics from recent history
                if !history_guard.is_empty() {
                    let total_packets: u64 = history_guard.iter().map(|s| s.packets_sent).sum();
                    let total_bandwidth: u64 = history_guard.iter().map(|s| s.bandwidth_used).sum();
                    let window_duration = window_size as f64;

                    metrics_guard.packets_generated = total_packets;
                    metrics_guard.actual_rate = total_packets as f64 / window_duration;
                    metrics_guard.bandwidth_used = (total_bandwidth as f64 / window_duration) as u64;
                }
            }
        });
    }

    /// Start the actual traffic generation loop.
    async fn start_traffic_generation(&self) {
        let current_lambda = Arc::clone(&self.current_lambda);
        let history = Arc::clone(&self.history);
        let last_packet_time = Arc::clone(&self.last_packet_time);
        let stats = Arc::clone(&self.stats);
        let cover_bytes_window = Arc::clone(&self.cover_bytes_window);

        tokio::spawn(async move {
            let start_time = Instant::now();

            loop {
                let lambda = *current_lambda.read().await;
                // Use exponential inter-arrival with mean 1/λ to generate Poisson process
                let gen = CoverGenerator::new(lambda.max(0.0001));
                let delay = gen.next_delay().max(Duration::from_millis(50));

                let next_packet_time = TokioInstant::now() + delay;
                sleep_until(next_packet_time).await;

                // Generate cover packet
                let packet_size = {
                    let mut rng = rand::thread_rng();
                    Self::generate_packet_size(&mut rng)
                };
                
                // Update history
                let sample = TrafficSample {
                    timestamp: Instant::now(),
                    lambda,
                    packets_sent: 1,
                    bandwidth_used: packet_size as u64,
                    power_state: Self::get_current_power_profile().await,
                };

                history.write().await.push_back(sample);
                *last_packet_time.write().await = Instant::now();

                // Update sliding window for cover bytes
                cover_bytes_window.write().await.record(packet_size);

                // Update statistics
                let mut stats_guard = stats.write().await;
                stats_guard.total_packets += 1;
                stats_guard.total_bytes += packet_size as u64;
                stats_guard.total_runtime = start_time.elapsed();

                trace!("Generated cover packet: {} bytes, lambda: {:.3}", packet_size, lambda);
            }
        });
    }

    /// Calculate power state scaling factor.
    async fn calculate_power_scale(config: &AdaptiveCoverConfig) -> f64 {
        if !config.mobile_adaptation {
            return 1.0;
        }

        #[cfg(feature = "mobile")]
        {
            if let Some(monitor) = nyx_core::mobile::mobile_monitor() {
                let power_state = monitor.power_state().await;
                return power_state.power_profile.cover_traffic_scale();
            }
        }

        1.0
    }

    /// Calculate network condition scaling factor.
    async fn calculate_network_scale(config: &AdaptiveCoverConfig) -> f64 {
        if !config.network_adaptation {
            return 1.0;
        }

        #[cfg(feature = "mobile")]
        {
            if let Some(monitor) = nyx_core::mobile::mobile_monitor() {
                let network_state = monitor.network_state().await;
                return match network_state {
                    NetworkState::WiFi => 1.0,
                    NetworkState::Cellular => 0.6,
                    NetworkState::Ethernet => 1.2,
                    NetworkState::None => 0.0,
                };
            }
        }

        1.0
    }

    /// Calculate time-based scaling factor.
    async fn calculate_time_scale(config: &AdaptiveCoverConfig) -> f64 {
        if !config.time_based_adaptation {
            return 1.0;
        }

        // Get current time of day
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let hour = (now / 3600) % 24;
        
        // Scale based on typical usage patterns
        match hour {
            0..=6 => 0.3,   // Night: reduced activity
            7..=9 => 0.8,   // Morning: moderate activity
            10..=17 => 1.0, // Day: normal activity
            18..=21 => 1.2, // Evening: peak activity
            22..=23 => 0.6, // Late evening: reduced activity
            _ => 1.0,
        }
    }

    /// Get current power profile from mobile monitor.
    async fn get_current_power_profile() -> Option<PowerProfile> {
        #[cfg(feature = "mobile")]
        {
            if let Some(monitor) = nyx_core::mobile::mobile_monitor() {
                let power_state = monitor.power_state().await;
                return Some(power_state.power_profile);
            }
        }
        None
    }

    /// Generate realistic packet size for cover traffic.
    fn generate_packet_size(rng: &mut impl Rng) -> usize {
        // Generate realistic packet sizes based on typical network traffic
        let size_class: f64 = rng.gen();
        
        if size_class < 0.4 {
            // Small packets (40-200 bytes) - 40%
            rng.gen_range(40..=200)
        } else if size_class < 0.7 {
            // Medium packets (200-800 bytes) - 30%
            rng.gen_range(200..=800)
        } else if size_class < 0.9 {
            // Large packets (800-1280 bytes) - 20%
            rng.gen_range(800..=1280)
        } else {
            // Fixed size packets (1280 bytes) - 10%
            1280
        }
    }
}

/// Sliding window accumulator for byte counts with automatic expiry.
#[derive(Debug, Clone)]
struct SlidingWindow {
    window: VecDeque<(Instant, usize)>,
    window_len: Duration,
    accumulated: usize,
}

impl SlidingWindow {
    fn new(window_len: Duration) -> Self {
        Self { window: VecDeque::new(), window_len, accumulated: 0 }
    }

    fn set_window_len(&mut self, len: Duration) {
        self.window_len = len.max(Duration::from_secs(1));
        self.purge_old(Instant::now());
    }

    fn record(&mut self, bytes: usize) {
        let now = Instant::now();
        self.window.push_back((now, bytes));
        self.accumulated = self.accumulated.saturating_add(bytes);
        self.purge_old(now);
    }

    fn total_bytes(&mut self) -> usize {
        let now = Instant::now();
        self.purge_old(now);
        self.accumulated
    }

    fn purge_old(&mut self, now: Instant) {
        while let Some(&(ts, bytes)) = self.window.front() {
            if now.duration_since(ts) > self.window_len {
                self.window.pop_front();
                self.accumulated = self.accumulated.saturating_sub(bytes);
            } else {
                break;
            }
        }
    }
}

/// Real-time cover traffic testing framework.
#[allow(dead_code)]
pub struct CoverTrafficTester {
    generator: AdaptiveCoverGenerator,
    test_duration: Duration,
    test_scenarios: Vec<TestScenario>,
}

/// Test scenario for cover traffic validation.
#[derive(Debug, Clone)]
pub struct TestScenario {
    pub name: String,
    pub duration: Duration,
    pub expected_lambda_range: (f64, f64),
    pub power_profile: Option<PowerProfile>,
    pub network_state: Option<NetworkState>,
    pub app_state: Option<AppState>,
}

impl CoverTrafficTester {
    /// Create a new cover traffic tester.
    pub fn new(config: AdaptiveCoverConfig) -> Self {
        let generator = AdaptiveCoverGenerator::new(config);
        
        Self {
            generator,
            test_duration: Duration::from_secs(300), // 5 minutes default
            test_scenarios: Self::default_test_scenarios(),
        }
    }

    /// Run comprehensive cover traffic tests.
    pub async fn run_tests(&self) -> Result<TestResults, Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting cover traffic real-time tests");

        let mut results = TestResults::new();
        
        // Start the generator
        self.generator.start().await?;

        // Run each test scenario
        for scenario in &self.test_scenarios {
            info!("Running test scenario: {}", scenario.name);
            
            let scenario_result = self.run_scenario(scenario).await?;
            results.scenario_results.push(scenario_result);
        }

        // Calculate overall results
        results.calculate_summary();

        info!("Cover traffic tests completed. Overall success rate: {:.1}%", 
              results.overall_success_rate * 100.0);

        Ok(results)
    }

    /// Run a single test scenario.
    async fn run_scenario(&self, scenario: &TestScenario) -> Result<ScenarioResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let mut _lambda_samples = Vec::new();
        let mut metrics_rx = self.generator.subscribe_metrics();
        let scenario_duration = scenario.duration;

        // Simulate scenario conditions
        self.simulate_scenario_conditions(scenario).await;

        // Collect metrics during scenario
        let metrics_task = tokio::spawn(async move {
            let mut samples = Vec::new();
            let end_time = start_time + scenario_duration;
            
            while Instant::now() < end_time {
                if let Ok(metrics) = metrics_rx.recv().await {
                    samples.push(metrics.current_lambda);
                }
            }
            samples
        });

        // Wait for scenario duration
        sleep_until(TokioInstant::now() + scenario_duration).await;

        _lambda_samples = metrics_task.await.unwrap_or_default();

        // Analyze results
        let avg_lambda = _lambda_samples.iter().sum::<f64>() / _lambda_samples.len() as f64;
        let in_range = avg_lambda >= scenario.expected_lambda_range.0 && avg_lambda <= scenario.expected_lambda_range.1;

        Ok(ScenarioResult {
            name: scenario.name.clone(),
            duration: scenario_duration,
            average_lambda: avg_lambda,
            expected_range: scenario.expected_lambda_range,
            lambda_samples: _lambda_samples,
            success: in_range,
            efficiency: self.calculate_scenario_efficiency(scenario, avg_lambda).await,
        })
    }

    /// Simulate scenario-specific conditions.
    async fn simulate_scenario_conditions(&self, _scenario: &TestScenario) {
        // In a real implementation, this would:
        // - Set mock power states
        // - Simulate network conditions
        // - Trigger app state changes
        // For now, we just log the scenario
        debug!("Simulating conditions for scenario: {}", _scenario.name);
    }

    /// Calculate efficiency for a scenario.
    async fn calculate_scenario_efficiency(&self, scenario: &TestScenario, avg_lambda: f64) -> f64 {
        let target_lambda = (scenario.expected_lambda_range.0 + scenario.expected_lambda_range.1) / 2.0;
        if target_lambda > 0.0 {
            1.0 - (avg_lambda - target_lambda).abs() / target_lambda
        } else {
            1.0
        }
    }

    /// Default test scenarios for cover traffic validation.
    fn default_test_scenarios() -> Vec<TestScenario> {
        vec![
            TestScenario {
                name: "High Performance Mode".to_string(),
                duration: Duration::from_secs(60),
                expected_lambda_range: (1.8, 2.2),
                power_profile: Some(PowerProfile::HighPerformance),
                network_state: Some(NetworkState::WiFi),
                app_state: Some(AppState::Active),
            },
            TestScenario {
                name: "Power Saver Mode".to_string(),
                duration: Duration::from_secs(60),
                expected_lambda_range: (0.5, 0.8),
                power_profile: Some(PowerProfile::PowerSaver),
                network_state: Some(NetworkState::WiFi),
                app_state: Some(AppState::Background),
            },
            TestScenario {
                name: "Ultra Low Power Mode".to_string(),
                duration: Duration::from_secs(60),
                expected_lambda_range: (0.1, 0.3),
                power_profile: Some(PowerProfile::UltraLowPower),
                network_state: Some(NetworkState::Cellular),
                app_state: Some(AppState::Background),
            },
            TestScenario {
                name: "Cellular Network".to_string(),
                duration: Duration::from_secs(45),
                expected_lambda_range: (1.0, 1.5),
                power_profile: Some(PowerProfile::Balanced),
                network_state: Some(NetworkState::Cellular),
                app_state: Some(AppState::Active),
            },
            TestScenario {
                name: "No Network".to_string(),
                duration: Duration::from_secs(30),
                expected_lambda_range: (0.0, 0.1),
                power_profile: Some(PowerProfile::Balanced),
                network_state: Some(NetworkState::None),
                app_state: Some(AppState::Active),
            },
        ]
    }
}

/// Results from cover traffic testing.
#[derive(Debug, Clone)]
pub struct TestResults {
    pub scenario_results: Vec<ScenarioResult>,
    pub overall_success_rate: f64,
    pub total_test_time: Duration,
    pub average_efficiency: f64,
}

impl TestResults {
    fn new() -> Self {
        Self {
            scenario_results: Vec::new(),
            overall_success_rate: 0.0,
            total_test_time: Duration::ZERO,
            average_efficiency: 0.0,
        }
    }

    fn calculate_summary(&mut self) {
        let successful = self.scenario_results.iter().filter(|r| r.success).count();
        self.overall_success_rate = successful as f64 / self.scenario_results.len() as f64;
        
        self.total_test_time = self.scenario_results.iter().map(|r| r.duration).sum();
        
        self.average_efficiency = self.scenario_results.iter()
            .map(|r| r.efficiency)
            .sum::<f64>() / self.scenario_results.len() as f64;
    }
}

/// Results from a single test scenario.
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    pub name: String,
    pub duration: Duration,
    pub average_lambda: f64,
    pub expected_range: (f64, f64),
    pub lambda_samples: Vec<f64>,
    pub success: bool,
    pub efficiency: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use rand::thread_rng;

    #[tokio::test]
    async fn test_adaptive_cover_generator() {
        let config = AdaptiveCoverConfig::default();
        let generator = AdaptiveCoverGenerator::new(config);

        // Test initial state
        let initial_lambda = generator.current_lambda().await;
        assert_eq!(initial_lambda, 2.0);

        // Test metrics
        let metrics = generator.metrics().await;
        assert_eq!(metrics.current_lambda, 2.0);
    }

    #[tokio::test]
    async fn test_power_profile_scaling() {
        assert_eq!(PowerProfile::HighPerformance.cover_traffic_scale(), 1.0);
        assert_eq!(PowerProfile::PowerSaver.cover_traffic_scale(), 0.3);
        assert_eq!(PowerProfile::UltraLowPower.cover_traffic_scale(), 0.1);
    }

    #[tokio::test]
    async fn test_packet_size_generation() {
        let mut rng = thread_rng();
        let size = AdaptiveCoverGenerator::generate_packet_size(&mut rng);
        assert!(size >= 40 && size <= 1280);
    }

    #[tokio::test]
    async fn test_cover_traffic_tester() {
        let config = AdaptiveCoverConfig {
            base_lambda: 1.0,
            max_lambda: 5.0,
            min_lambda: 0.1,
            ..Default::default()
        };
        
        let tester = CoverTrafficTester::new(config);
        assert_eq!(tester.test_scenarios.len(), 5);
    }

    #[tokio::test]
    async fn test_time_based_scaling() {
        let config = AdaptiveCoverConfig::default();
        let scale = AdaptiveCoverGenerator::calculate_time_scale(&config).await;
        assert!(scale > 0.0 && scale <= 1.2);
    }

    #[tokio::test]
    async fn test_lambda_adaptation_with_manual_scale() {
        // Fast adaptation config
        let config = AdaptiveCoverConfig {
            base_lambda: 4.0,
            max_lambda: 10.0,
            min_lambda: 0.1,
            adaptation_speed: 1.0, // immediate move to target
            manual_scale: Some(0.25), // target lambda initially 1.0
            adaptation_interval_ms: 50,
            mobile_adaptation: false,
            time_based_adaptation: false,
            network_adaptation: false,
            ..AdaptiveCoverConfig::default()
        };

        let generator = AdaptiveCoverGenerator::new(config.clone());
        generator.start().await.unwrap();
        tokio::time::sleep(Duration::from_millis(160)).await; // allow several cycles

        let lambda1 = generator.current_lambda().await;
        assert!((lambda1 - 1.0).abs() < 0.15, "lambda should adapt down to ~1.0, got {}", lambda1);

        // Update config to increase scale
        let mut new_cfg = config.clone();
        new_cfg.manual_scale = Some(2.0); // target 8.0 but clamped by max 10.0
        generator.update_config(new_cfg).await;
        tokio::time::sleep(Duration::from_millis(160)).await;

        let lambda2 = generator.current_lambda().await;
        assert!(lambda2 > lambda1 + 0.5, "lambda should increase after manual_scale up ({} -> {})", lambda1, lambda2);
        assert!(lambda2 <= 10.0);
    }

    #[tokio::test]
    async fn ratio_feedback_increases_lambda_under_real_load() {
        // Fast control to observe effect quickly
        let config = AdaptiveCoverConfig {
            base_lambda: 2.0,
            max_lambda: 20.0,
            min_lambda: 0.1,
            adaptation_speed: 0.8,
            alpha_gain: 0.6,
            ema_beta: 0.4,
            adaptation_interval_ms: 50,
            mobile_adaptation: false,
            time_based_adaptation: false,
            network_adaptation: false,
            ..AdaptiveCoverConfig::default()
        };

        let generator = AdaptiveCoverGenerator::new(config);
        generator.start().await.unwrap();

        // Baseline after a few cycles
        tokio::time::sleep(Duration::from_millis(150)).await;
        let baseline = generator.current_lambda().await;

        // Inject heavy real traffic so cover ratio becomes low and controller increases λ
        for _ in 0..10 {
            generator.record_real_bytes(20000).await;
            tokio::time::sleep(Duration::from_millis(60)).await;
        }

        let increased = generator.current_lambda().await;
        assert!(increased > baseline, "lambda should increase under heavy real load ({} -> {})", baseline, increased);
    }

    #[tokio::test]
    async fn heartbeat_bump_when_idle_applies_minimum_activity() {
        let config = AdaptiveCoverConfig {
            base_lambda: 1.0,
            max_lambda: 5.0,
            min_lambda: 0.2,
            heartbeat_bump_sec: 1,
            adaptation_speed: 1.0,
            adaptation_interval_ms: 100,
            mobile_adaptation: false,
            time_based_adaptation: false,
            network_adaptation: false,
            ..AdaptiveCoverConfig::default()
        };
        let generator = AdaptiveCoverGenerator::new(config);
        generator.start().await.unwrap();

        tokio::time::sleep(Duration::from_millis(1200)).await; // exceed heartbeat window
        let lambda_after_idle = generator.current_lambda().await;
        assert!(lambda_after_idle >= 0.2 * 1.5 - 0.01, "heartbeat bump should raise lambda near 1.5x min; got {}", lambda_after_idle);
    }
} 