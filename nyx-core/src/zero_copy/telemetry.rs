#![forbid(unsafe_code)]

//! Telemetry integration for zero-copy optimization metrics.
//!
//! This module provides comprehensive telemetry collection and reporting
//! for zero-copy optimization across the critical data path. It integrates
//! with the existing Nyx telemetry system to provide detailed insights
//! into memory allocation patterns, buffer pool performance, and
//! optimization effectiveness.

use super::*;
use crate::zero_copy::manager::{AggregatedMetrics, ZeroCopyManager};
// Conditional import - only available with telemetry feature
#[cfg(feature = "telemetry")]
use nyx_telemetry::{MetricType, TelemetryCollector, Timestamp};

#[cfg(not(feature = "telemetry"))]
mod mock_telemetry {
    use std::collections::HashMap;

    #[derive(Clone)]
    pub struct TelemetryCollector;

    impl TelemetryCollector {
        pub async fn record_metric(
            &self,
            _name: &str,
            _metric_type: MetricType,
            _value: f64,
            _timestamp: Timestamp,
            _labels: Option<HashMap<String, String>>,
        ) {
            // No-op for testing
        }
    }

    #[derive(Clone, Copy)]
    pub enum MetricType {
        Counter,
        Gauge,
    }

    pub type Timestamp = std::time::Instant;

    // We can't implement methods on external types, so just use constructor
    pub fn now() -> Timestamp {
        std::time::Instant::now()
    }
}

#[cfg(not(feature = "telemetry"))]
use mock_telemetry::{now, MetricType, TelemetryCollector};

#[cfg(feature = "telemetry")]
fn now() -> std::time::Instant {
    std::time::Instant::now()
}
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

/// Telemetry collector for zero-copy metrics
pub struct ZeroCopyTelemetry {
    /// Reference to telemetry system
    collector: Arc<TelemetryCollector>,
    /// Zero-copy manager reference
    manager: Arc<ZeroCopyManager>,
    /// Collection configuration
    config: TelemetryConfig,
    /// Metric history for trend analysis
    metric_history: Arc<RwLock<VecDeque<TimestampedMetrics>>>,
}

/// Configuration for zero-copy telemetry collection
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Collection interval
    pub collection_interval: Duration,
    /// Maximum history entries to retain
    pub max_history_entries: usize,
    /// Enable detailed per-stage metrics
    pub enable_detailed_metrics: bool,
    /// Enable buffer pool metrics
    pub enable_pool_metrics: bool,
    /// Enable optimization trend analysis
    pub enable_trend_analysis: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            collection_interval: Duration::from_secs(10),
            max_history_entries: 1000,
            enable_detailed_metrics: true,
            enable_pool_metrics: true,
            enable_trend_analysis: true,
        }
    }
}

/// Timestamped metrics snapshot
#[derive(Debug, Clone)]
pub struct TimestampedMetrics {
    pub timestamp: Instant,
    pub metrics: AggregatedMetrics,
    pub pool_stats: HashMap<String, BufferPoolStats>,
}

impl ZeroCopyTelemetry {
    /// Create new telemetry collector
    pub fn new(
        collector: Arc<TelemetryCollector>,
        manager: Arc<ZeroCopyManager>,
        config: TelemetryConfig,
    ) -> Self {
        Self {
            collector,
            manager,
            config: config.clone(),
            metric_history: Arc::new(RwLock::new(VecDeque::with_capacity(
                config.max_history_entries,
            ))),
        }
    }

    /// Start telemetry collection background task
    pub async fn start_collection(&self) -> tokio::task::JoinHandle<()> {
        let collector = Arc::clone(&self.collector);
        let manager = Arc::clone(&self.manager);
        let config = self.config.clone();
        let metric_history = Arc::clone(&self.metric_history);

        tokio::spawn(async move {
            let mut interval = interval(config.collection_interval);

            loop {
                interval.tick().await;

                // Collect current metrics
                let metrics = manager.get_aggregated_metrics().await;
                let timestamp = now();

                // Record core metrics
                Self::record_core_metrics(&collector, &metrics).await;

                // Record detailed per-stage metrics if enabled
                if config.enable_detailed_metrics {
                    Self::record_detailed_metrics(&collector, &metrics).await;
                }

                // Record buffer pool metrics if enabled
                let mut pool_stats = HashMap::new();
                if config.enable_pool_metrics {
                    // Query manager for per-path BufferPool statistics.
                    pool_stats = manager.get_all_pool_stats().await;

                    // Emit pool stats to the telemetry backend as gauges.
                    for (path_id, stats) in &pool_stats {
                        let mut labels = HashMap::new();
                        labels.insert("path_id".to_string(), path_id.clone());

                        collector
                            .record_metric(
                                "zero_copy_pool_total_buffers",
                                MetricType::Gauge,
                                stats.total_buffers as f64,
                                timestamp,
                                Some(labels.clone()),
                            )
                            .await;

                        collector
                            .record_metric(
                                "zero_copy_pool_hit_ratio",
                                MetricType::Gauge,
                                stats.hit_ratio,
                                timestamp,
                                Some(labels.clone()),
                            )
                            .await;

                        collector
                            .record_metric(
                                "zero_copy_pool_hits",
                                MetricType::Counter,
                                stats.hits as f64,
                                timestamp,
                                Some(labels.clone()),
                            )
                            .await;

                        collector
                            .record_metric(
                                "zero_copy_pool_misses",
                                MetricType::Counter,
                                stats.misses as f64,
                                timestamp,
                                Some(labels.clone()),
                            )
                            .await;
                    }
                }

                // Store in history for trend analysis
                if config.enable_trend_analysis {
                    let mut history = metric_history.write().await;
                    if history.len() >= config.max_history_entries {
                        history.pop_front();
                    }
                    history.push_back(TimestampedMetrics {
                        timestamp,
                        metrics,
                        pool_stats,
                    });
                }
            }
        })
    }

    /// Record core zero-copy metrics
    async fn record_core_metrics(collector: &TelemetryCollector, metrics: &AggregatedMetrics) {
        let timestamp = now();

        // Pipeline-level metrics
        collector
            .record_metric(
                "zero_copy_total_allocations",
                MetricType::Counter,
                metrics.combined_allocations as f64,
                timestamp,
                None,
            )
            .await;

        collector
            .record_metric(
                "zero_copy_total_bytes",
                MetricType::Counter,
                metrics.combined_bytes as f64,
                timestamp,
                None,
            )
            .await;

        collector
            .record_metric(
                "zero_copy_ratio",
                MetricType::Gauge,
                metrics.average_zero_copy_ratio,
                timestamp,
                None,
            )
            .await;

        collector
            .record_metric(
                "zero_copy_reduction_ratio",
                MetricType::Gauge,
                metrics.average_reduction_ratio,
                timestamp,
                None,
            )
            .await;

        collector
            .record_metric(
                "zero_copy_allocation_overhead_ns",
                MetricType::Gauge,
                metrics.total_allocation_overhead_ns as f64,
                timestamp,
                None,
            )
            .await;

        collector
            .record_metric(
                "zero_copy_active_paths",
                MetricType::Gauge,
                metrics.total_paths as f64,
                timestamp,
                None,
            )
            .await;
    }

    /// Record detailed per-stage metrics
    async fn record_detailed_metrics(collector: &TelemetryCollector, metrics: &AggregatedMetrics) {
        let timestamp = now();

        // Aggregate stage metrics across all paths
        let mut stage_totals: HashMap<Stage, StageStats> = HashMap::new();

        for path_metrics in metrics.per_path_metrics.values() {
            for (stage, stats) in &path_metrics.stages {
                let total_stats = stage_totals
                    .entry(*stage)
                    .or_insert_with(StageStats::default);
                total_stats.total_allocations += stats.total_allocations;
                total_stats.total_bytes += stats.total_bytes;
                total_stats.total_copies += stats.total_copies;
                total_stats.total_copy_bytes += stats.total_copy_bytes;
                total_stats.zero_copy_ops += stats.zero_copy_ops;
                total_stats.pool_hits += stats.pool_hits;
                total_stats.pool_misses += stats.pool_misses;
            }
        }

        // Record per-stage metrics
        for (stage, stats) in stage_totals {
            let stage_name = match stage {
                Stage::Crypto => "crypto",
                Stage::Fec => "fec",
                Stage::Transmission => "transmission",
            };

            let mut labels = HashMap::new();
            labels.insert("stage".to_string(), stage_name.to_string());

            collector
                .record_metric(
                    "zero_copy_stage_allocations",
                    MetricType::Counter,
                    stats.total_allocations as f64,
                    timestamp,
                    Some(labels.clone()),
                )
                .await;

            collector
                .record_metric(
                    "zero_copy_stage_bytes",
                    MetricType::Counter,
                    stats.total_bytes as f64,
                    timestamp,
                    Some(labels.clone()),
                )
                .await;

            collector
                .record_metric(
                    "zero_copy_stage_copies",
                    MetricType::Counter,
                    stats.total_copies as f64,
                    timestamp,
                    Some(labels.clone()),
                )
                .await;

            collector
                .record_metric(
                    "zero_copy_stage_zero_copy_ops",
                    MetricType::Counter,
                    stats.zero_copy_ops as f64,
                    timestamp,
                    Some(labels.clone()),
                )
                .await;

            // Pool efficiency metrics
            let total_requests = stats.pool_hits + stats.pool_misses;
            let hit_ratio = if total_requests > 0 {
                stats.pool_hits as f64 / total_requests as f64
            } else {
                0.0
            };

            collector
                .record_metric(
                    "zero_copy_stage_pool_hit_ratio",
                    MetricType::Gauge,
                    hit_ratio,
                    timestamp,
                    Some(labels),
                )
                .await;
        }
    }

    /// Generate zero-copy optimization report
    pub async fn generate_optimization_report(&self) -> OptimizationReport {
        let current_metrics = self.manager.get_aggregated_metrics().await;
        let history = self.metric_history.read().await;

        let mut report = OptimizationReport {
            generated_at: Instant::now(),
            total_paths: current_metrics.total_paths,
            current_metrics: current_metrics.clone(),
            optimization_opportunities: Vec::new(),
            performance_trends: Vec::new(),
            recommendations: Vec::new(),
        };

        // Analyze optimization opportunities
        if current_metrics.average_zero_copy_ratio < 0.7 {
            report
                .optimization_opportunities
                .push(OptimizationOpportunity {
                    category: "Zero-Copy Ratio".to_string(),
                    description: format!(
                        "Current zero-copy ratio ({:.2}%) is below optimal threshold (70%)",
                        current_metrics.average_zero_copy_ratio * 100.0
                    ),
                    potential_impact: ImpactLevel::High,
                    suggested_actions: vec![
                        "Enable buffer pooling for all stages".to_string(),
                        "Implement in-place operations where possible".to_string(),
                        "Review data transformation requirements".to_string(),
                    ],
                });
        }

        if current_metrics.average_reduction_ratio < 0.5 {
            report
                .optimization_opportunities
                .push(OptimizationOpportunity {
                    category: "Copy Reduction".to_string(),
                    description: format!(
                        "Current copy reduction ratio ({:.2}%) indicates high memory copy overhead",
                        current_metrics.average_reduction_ratio * 100.0
                    ),
                    potential_impact: ImpactLevel::Medium,
                    suggested_actions: vec![
                        "Increase buffer pool sizes".to_string(),
                        "Implement larger buffer size classes".to_string(),
                        "Consider streaming processing patterns".to_string(),
                    ],
                });
        }

        // Analyze trends if sufficient history exists
        if history.len() >= 10 {
            let recent_metrics: Vec<_> = history.iter().rev().take(10).collect();

            // Zero-copy ratio trend
            let zero_copy_trend =
                Self::calculate_trend(&recent_metrics, |m| m.metrics.average_zero_copy_ratio);
            report.performance_trends.push(PerformanceTrend {
                metric_name: "Zero-Copy Ratio".to_string(),
                trend_direction: zero_copy_trend.direction,
                change_rate: zero_copy_trend.rate,
                significance: zero_copy_trend.significance,
            });

            // Allocation overhead trend
            let overhead_trend = Self::calculate_trend(&recent_metrics, |m| {
                m.metrics.total_allocation_overhead_ns as f64
            });
            report.performance_trends.push(PerformanceTrend {
                metric_name: "Allocation Overhead".to_string(),
                trend_direction: overhead_trend.direction,
                change_rate: overhead_trend.rate,
                significance: overhead_trend.significance,
            });
        }

        // Generate recommendations based on analysis
        report.recommendations = Self::generate_recommendations(&report);

        report
    }

    /// Calculate trend for a specific metric
    fn calculate_trend<F>(metrics: &[&TimestampedMetrics], extractor: F) -> TrendAnalysis
    where
        F: Fn(&TimestampedMetrics) -> f64,
    {
        if metrics.len() < 3 {
            return TrendAnalysis {
                direction: TrendDirection::Stable,
                rate: 0.0,
                significance: SignificanceLevel::Low,
            };
        }

        let values: Vec<f64> = metrics.iter().map(|m| extractor(m)).collect();
        let n = values.len() as f64;

        // Simple linear regression to determine trend
        let sum_x: f64 = (0..values.len()).sum::<usize>() as f64;
        let sum_y: f64 = values.iter().sum();
        let sum_xy: f64 = values.iter().enumerate().map(|(i, &y)| i as f64 * y).sum();
        let sum_x2: f64 = (0..values.len()).map(|i| (i * i) as f64).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);

        let direction = if slope > 0.001 {
            TrendDirection::Increasing
        } else if slope < -0.001 {
            TrendDirection::Decreasing
        } else {
            TrendDirection::Stable
        };

        let significance = if slope.abs() > 0.1 {
            SignificanceLevel::High
        } else if slope.abs() > 0.01 {
            SignificanceLevel::Medium
        } else {
            SignificanceLevel::Low
        };

        TrendAnalysis {
            direction,
            rate: slope,
            significance,
        }
    }

    /// Generate recommendations based on analysis
    fn generate_recommendations(report: &OptimizationReport) -> Vec<String> {
        let mut recommendations = Vec::new();

        // Based on zero-copy ratio
        if report.current_metrics.average_zero_copy_ratio < 0.5 {
            recommendations.push(
                "Priority: Enable comprehensive buffer pooling across all stages".to_string(),
            );
            recommendations.push(
                "Consider implementing custom allocators for frequent operations".to_string(),
            );
        }

        // Based on trends
        for trend in &report.performance_trends {
            if trend.metric_name == "Allocation Overhead"
                && matches!(trend.trend_direction, TrendDirection::Increasing)
                && matches!(trend.significance, SignificanceLevel::High)
            {
                recommendations.push("Alert: Allocation overhead is increasing significantly - investigate memory leaks".to_string());
            }
        }

        // General recommendations
        if report.total_paths > 100 {
            recommendations.push(
                "Consider implementing global buffer pools for high-scale deployments".to_string(),
            );
        }

        if recommendations.is_empty() {
            recommendations.push("Current zero-copy optimization is performing well".to_string());
        }

        recommendations
    }
}

/// Convenience: spawn telemetry collection task for a given `ZeroCopyManager` using a provided collector and config.
/// This helper allows external components (e.g., daemon) to bind zero-copy metrics into their existing
/// telemetry pipeline without duplicating wiring code.
pub async fn spawn_zerocopy_telemetry_for_manager(
    collector: std::sync::Arc<TelemetryCollector>,
    manager: std::sync::Arc<crate::zero_copy::manager::ZeroCopyManager>,
    cfg: TelemetryConfig,
) -> tokio::task::JoinHandle<()> {
    let zt = ZeroCopyTelemetry {
        collector,
        manager,
        config: cfg.clone(),
        metric_history: std::sync::Arc::new(tokio::sync::RwLock::new(
            std::collections::VecDeque::with_capacity(cfg.max_history_entries),
        )),
    };
    zt.start_collection().await
}

/// Optimization analysis report
#[derive(Debug, Clone)]
pub struct OptimizationReport {
    pub generated_at: Instant,
    pub total_paths: usize,
    pub current_metrics: AggregatedMetrics,
    pub optimization_opportunities: Vec<OptimizationOpportunity>,
    pub performance_trends: Vec<PerformanceTrend>,
    pub recommendations: Vec<String>,
}

/// Identified optimization opportunity
#[derive(Debug, Clone)]
pub struct OptimizationOpportunity {
    pub category: String,
    pub description: String,
    pub potential_impact: ImpactLevel,
    pub suggested_actions: Vec<String>,
}

/// Performance trend analysis
#[derive(Debug, Clone)]
pub struct PerformanceTrend {
    pub metric_name: String,
    pub trend_direction: TrendDirection,
    pub change_rate: f64,
    pub significance: SignificanceLevel,
}

/// Trend analysis result
#[derive(Debug, Clone)]
pub struct TrendAnalysis {
    pub direction: TrendDirection,
    pub rate: f64,
    pub significance: SignificanceLevel,
}

/// Trend direction
#[derive(Debug, Clone, PartialEq)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
}

/// Impact level classification
#[derive(Debug, Clone, PartialEq)]
pub enum ImpactLevel {
    High,
    Medium,
    Low,
}

/// Statistical significance level
#[derive(Debug, Clone, PartialEq)]
pub enum SignificanceLevel {
    High,
    Medium,
    Low,
}
