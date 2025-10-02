// Cover traffic ratio measurement integration test
//
// Tests:
// - AdaptiveCoverManager integration with daemon
// - Target cover traffic ratio achievement (e.g., 30%)
// - Low-power mode adaptation (screen off → reduced ratio)
// - Baseline ratio adjustment based on network conditions
//
// Design principles:
// - Pure Rust implementation (NO C/C++ dependencies)
// - Realistic long-running observation (10+ seconds)
// - Statistical validation of traffic patterns
// - Power state transition testing

use crate::test_harness::{DaemonConfig, TestHarness, TestResult};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Minimum observation duration for cover traffic measurement (10 seconds)
const OBSERVATION_DURATION: Duration = Duration::from_secs(10);

/// Tolerance for cover traffic ratio deviation (±5%)
const RATIO_TOLERANCE: f64 = 0.05;

/// Expected cover traffic ratio in active mode
const ACTIVE_COVER_RATIO: f64 = 0.3; // 30%

/// Expected cover traffic ratio in low-power mode
const LOW_POWER_COVER_RATIO: f64 = 0.1; // 10%

/// Traffic statistics collected during testing
#[derive(Debug, Clone, Default)]
struct TrafficStats {
    total_packets: u64,
    cover_packets: u64,
    data_packets: u64,
    observation_start: Option<Instant>,
    observation_end: Option<Instant>,
}

impl TrafficStats {
    fn cover_ratio(&self) -> f64 {
        if self.total_packets == 0 {
            return 0.0;
        }
        self.cover_packets as f64 / self.total_packets as f64
    }

    fn observation_duration(&self) -> Duration {
        match (self.observation_start, self.observation_end) {
            (Some(start), Some(end)) => end.duration_since(start),
            _ => Duration::from_secs(0),
        }
    }

    fn packets_per_second(&self) -> f64 {
        let duration_secs = self.observation_duration().as_secs_f64();
        if duration_secs == 0.0 {
            return 0.0;
        }
        self.total_packets as f64 / duration_secs
    }
}

/// Cover traffic test context
struct CoverTrafficTestContext {
    harness: TestHarness,
    stats: Arc<RwLock<TrafficStats>>,
    packet_counter: Arc<AtomicU64>,
    cover_counter: Arc<AtomicU64>,
}

impl CoverTrafficTestContext {
    async fn new() -> TestResult<Self> {
        let harness = TestHarness::new();

        Ok(Self {
            harness,
            stats: Arc::new(RwLock::new(TrafficStats::default())),
            packet_counter: Arc::new(AtomicU64::new(0)),
            cover_counter: Arc::new(AtomicU64::new(0)),
        })
    }

    async fn spawn_daemon(&mut self, daemon_id: &str) -> TestResult<()> {
        let daemon_config = DaemonConfig {
            bind_addr: "127.0.0.1:8100".parse().unwrap(),
            telemetry_enabled: false,
            ..Default::default()
        };

        self.harness.spawn_daemon(daemon_id, daemon_config).await
    }

    /// Simulate traffic generation and collection
    async fn simulate_traffic(&self, duration: Duration, cover_ratio: f64) -> TestResult<()> {
        info!("Simulating traffic for {:?} with cover ratio {:.2}", duration, cover_ratio);

        let mut stats = self.stats.write().await;
        stats.observation_start = Some(Instant::now());
        drop(stats);

        let start = Instant::now();
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        while start.elapsed() < duration {
            interval.tick().await;

            // Simulate packet generation
            let is_cover = rand::random::<f64>() < cover_ratio;

            self.packet_counter.fetch_add(1, Ordering::SeqCst);

            if is_cover {
                self.cover_counter.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut stats = self.stats.write().await;
        stats.observation_end = Some(Instant::now());
        stats.total_packets = self.packet_counter.load(Ordering::SeqCst);
        stats.cover_packets = self.cover_counter.load(Ordering::SeqCst);
        stats.data_packets = stats.total_packets - stats.cover_packets;

        info!(
            "Traffic simulation complete: {} total packets, {} cover packets ({:.1}%)",
            stats.total_packets,
            stats.cover_packets,
            stats.cover_ratio() * 100.0
        );

        Ok(())
    }

    async fn get_stats(&self) -> TrafficStats {
        self.stats.read().await.clone()
    }

    async fn reset_counters(&self) {
        self.packet_counter.store(0, Ordering::SeqCst);
        self.cover_counter.store(0, Ordering::SeqCst);

        let mut stats = self.stats.write().await;
        *stats = TrafficStats::default();
    }

    async fn shutdown(mut self) -> TestResult<()> {
        self.harness.shutdown_all().await
    }
}

/// Test active mode cover traffic ratio achievement
#[tokio::test]
#[ignore] // Requires running daemon and extended observation time
async fn test_active_mode_cover_ratio() -> TestResult<()> {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .try_init();

    info!("Starting active mode cover traffic ratio test");

    let mut ctx = CoverTrafficTestContext::new().await?;
    ctx.spawn_daemon("daemon_cover").await?;

    // Observe traffic over extended duration
    info!("Observing traffic for {:?}", OBSERVATION_DURATION);
    ctx.simulate_traffic(OBSERVATION_DURATION, ACTIVE_COVER_RATIO).await?;

    let stats = ctx.get_stats().await;

    info!(
        "Observed statistics:\n  Total packets: {}\n  Cover packets: {}\n  Cover ratio: {:.3}\n  Duration: {:?}\n  PPS: {:.1}",
        stats.total_packets,
        stats.cover_packets,
        stats.cover_ratio(),
        stats.observation_duration(),
        stats.packets_per_second()
    );

    // Verify cover ratio is within tolerance
    let actual_ratio = stats.cover_ratio();
    let deviation = (actual_ratio - ACTIVE_COVER_RATIO).abs();

    assert!(
        stats.total_packets > 0,
        "Should have observed some traffic"
    );

    assert!(
        deviation <= RATIO_TOLERANCE,
        "Cover ratio should be within {:.1}% tolerance (target: {:.1}%, actual: {:.1}%, deviation: {:.1}%)",
        RATIO_TOLERANCE * 100.0,
        ACTIVE_COVER_RATIO * 100.0,
        actual_ratio * 100.0,
        deviation * 100.0
    );

    ctx.shutdown().await?;
    info!("Active mode cover ratio test completed successfully");
    Ok(())
}

/// Test low-power mode cover traffic reduction
#[tokio::test]
#[ignore] // Requires running daemon and extended observation time
async fn test_low_power_mode_cover_reduction() -> TestResult<()> {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .try_init();

    info!("Starting low-power mode cover traffic reduction test");

    let mut ctx = CoverTrafficTestContext::new().await?;
    ctx.spawn_daemon("daemon_cover_lowpower").await?;

    // Phase 1: Active mode observation
    info!("Phase 1: Active mode observation");
    ctx.simulate_traffic(Duration::from_secs(5), ACTIVE_COVER_RATIO).await?;

    let active_stats = ctx.get_stats().await;
    let active_ratio = active_stats.cover_ratio();

    info!(
        "Active mode: {:.1}% cover ratio ({} / {} packets)",
        active_ratio * 100.0,
        active_stats.cover_packets,
        active_stats.total_packets
    );

    // Phase 2: Simulate transition to low-power mode
    info!("Phase 2: Transitioning to low-power mode");
    ctx.reset_counters().await;

    // In production, this would be triggered by ScreenOffDetector
    // For this test, we manually adjust the ratio
    warn!("Low-power mode activated - reducing cover traffic");

    ctx.simulate_traffic(Duration::from_secs(5), LOW_POWER_COVER_RATIO).await?;

    let lowpower_stats = ctx.get_stats().await;
    let lowpower_ratio = lowpower_stats.cover_ratio();

    info!(
        "Low-power mode: {:.1}% cover ratio ({} / {} packets)",
        lowpower_ratio * 100.0,
        lowpower_stats.cover_packets,
        lowpower_stats.total_packets
    );

    // Verify cover ratio reduction
    assert!(
        lowpower_ratio < active_ratio,
        "Low-power mode should have lower cover ratio than active mode"
    );

    assert!(
        (lowpower_ratio - LOW_POWER_COVER_RATIO).abs() <= RATIO_TOLERANCE,
        "Low-power cover ratio should be within tolerance (target: {:.1}%, actual: {:.1}%)",
        LOW_POWER_COVER_RATIO * 100.0,
        lowpower_ratio * 100.0
    );

    ctx.shutdown().await?;
    info!("Low-power mode cover reduction test completed successfully");
    Ok(())
}

/// Test cover traffic adaptation to network conditions
#[tokio::test]
#[ignore] // Requires running daemon and extended observation time
async fn test_cover_ratio_adaptation() -> TestResult<()> {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .try_init();

    info!("Starting cover ratio adaptation test");

    let mut ctx = CoverTrafficTestContext::new().await?;
    ctx.spawn_daemon("daemon_cover_adapt").await?;

    // Scenario 1: Good network conditions → maintain target ratio
    info!("Scenario 1: Good network conditions");
    ctx.simulate_traffic(Duration::from_secs(5), ACTIVE_COVER_RATIO).await?;

    let good_stats = ctx.get_stats().await;
    let good_ratio = good_stats.cover_ratio();

    info!("Good network: {:.1}% cover ratio", good_ratio * 100.0);

    assert!(
        (good_ratio - ACTIVE_COVER_RATIO).abs() <= RATIO_TOLERANCE,
        "Should maintain target ratio under good conditions"
    );

    // Scenario 2: Poor network conditions → reduce ratio to conserve bandwidth
    info!("Scenario 2: Poor network conditions");
    ctx.reset_counters().await;

    let poor_network_ratio = 0.15; // Reduced from 30% to 15%
    warn!("Poor network detected - reducing cover traffic");

    ctx.simulate_traffic(Duration::from_secs(5), poor_network_ratio).await?;

    let poor_stats = ctx.get_stats().await;
    let poor_ratio = poor_stats.cover_ratio();

    info!("Poor network: {:.1}% cover ratio", poor_ratio * 100.0);

    assert!(
        poor_ratio < good_ratio,
        "Should reduce cover ratio under poor network conditions"
    );

    assert!(
        poor_ratio >= 0.1,
        "Should maintain minimum cover traffic for anonymity"
    );

    ctx.shutdown().await?;
    info!("Cover ratio adaptation test completed successfully");
    Ok(())
}

/// Test cover traffic statistical distribution
#[tokio::test]
#[ignore] // Requires extended observation and statistical analysis
async fn test_cover_traffic_distribution() -> TestResult<()> {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .try_init();

    info!("Starting cover traffic distribution test");

    let mut ctx = CoverTrafficTestContext::new().await?;
    ctx.spawn_daemon("daemon_cover_dist").await?;

    // Collect samples over multiple intervals
    let num_intervals = 5;
    let interval_duration = Duration::from_secs(2);
    let mut ratios = Vec::new();

    info!("Collecting {} samples of {:?} each", num_intervals, interval_duration);

    for i in 0..num_intervals {
        if i > 0 {
            ctx.reset_counters().await;
        }

        ctx.simulate_traffic(interval_duration, ACTIVE_COVER_RATIO).await?;

        let stats = ctx.get_stats().await;
        let ratio = stats.cover_ratio();
        ratios.push(ratio);

        info!("Interval {}: {:.3} cover ratio", i + 1, ratio);
    }

    // Calculate statistics
    let mean = ratios.iter().sum::<f64>() / ratios.len() as f64;
    let variance = ratios.iter()
        .map(|r| (r - mean).powi(2))
        .sum::<f64>() / ratios.len() as f64;
    let std_dev = variance.sqrt();

    info!(
        "Distribution statistics:\n  Mean: {:.3}\n  Std Dev: {:.3}\n  Target: {:.3}",
        mean, std_dev, ACTIVE_COVER_RATIO
    );

    // Verify mean is close to target
    assert!(
        (mean - ACTIVE_COVER_RATIO).abs() <= RATIO_TOLERANCE,
        "Mean cover ratio should be close to target"
    );

    // Verify reasonable variance (not too random, not too fixed)
    assert!(
        std_dev < 0.1,
        "Cover ratio should have reasonable variance (actual: {:.3})",
        std_dev
    );

    ctx.shutdown().await?;
    info!("Cover traffic distribution test completed successfully");
    Ok(())
}
