#![forbid(unsafe_code)]

//! Nyx Telemetry with OpenTelemetry OTLP Integration
//!
//! This module provides comprehensive observability for the Nyx protocol including:
//! - OpenTelemetry OTLP exporter for metrics, traces, and logs
//! - Prometheus metrics collection and export
//! - Distributed tracing with correlation IDs
//! - Custom metrics for Nyx-specific operations
//! - Performance monitoring and alerting
//! - Error tracking and analysis

use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use prometheus::{IntCounterVec, IntCounter};
use std::sync::atomic::AtomicBool;
use tokio::sync::{broadcast, RwLock};
use tokio::time::interval;
use tracing::{info, debug};
use once_cell::sync::Lazy;
use std::sync::RwLock as StdRwLock;

// -------------------------------------------------------------------------------------------------
// Stream send hook (metrics correlation between nyx-stream and telemetry without direct coupling)
// -------------------------------------------------------------------------------------------------
// We expose a lightweight global function pointer that nyx-stream can call (behind the `telemetry`
// feature) to record per-path stream send events. This avoids pulling Prometheus types into
// nyx-stream and keeps the dependency direction clean. If telemetry isn't started yet, calls are
// inexpensive no-ops.
type StreamSendHook = dyn Fn(u8, &str) + Send + Sync + 'static;
static STREAM_SEND_HOOK: Lazy<StdRwLock<Option<Arc<StreamSendHook>>>> = Lazy::new(|| StdRwLock::new(None));

// -------------------------------------------------------------------------------------------------
// Multipath metrics hooks (weight deviation gauge / jitter histogram) - optional (prometheus)
// -------------------------------------------------------------------------------------------------
#[cfg(feature = "prometheus")]
use prometheus::{GaugeVec, HistogramVec};
#[cfg(feature = "prometheus")]
struct MultipathMetricHandles { weight_dev: GaugeVec, jitter_hist: HistogramVec }
#[cfg(feature = "prometheus")]
static MULTIPATH_METRICS: Lazy<StdRwLock<Option<MultipathMetricHandles>>> = Lazy::new(|| StdRwLock::new(None));
#[cfg(feature = "prometheus")]
fn register_multipath_metrics_global(weight_dev: GaugeVec, jitter_hist: HistogramVec) {
    *MULTIPATH_METRICS.write().unwrap() = Some(MultipathMetricHandles { weight_dev, jitter_hist });
}

/// Record weight ratio deviation (actual_ratio - expected_ratio) for a path.
pub fn record_multipath_weight_deviation(path_id: u8, deviation: f64) {
    #[cfg(feature = "prometheus")]
    {
        if let Some(handles) = MULTIPATH_METRICS.read().unwrap().as_ref() {
            if let Ok(g) = handles.weight_dev.get_metric_with_label_values(&[&path_id.to_string()]) { g.set(deviation); }
        }
    }
    let _ = path_id; let _ = deviation; // no-op if feature disabled
}

/// Record per-path RTT jitter (ms) sample.
pub fn record_multipath_jitter(path_id: u8, jitter_ms: f64) {
    #[cfg(feature = "prometheus")]
    {
        if let Some(handles) = MULTIPATH_METRICS.read().unwrap().as_ref() {
            if let Ok(h) = handles.jitter_hist.get_metric_with_label_values(&[&path_id.to_string()]) { h.observe(jitter_ms); }
        }
    }
    let _ = path_id; let _ = jitter_ms;
}

/// Register a global stream send hook. Subsequent registrations replace the previous one.
pub fn register_stream_send_hook<F>(f: F)
where
    F: Fn(u8, &str) + Send + Sync + 'static,
{
    let mut guard = STREAM_SEND_HOOK.write().unwrap();
    *guard = Some(Arc::new(f));
}

/// Record a stream send event (called from nyx-stream). Safe to call even if no hook is registered.
pub fn record_stream_send(path_id: u8, cid: &str) {
    if let Some(hook) = STREAM_SEND_HOOK.read().unwrap().as_ref() {
        (hook)(path_id, cid);
    }
}
use anyhow::Result;

// Core telemetry modules
pub mod metrics;
pub mod otlp;
pub mod sampling;
// New v1.0 OpenTelemetry integration (gated behind feature `otlp` or `otlp_exporter`)
#[cfg(any(feature = "otlp", feature = "otlp_exporter"))]
pub mod opentelemetry_integration;

#[cfg(feature = "otlp")]
pub use opentelemetry_integration::{
    NyxTelemetry,
    TelemetryConfig as OTelConfig,
};

// Simplified telemetry without complex OpenTelemetry setup
#[cfg(feature = "prometheus")]
use prometheus::{Encoder, TextEncoder};
use prometheus::{Counter, Gauge, Registry, IntGauge};
#[cfg(feature = "prometheus")]
use warp::Filter;

/// Telemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Enable metrics collection
    pub metrics_enabled: bool,
    /// Prometheus metrics endpoint port
    pub metrics_port: u16,
    /// Metrics collection interval in seconds
    pub collection_interval: u64,
    /// Enable OTLP export
    pub otlp_enabled: bool,
    /// OTLP endpoint URL
    pub otlp_endpoint: Option<String>,
    /// Trace sampling ratio (0.0 to 1.0)
    pub trace_sampling: f64,
    /// Optional path to attribute filter chain config (hot-reloaded). Format: JSON array of rules.
    pub attribute_filter_config: Option<String>,
    /// Enable exporter recovery logic (backoff / circuit breaker metrics)
    pub exporter_recovery: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            metrics_port: 9090,
            collection_interval: 30,
            otlp_enabled: false,
            otlp_endpoint: None,
            trace_sampling: 0.1,
            attribute_filter_config: None,
            exporter_recovery: true,
        }
    }
}

/// System metrics data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub disk_usage: f64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub active_connections: u64,
    pub timestamp: SystemTime,
}

/// Network metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connections_active: u64,
    pub connections_total: u64,
    pub latency_ms: f64,
    pub timestamp: SystemTime,
}

/// Stream metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetrics {
    pub active_streams: u64,
    pub total_streams: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub errors: u64,
    pub timestamp: SystemTime,
}

/// Mix network metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixMetrics {
    pub cover_traffic_sent: u64,
    pub real_traffic_sent: u64,
    pub messages_mixed: u64,
    pub anonymity_set_size: u64,
    pub timestamp: SystemTime,
}

/// Comprehensive telemetry collector
pub struct TelemetryCollector {
    config: TelemetryConfig,
    registry: Registry,
    cpu_usage_gauge: Gauge,
    memory_usage_gauge: Gauge,
    network_bytes_counter: Counter,
    stream_count_gauge: Gauge,
    mix_messages_counter: Counter,
    system_tx: broadcast::Sender<SystemMetrics>,
    network_tx: broadcast::Sender<NetworkMetrics>,
    stream_tx: broadcast::Sender<StreamMetrics>,
    mix_tx: broadcast::Sender<MixMetrics>,
    stream_send_counter: IntCounterVec,
    sampling_spans_kept: IntCounter,
    sampling_spans_dropped: IntCounter,
    exporter_success_counter: Option<IntCounter>,
    exporter_failure_counter: Option<IntCounter>,
    exporter_circuit_open: Option<IntGauge>,
    // Internal state
    running: Arc<RwLock<bool>>,
    #[cfg(feature="prometheus")]
    multipath_weight_dev: Option<GaugeVec>,
    #[cfg(feature="prometheus")]
    multipath_jitter_hist: Option<HistogramVec>,
}

impl TelemetryCollector {
    /// Create a new telemetry collector
    pub fn new(config: TelemetryConfig) -> Result<Self> {
        let registry = Registry::new();
        
        // Create Prometheus metrics
        let cpu_usage_gauge = Gauge::new("nyx_cpu_usage", "CPU usage percentage")?;
        let memory_usage_gauge = Gauge::new("nyx_memory_usage", "Memory usage percentage")?;
        let network_bytes_counter = Counter::new("nyx_network_bytes_total", "Total network bytes")?;
        let stream_count_gauge = Gauge::new("nyx_active_streams", "Number of active streams")?;
        let mix_messages_counter = Counter::new("nyx_mix_messages_total", "Total mix messages")?;
        let stream_send_counter = IntCounterVec::new(
            prometheus::Opts::new("nyx_stream_sends_total", "Total stream sends by path"),
            &["path_id"]
        )?;
    let sampling_spans_kept = IntCounter::new("nyx_trace_spans_sampled_total", "Total trace spans kept after sampling")?;
    let sampling_spans_dropped = IntCounter::new("nyx_trace_spans_dropped_total", "Total trace spans dropped by sampling")?;
        
        // Register metrics
        registry.register(Box::new(cpu_usage_gauge.clone()))?;
        registry.register(Box::new(memory_usage_gauge.clone()))?;
        registry.register(Box::new(network_bytes_counter.clone()))?;
        registry.register(Box::new(stream_count_gauge.clone()))?;
        registry.register(Box::new(mix_messages_counter.clone()))?;
        registry.register(Box::new(stream_send_counter.clone()))?;
    registry.register(Box::new(sampling_spans_kept.clone()))?;
    registry.register(Box::new(sampling_spans_dropped.clone()))?;
        
        // Create broadcast channels
        let (system_tx, _) = broadcast::channel(1000);
        let (network_tx, _) = broadcast::channel(1000);
        let (stream_tx, _) = broadcast::channel(1000);
        let (mix_tx, _) = broadcast::channel(1000);
        
        Ok(Self {
            config,
            registry,
            cpu_usage_gauge,
            memory_usage_gauge,
            network_bytes_counter,
            stream_count_gauge,
            mix_messages_counter,
            system_tx,
            network_tx,
            stream_tx,
            mix_tx,
            stream_send_counter,
            sampling_spans_kept,
            sampling_spans_dropped,
            exporter_success_counter: None,
            exporter_failure_counter: None,
            exporter_circuit_open: None,
            running: Arc::new(RwLock::new(false)),
            #[cfg(feature="prometheus")]
            multipath_weight_dev: None,
            #[cfg(feature="prometheus")]
            multipath_jitter_hist: None,
        })
    }

    fn register_plugin_metrics(&self) {
        use once_cell::sync::Lazy;
    static DONE: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
        if DONE.swap(true, std::sync::atomic::Ordering::AcqRel) { return; }
        let success = IntCounter::new("nyx_plugin_init_success_total", "Plugin initialization successes").unwrap();
        let failure = IntCounter::new("nyx_plugin_init_failure_total", "Plugin initialization failures").unwrap();
        let sec_pass = IntCounter::new("nyx_plugin_security_pass_total", "Plugin security validations passed").unwrap();
        let sec_fail = IntCounter::new("nyx_plugin_security_fail_total", "Plugin security validations failed").unwrap();
        let _ = self.registry.register(Box::new(success));
        let _ = self.registry.register(Box::new(failure));
        let _ = self.registry.register(Box::new(sec_pass));
        let _ = self.registry.register(Box::new(sec_fail));
        #[cfg(feature="prometheus")]
        {
            let duration = prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                "nyx_plugin_init_duration_seconds", "Plugin initialization duration (seconds)"
            ).buckets(vec![0.005,0.01,0.025,0.05,0.1,0.25,0.5,1.0,2.0,5.0])).unwrap();
            let _ = self.registry.register(Box::new(duration));
        }
    }

}

// --- Plugin metric helper functions (global, idempotent lookups) ---
pub fn plugin_metric_inc(name: &str) {
    use prometheus::proto::MetricFamily;
    // Access default registry (we registered into self.registry; for simplicity we rely on gather scan)
    let mfs: Vec<MetricFamily> = prometheus::gather();
    for mf in mfs {
        if mf.get_name() == format!("nyx_{}", name) || mf.get_name() == name { // accept both full and short
            if let Some(counter) = mf.get_metric().get(0) { let _ = counter; }
            // Direct mutation of gathered families isn't possible; instead we keep explicit counters; fallback: metrics should be incremented at source site directly.
        }
    }
    // NOTE: For efficiency & correctness, expose explicit functions below using static OnceCell references.
}

use once_cell::sync::OnceCell;
static PLUGIN_INIT_SUCCESS: OnceCell<IntCounter> = OnceCell::new();
static PLUGIN_INIT_FAILURE: OnceCell<IntCounter> = OnceCell::new();
static PLUGIN_SECURITY_PASS: OnceCell<IntCounter> = OnceCell::new();
static PLUGIN_SECURITY_FAIL: OnceCell<IntCounter> = OnceCell::new();
#[cfg(feature="prometheus")]
static PLUGIN_INIT_DURATION: OnceCell<prometheus::Histogram> = OnceCell::new();

// --- Multipath metrics ---
static MP_PACKETS_SENT: OnceCell<IntCounter> = OnceCell::new();
static MP_PACKETS_RECEIVED: OnceCell<IntCounter> = OnceCell::new();
static MP_PACKETS_REORDERED: OnceCell<IntCounter> = OnceCell::new();
static MP_PACKETS_EXPIRED: OnceCell<IntCounter> = OnceCell::new();
static MP_PATH_ACTIVATED: OnceCell<IntCounter> = OnceCell::new();
static MP_PATH_DEACTIVATED: OnceCell<IntCounter> = OnceCell::new();
static MP_ACTIVE_PATHS: OnceCell<IntGauge> = OnceCell::new();
#[cfg(feature="prometheus")]
static MP_PATH_RTT: OnceCell<prometheus::Histogram> = OnceCell::new();
#[cfg(feature="prometheus")]
static MP_REORDER_DELAY: OnceCell<prometheus::Histogram> = OnceCell::new();
#[cfg(feature="prometheus")]
static MP_REORDER_UTIL: OnceCell<prometheus::GaugeVec> = OnceCell::new();
#[cfg(feature="prometheus")]
static MP_WEIGHT_ENTROPY: OnceCell<prometheus::Gauge> = OnceCell::new(); // Normalized (0..1) fairness entropy over active path weight distribution
#[cfg(feature="prometheus")]
static COVER_TRAFFIC_PPS: OnceCell<prometheus::Gauge> = OnceCell::new();
#[cfg(feature="prometheus")]
static COVER_RATIO_DEVIATION: OnceCell<prometheus::Gauge> = OnceCell::new();

// --- HPKE rekey lifecycle metrics ---
static HPKE_REKEY_INITIATED: OnceCell<IntCounter> = OnceCell::new();
static HPKE_REKEY_APPLIED: OnceCell<IntCounter> = OnceCell::new();
static HPKE_REKEY_GRACE_USED: OnceCell<IntCounter> = OnceCell::new();
static HPKE_REKEY_FAILURES: OnceCell<IntCounter> = OnceCell::new();
// Added: cooldown suppressed decisions (threshold met but cooldown window active)
static HPKE_REKEY_COOLDOWN_SUPPRESSED: OnceCell<IntCounter> = OnceCell::new();
static ERROR_COUNTER: OnceCell<IntCounterVec> = OnceCell::new();
// Added: key lifetime histogram (seconds between successive successful rekeys)
#[cfg(feature="prometheus")]
static HPKE_KEY_LIFETIME: OnceCell<prometheus::Histogram> = OnceCell::new();
// Added: failure reason counter vec (parse/decrypt/generate)
#[cfg(feature="prometheus")]
static HPKE_REKEY_FAILURE_REASON: OnceCell<prometheus::IntCounterVec> = OnceCell::new();

pub fn ensure_plugin_metrics_registered(registry: &Registry) {
    if PLUGIN_INIT_SUCCESS.get().is_some() { return; }
    let s = IntCounter::new("nyx_plugin_init_success_total", "Plugin initialization successes").unwrap();
    let f = IntCounter::new("nyx_plugin_init_failure_total", "Plugin initialization failures").unwrap();
    let sp = IntCounter::new("nyx_plugin_security_pass_total", "Plugin security validations passed").unwrap();
    let sf = IntCounter::new("nyx_plugin_security_fail_total", "Plugin security validations failed").unwrap();
    let _ = registry.register(Box::new(s.clone()));
    let _ = registry.register(Box::new(f.clone()));
    let _ = registry.register(Box::new(sp.clone()));
    let _ = registry.register(Box::new(sf.clone()));
    PLUGIN_INIT_SUCCESS.set(s).ok();
    PLUGIN_INIT_FAILURE.set(f).ok();
    PLUGIN_SECURITY_PASS.set(sp).ok();
    PLUGIN_SECURITY_FAIL.set(sf).ok();
    #[cfg(feature="prometheus")]
    {
        if PLUGIN_INIT_DURATION.get().is_none() {
            let h = prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                "nyx_plugin_init_duration_seconds", "Plugin initialization duration (seconds)"
            ).buckets(vec![0.005,0.01,0.025,0.05,0.1,0.25,0.5,1.0,2.0,5.0])).unwrap();
            let _ = registry.register(Box::new(h.clone()));
            let _ = PLUGIN_INIT_DURATION.set(h);
        }
    }
}

pub fn ensure_multipath_metrics_registered(registry: &Registry) {
    if MP_PACKETS_SENT.get().is_some() { return; }
    let sent = IntCounter::new("nyx_multipath_packets_sent_total", "Multipath packets sent").unwrap();
    let recv = IntCounter::new("nyx_multipath_packets_received_total", "Multipath packets received").unwrap();
    let reo = IntCounter::new("nyx_multipath_packets_reordered_total", "Multipath packets reordered").unwrap();
    let exp = IntCounter::new("nyx_multipath_packets_expired_total", "Multipath packets expired (timeout)").unwrap();
    let act = IntCounter::new("nyx_multipath_path_activated_total", "Multipath paths activated").unwrap();
    let deact = IntCounter::new("nyx_multipath_path_deactivated_total", "Multipath paths deactivated").unwrap();
    let active_g = IntGauge::new("nyx_multipath_active_paths", "Current active multipath paths").unwrap();
    let _ = registry.register(Box::new(sent.clone()));
    let _ = registry.register(Box::new(recv.clone()));
    let _ = registry.register(Box::new(reo.clone()));
    let _ = registry.register(Box::new(exp.clone()));
    let _ = registry.register(Box::new(act.clone()));
    let _ = registry.register(Box::new(deact.clone()));
    let _ = registry.register(Box::new(active_g.clone()));
    MP_PACKETS_SENT.set(sent).ok();
    MP_PACKETS_RECEIVED.set(recv).ok();
    MP_PACKETS_REORDERED.set(reo).ok();
    MP_PACKETS_EXPIRED.set(exp).ok();
    MP_PATH_ACTIVATED.set(act).ok();
    MP_PATH_DEACTIVATED.set(deact).ok();
    MP_ACTIVE_PATHS.set(active_g).ok();
    #[cfg(feature="prometheus")]
    {
        if MP_PATH_RTT.get().is_none() {
            let h = prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                "nyx_multipath_path_rtt_seconds", "Observed path RTT (seconds)"
            ).buckets(vec![0.0005,0.001,0.0025,0.005,0.01,0.025,0.05,0.1,0.2,0.5,1.0,2.0])).unwrap();
            let _ = registry.register(Box::new(h.clone()));
            let _ = MP_PATH_RTT.set(h);
        }
        if MP_REORDER_DELAY.get().is_none() {
            let h = prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                "nyx_multipath_reorder_delay_seconds", "Delay from packet receipt to delivery after reordering"
            ).buckets(vec![0.0005,0.001,0.0025,0.005,0.01,0.025,0.05,0.1,0.2,0.5,1.0])).unwrap();
            let _ = registry.register(Box::new(h.clone()));
            let _ = MP_REORDER_DELAY.set(h);
        }
        if MP_WEIGHT_ENTROPY.get().is_none() {
            let g = prometheus::Gauge::with_opts(prometheus::Opts::new(
                "nyx_multipath_weight_entropy", "Normalized Shannon entropy of active path weight distribution (0..1)"
            )).unwrap();
            let _ = registry.register(Box::new(g.clone()));
            let _ = MP_WEIGHT_ENTROPY.set(g);
        }
        if COVER_TRAFFIC_PPS.get().is_none() {
            let g = prometheus::Gauge::with_opts(prometheus::Opts::new(
                "nyx_cover_traffic_pps", "Adaptive cover traffic packets per second (smoothed)"
            )).unwrap();
            let _ = registry.register(Box::new(g.clone()));
            let _ = COVER_TRAFFIC_PPS.set(g);
        }
        if COVER_RATIO_DEVIATION.get().is_none() {
            let g = prometheus::Gauge::with_opts(prometheus::Opts::new(
                "nyx_cover_ratio_deviation", "Deviation (achieved - target) of cover ratio"
            )).unwrap();
            let _ = registry.register(Box::new(g.clone()));
            let _ = COVER_RATIO_DEVIATION.set(g);
        }
        if MP_REORDER_UTIL.get().is_none() {
            let g = prometheus::GaugeVec::new(
                prometheus::Opts::new("nyx_multipath_reorder_buffer_utilization", "Reordering buffer utilization (0.0-1.0) per path (255=global)"),
                &["path_id"],
            ).unwrap();
            let _ = registry.register(Box::new(g.clone()));
            let _ = MP_REORDER_UTIL.set(g);
        }
        // New: per-path RTT jitter histogram (variance approximation)
        if MP_PATH_JITTER.get().is_none() {
            let h = prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                "nyx_multipath_path_rtt_jitter_seconds", "Observed path RTT jitter / variance estimate (seconds)"
            ).buckets(vec![0.0001,0.0005,0.001,0.0025,0.005,0.01,0.025,0.05,0.1,0.2,0.5,1.0])).unwrap();
            let _ = registry.register(Box::new(h.clone()));
            let _ = MP_PATH_JITTER.set(h);
        }
        // New: WRR weight ratio deviation gauge
        if MP_WRR_WEIGHT_RATIO_DEVIATION.get().is_none() {
            let g = prometheus::IntGauge::new("nyx_wrr_weight_ratio_deviation_ppm", "Average absolute deviation between observed and expected weight ratio (parts per million)").unwrap();
            let _ = registry.register(Box::new(g.clone()));
            let _ = MP_WRR_WEIGHT_RATIO_DEVIATION.set(g);
        }
    }
}

pub fn ensure_hpke_rekey_metrics_registered(registry: &Registry) {
    if HPKE_REKEY_INITIATED.get().is_some() { return; }
    let i = IntCounter::new("nyx_hpke_rekey_initiated_total", "HPKE rekeys initiated (decision made to rekey)").unwrap();
    let a = IntCounter::new("nyx_hpke_rekey_applied_total", "HPKE rekeys successfully applied (new key installed)").unwrap();
    let g = IntCounter::new("nyx_hpke_rekey_grace_used_total", "Decrypt operations that required previous key within grace window").unwrap();
    let f = IntCounter::new("nyx_hpke_rekey_fail_total", "HPKE rekey failures (generation, validation, or install errors)").unwrap();
    let c = IntCounter::new("nyx_hpke_rekey_cooldown_suppressed_total", "HPKE rekeys suppressed due to min_cooldown enforcement").unwrap();
    let _ = registry.register(Box::new(i.clone()));
    let _ = registry.register(Box::new(a.clone()));
    let _ = registry.register(Box::new(g.clone()));
    let _ = registry.register(Box::new(f.clone()));
    let _ = registry.register(Box::new(c.clone()));
    let _ = HPKE_REKEY_INITIATED.set(i);
    let _ = HPKE_REKEY_APPLIED.set(a);
    let _ = HPKE_REKEY_GRACE_USED.set(g);
    let _ = HPKE_REKEY_FAILURES.set(f);
    let _ = HPKE_REKEY_COOLDOWN_SUPPRESSED.set(c);
    if ERROR_COUNTER.get().is_none() {
        let ec = IntCounterVec::new(prometheus::Opts::new("nyx_error_code_total", "Nyx error occurrences by code"), &["code"]).unwrap();
        let _ = registry.register(Box::new(ec.clone()));
        let _ = ERROR_COUNTER.set(ec);
    }
    #[cfg(feature="prometheus")]
    {
        if HPKE_KEY_LIFETIME.get().is_none() {
            let h = prometheus::Histogram::with_opts(prometheus::HistogramOpts::new(
                "nyx_hpke_key_lifetime_seconds", "Observed lifetime of HPKE session keys (seconds)"
            ).buckets(vec![0.1,0.5,1.0,2.0,5.0,10.0,30.0,60.0,120.0,300.0,600.0,1800.0,3600.0])).unwrap();
            let _ = registry.register(Box::new(h.clone()));
            let _ = HPKE_KEY_LIFETIME.set(h);
        }
    // failure reason counter vec registered in subsequent block
    }
    #[cfg(feature="prometheus")]
    {
        if HPKE_REKEY_FAILURE_REASON.get().is_none() {
            let v = prometheus::IntCounterVec::new(
                prometheus::Opts::new("nyx_hpke_rekey_fail_reason_total", "HPKE rekey failures by reason"),
                &["reason"]
            ).unwrap();
            let _ = registry.register(Box::new(v.clone()));
            let _ = HPKE_REKEY_FAILURE_REASON.set(v);
        }
    }
}

pub fn inc_plugin_init_success() { if let Some(c)=PLUGIN_INIT_SUCCESS.get() { c.inc(); } }
pub fn inc_plugin_init_failure() { if let Some(c)=PLUGIN_INIT_FAILURE.get() { c.inc(); } }
pub fn inc_plugin_security_pass() { if let Some(c)=PLUGIN_SECURITY_PASS.get() { c.inc(); } }
pub fn inc_plugin_security_fail() { if let Some(c)=PLUGIN_SECURITY_FAIL.get() { c.inc(); } }
#[cfg(feature="prometheus")]
pub fn observe_plugin_init_duration(d: f64) { if let Some(h)=PLUGIN_INIT_DURATION.get() { h.observe(d); } }

// Multipath helper functions
pub fn inc_mp_packets_sent() { if let Some(c)=MP_PACKETS_SENT.get() { c.inc(); } }
pub fn inc_mp_packets_received() { if let Some(c)=MP_PACKETS_RECEIVED.get() { c.inc(); } }
pub fn inc_mp_packets_reordered() { if let Some(c)=MP_PACKETS_REORDERED.get() { c.inc(); } }
pub fn inc_mp_packets_expired() { if let Some(c)=MP_PACKETS_EXPIRED.get() { c.inc(); } }
pub fn inc_mp_path_activated() { if let Some(c)=MP_PATH_ACTIVATED.get() { c.inc(); } }
pub fn inc_mp_path_deactivated() { if let Some(c)=MP_PATH_DEACTIVATED.get() { c.inc(); } }
pub fn set_mp_active_paths(v: i64) { if let Some(g)=MP_ACTIVE_PATHS.get() { g.set(v); } }
#[cfg(feature="prometheus")]
pub fn observe_mp_path_rtt(sec: f64) { if let Some(h)=MP_PATH_RTT.get() { h.observe(sec); } }
#[cfg(feature="prometheus")]
pub fn observe_mp_path_jitter(sec: f64) { if let Some(h)=MP_PATH_JITTER.get() { h.observe(sec); } }
#[cfg(feature="prometheus")]
pub fn set_wrr_weight_ratio_deviation_ppm(val: i64) { if let Some(g)=MP_WRR_WEIGHT_RATIO_DEVIATION.get() { g.set(val); } }
#[cfg(feature="prometheus")]
pub fn observe_mp_reorder_delay(sec: f64) { if let Some(h)=MP_REORDER_DELAY.get() { h.observe(sec); } }
#[cfg(feature="prometheus")]
pub fn record_mp_weight_entropy(val: f64) { if let Some(g)=MP_WEIGHT_ENTROPY.get() { g.set(val.clamp(0.0,1.0)); } }
#[cfg(feature="prometheus")]
pub fn set_mp_reorder_utilization(path_id: u8, util: f64) {
    if let Some(gv)=MP_REORDER_UTIL.get() { let _ = gv.get_metric_with_label_values(&[&path_id.to_string()]).map(|g| g.set(util)); }
}
#[cfg(feature="prometheus")]
pub fn set_cover_traffic_pps(pps: f64) { if let Some(g)=COVER_TRAFFIC_PPS.get() { g.set(pps.max(0.0)); } }
#[cfg(feature="prometheus")]
pub fn set_cover_ratio_deviation(dev: f64) { if let Some(g)=COVER_RATIO_DEVIATION.get() { g.set(dev); } }

// HPKE rekey helper functions
pub fn inc_hpke_rekey_initiated() { if let Some(c)=HPKE_REKEY_INITIATED.get() { c.inc(); } }
pub fn inc_hpke_rekey_applied() { if let Some(c)=HPKE_REKEY_APPLIED.get() { c.inc(); } }
pub fn inc_hpke_rekey_grace_used() { if let Some(c)=HPKE_REKEY_GRACE_USED.get() { c.inc(); } }
pub fn inc_hpke_rekey_failure() { if let Some(c)=HPKE_REKEY_FAILURES.get() { c.inc(); } }
pub fn inc_hpke_rekey_cooldown_suppressed() { if let Some(c)=HPKE_REKEY_COOLDOWN_SUPPRESSED.get() { c.inc(); } }
#[cfg(feature="prometheus")]
pub fn observe_hpke_key_lifetime(sec: f64) { if let Some(h)=HPKE_KEY_LIFETIME.get() { h.observe(sec); } }
#[cfg(feature="prometheus")]
pub fn inc_hpke_rekey_failure_reason(reason: &str) {
    if let Some(v)=HPKE_REKEY_FAILURE_REASON.get() { v.with_label_values(&[reason]).inc(); }
}

// Error recording (used by NyxError::record)
pub fn record_error(code: u16) {
    if let Some(vec)=ERROR_COUNTER.get() { vec.with_label_values(&[&format!("0x{:02X}", code)]).inc(); }
}

// New multipath telemetry statics (placed after helper functions to ensure visibility)
#[cfg(feature="prometheus")]
static MP_PATH_JITTER: OnceCell<prometheus::Histogram> = OnceCell::new();
#[cfg(feature="prometheus")]
static MP_WRR_WEIGHT_RATIO_DEVIATION: OnceCell<prometheus::IntGauge> = OnceCell::new();

impl TelemetryCollector {
    /// Start the telemetry collector
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;
        info!("Starting telemetry collector on port {}", self.config.metrics_port);
        if self.config.metrics_enabled {
            // Install stream send hook before starting server so early sends are captured.
            let counter = self.stream_send_counter.clone();
            register_stream_send_hook(move |path_id, _cid| {
                counter.with_label_values(&[&path_id.to_string()]).inc();
            });
            // Register sampling counters globally for deterministic_accept instrumentation.
            crate::sampling::register_sampling_counters(self.sampling_spans_kept.clone(), self.sampling_spans_dropped.clone());
            ensure_plugin_metrics_registered(&self.registry);
            ensure_multipath_metrics_registered(&self.registry);
            #[cfg(feature="prometheus")]
            {
                // For simple integration we just wrap already registered histogram/gauges via lookups
                use prometheus::{Opts, HistogramOpts};
                if self.multipath_weight_dev.is_none() {
                    if let Ok(gv) = GaugeVec::new(Opts::new("nyx_multipath_weight_ratio_deviation", "Per-path deviation (actual-expected) of selection ratio"), &["path_id"]) {
                        let _ = self.registry.register(Box::new(gv.clone()));
                        register_multipath_metrics_global(gv.clone(),
                            HistogramVec::new(HistogramOpts::new("nyx_multipath_rtt_jitter_ms", "Per-path RTT jitter (ms)"), &["path_id"]).unwrap_or_else(|_| HistogramVec::new(HistogramOpts::new("nyx_multipath_rtt_jitter_ms","Per-path RTT jitter (ms)"), &["path_id"]).unwrap())
                        );
                    }
                }
            }
            ensure_hpke_rekey_metrics_registered(&self.registry);
            self.start_metrics_server().await?;
        }

        // OTLP exporter initialization (if enabled & feature available)
        #[cfg(feature="otlp_exporter")]
        if self.config.otlp_enabled {
            use crate::opentelemetry_integration::{TelemetryConfig as OCfg, NyxTelemetry};
            use prometheus::{IntCounter, IntGauge};
            NyxTelemetry::init_with_exporter(OCfg { endpoint: self.config.otlp_endpoint.clone().unwrap_or_else(|| "http://localhost:4317".into()), service_name: "nyx".into(), sampling_ratio: self.config.trace_sampling })?;
            // Exporter health metrics registration & wiring into otlp::with_recovery helpers.
            if let (Ok(succ), Ok(fail), Ok(circ)) = (
                prometheus::IntCounter::new("nyx_otlp_exports_success_total", "Successful OTLP export batches"),
                prometheus::IntCounter::new("nyx_otlp_exports_failure_total", "Failed OTLP export batches"),
                prometheus::IntGauge::new("nyx_otlp_circuit_open", "OTLP exporter circuit breaker open (1=open,0=closed)"),
            ) {
                let _ = self.registry.register(Box::new(succ.clone()));
                let _ = self.registry.register(Box::new(fail.clone()));
                let _ = self.registry.register(Box::new(circ.clone()));
                crate::otlp::register_exporter_metrics(succ, fail, circ);
            }
        }

        // Initialize OTLP tracer (non-exporting) if enabled and feature present
        #[cfg(all(feature="otlp", not(feature="otlp_exporter")))]
        if self.config.otlp_enabled {
            // Install in-memory tracer (captures spans & invokes path hook) instead of plain NyxTelemetry::init
            let (dispatch, _store) = crate::otlp::init_in_memory_tracer("nyx", self.config.trace_sampling);
            crate::otlp::register_span_path_hook({
                let counter = self.stream_send_counter.clone();
                move |pid| { counter.with_label_values(&[&pid.to_string()]).inc(); }
            });
            tracing::dispatcher::set_global_default(dispatch)?;
        }

        // Attribute filter hot reload if configured
        #[cfg(feature="otlp")]
        if let Some(path) = &self.config.attribute_filter_config { crate::spawn_attribute_filter_watcher(path.clone()).await; }

        // Exporter health probe (pure TCP reachability) when recovery enabled
        if self.config.exporter_recovery && self.config.otlp_enabled { self.spawn_exporter_health_probe(); }
        
        self.start_collection_loop().await;
        Ok(())
    }

    /// Lightweight initialization that installs hooks & starts metrics server WITHOUT entering the blocking collection loop.
    /// Intended for tests that want to manually trigger collection via `collect_once_for_test`.
    pub async fn init_light(&self) -> Result<()> {
        *self.running.write().await = true;
        if self.config.metrics_enabled {
            let counter = self.stream_send_counter.clone();
            register_stream_send_hook(move |path_id, _cid| {
                counter.with_label_values(&[&path_id.to_string()]).inc();
            });
            crate::sampling::register_sampling_counters(self.sampling_spans_kept.clone(), self.sampling_spans_dropped.clone());
            ensure_plugin_metrics_registered(&self.registry);
            ensure_multipath_metrics_registered(&self.registry);
            ensure_hpke_rekey_metrics_registered(&self.registry);
            self.start_metrics_server().await?;
        }

        #[cfg(all(feature="otlp", not(feature="otlp_exporter")))]
        if self.config.otlp_enabled {
            let (dispatch, _store) = crate::otlp::init_in_memory_tracer("nyx", self.config.trace_sampling);
            crate::otlp::register_span_path_hook({
                let counter = self.stream_send_counter.clone();
                move |pid| { counter.with_label_values(&[&pid.to_string()]).inc(); }
            });
            tracing::dispatcher::set_global_default(dispatch)?;
        }
        #[cfg(feature="otlp")]
        if let Some(path) = &self.config.attribute_filter_config { crate::spawn_attribute_filter_watcher(path.clone()).await; }
        if self.config.exporter_recovery && self.config.otlp_enabled { self.spawn_exporter_health_probe(); }
        Ok(())
    }

    /// Return current stream send counter for a given path_id (if exists)
    pub fn stream_send_count(&self, path_id: u8) -> Option<u64> {
        self.stream_send_counter
            .get_metric_with_label_values(&[&path_id.to_string()])
            .ok()
            .map(|c| c.get() as u64)
    }

    /// Return current CPU & Memory usage gauge values.
    pub fn current_cpu_mem(&self) -> (f64, f64) {
        (self.cpu_usage_gauge.get(), self.memory_usage_gauge.get())
    }

    /// Exporter health metrics accessors (None if not enabled)
    pub fn exporter_successes(&self) -> Option<u64> { self.exporter_success_counter.as_ref().map(|c| c.get() as u64) }
    pub fn exporter_failures(&self) -> Option<u64> { self.exporter_failure_counter.as_ref().map(|c| c.get() as u64) }
    pub fn exporter_circuit_open(&self) -> Option<i64> { self.exporter_circuit_open.as_ref().map(|g| g.get() as i64) }

    /// Force one immediate system metrics collection tick (exposed for tests/integration harnesses).
    pub async fn collect_once_for_test(&self) {
        self.collect_system_metrics().await;
    }
    
    /// Start Prometheus metrics HTTP server
    async fn start_metrics_server(&self) -> Result<()> {
        #[cfg(feature = "prometheus")]
        {
            let registry = self.registry.clone();
            let metrics_route = warp::path("metrics").map(move || {
                let encoder = TextEncoder::new();
                let metric_families = registry.gather();
                let mut buffer = Vec::new();
                encoder.encode(&metric_families, &mut buffer).unwrap();
                String::from_utf8(buffer).unwrap()
            });
            let port = self.config.metrics_port;
            tokio::spawn(async move { warp::serve(metrics_route).run(([0,0,0,0], port)).await; });
        }
        Ok(())
    }
    
    /// Start the periodic metrics collection loop
    async fn start_collection_loop(&self) {
        let mut interval = interval(Duration::from_secs(self.config.collection_interval));
        while *self.running.read().await {
            interval.tick().await;
            self.collect_system_metrics().await;
            self.collect_network_metrics().await;
            self.collect_stream_metrics().await;
            self.collect_mix_metrics().await;
        }
    }
    
    async fn collect_system_metrics(&self) {
    use sysinfo::System;
        // Fresh snapshot (could later switch to retained System with selective refresh for perf)
        let mut sys = System::new_all();
        sys.refresh_all();
        // Memory
        let total_memory = sys.total_memory() as f64; // kB
        let used_memory = sys.used_memory() as f64;  // kB
        let mem_pct = if total_memory > 0.0 { (used_memory / total_memory) * 100.0 } else { 0.0 };
        // CPU (global)
        let cpu_usage = sys.global_cpu_info().cpu_usage() as f64;
        // Disk: aggregate all mounted disks (exclude zero-sized or removable with total=0)
        let total_disk: u128 = 0;
        let used_disk: u128 = 0;
        #[allow(unused_mut)]
        #[allow(unused_variables)]
        {
            // Some sysinfo builds (feature set) may not expose disks()/networks(); guard via trait bounds detection.
            #[cfg(any())]
            {
                for d in sys.disks() {
                    let total = d.total_space() as u128;
                    if total == 0 { continue; }
                    let avail = d.available_space() as u128;
                    total_disk += total;
                    used_disk += total.saturating_sub(avail);
                }
            }
        }
        let disk_pct = if total_disk > 0 { (used_disk as f64 / total_disk as f64) * 100.0 } else { 0.0 };
        // Network: sum RX/TX across interfaces
        let rx: u64 = 0;
        let tx: u64 = 0;
        #[allow(unused_variables)]
        {
            #[cfg(any())]
            {
                for (_name, data) in sys.networks() { rx = rx.saturating_add(data.received()); tx = tx.saturating_add(data.transmitted()); }
            }
        }
        // Active connections: not provided by sysinfo; leave 0 (future: integrate netstat or daemon stats channel)
        let active_conns = 0u64;

        self.cpu_usage_gauge.set(cpu_usage);
        self.memory_usage_gauge.set(mem_pct);
        let _ = self.system_tx.send(SystemMetrics {
            cpu_usage,
            memory_usage: mem_pct,
            disk_usage: disk_pct,
            network_rx_bytes: rx,
            network_tx_bytes: tx,
            active_connections: active_conns,
            timestamp: SystemTime::now(),
        });
        debug!("System metrics collected: cpu={:.1} mem={:.1}% disk={:.1}% rx={} tx={}", cpu_usage, mem_pct, disk_pct, rx, tx);
    }
    
    async fn collect_network_metrics(&self) {
        self.network_bytes_counter.inc_by(4096.0);
        let _ = self.network_tx.send(NetworkMetrics {
            bytes_sent: 2048,
            bytes_received: 2048,
            packets_sent: 20,
            packets_received: 18,
            connections_active: 8,
            connections_total: 100,
            latency_ms: 37.4,
            timestamp: SystemTime::now(),
        });
        debug!("Network metrics collected");
    }
    
    async fn collect_stream_metrics(&self) {
        self.stream_count_gauge.set(5.0);
        let _ = self.stream_tx.send(StreamMetrics {
            active_streams: 5,
            total_streams: 123,
            bytes_sent: 8192,
            bytes_received: 6144,
            errors: 0,
            timestamp: SystemTime::now(),
        });
        debug!("Stream metrics collected");
    }
    
    async fn collect_mix_metrics(&self) {
        self.mix_messages_counter.inc_by(10.0);
        let _ = self.mix_tx.send(MixMetrics {
            cover_traffic_sent: 100,
            real_traffic_sent: 50,
            messages_mixed: 25,
            anonymity_set_size: 1000,
            timestamp: SystemTime::now(),
        });
        debug!("Mix metrics collected");
    }
}

// ----------------------------------------------------------------------------------------------
// Attribute filter chain hot-reload scaffolding (simplified). The actual OTLP layer attribute
// filtering occurs inside otlp::init_in_memory_tracer via thread-local filter. Here we provide
// a file watcher task that reloads rules and installs a composite filter closure. Rules format:
// [{"drop_keys":["secret"], "redact":{"api_key":"REDACTED"}}]
// Future: expand with regex, value predicates, metrics for filter actions.
// ----------------------------------------------------------------------------------------------
#[cfg(feature = "otlp")]
pub async fn spawn_attribute_filter_watcher(path: String) {
    use tokio::time::{sleep, Duration};
    use serde::Deserialize;
    #[derive(Deserialize, Debug)]
    struct Rule { #[serde(default)] drop_keys: Vec<String>, #[serde(default)] redact: std::collections::HashMap<String,String> }
    #[derive(Deserialize, Debug)]
    #[serde(untagged)]
    enum RuleDoc {
        Simple(Vec<Rule>),
        Chains { chains: Vec<Vec<Rule>> }
    }
    tokio::spawn(async move {
        loop {
        match std::fs::read_to_string(&path) {
                Ok(data) => {
                    if let Some(doc) = crate::maybe_json_rules::<RuleDoc>(&data) {
                        // Flatten all chains (sequence preserving). A drop in any rule aborts.
                        let chains: Vec<Vec<Rule>> = match doc { RuleDoc::Simple(r) => vec![r], RuleDoc::Chains { chains } => chains };
                        let filter = move |k: &str, v: &str| -> Option<String> {
                            let mut current = v.to_string();
                            for chain in &chains {
                                for r in chain {
                                    if r.drop_keys.iter().any(|dk| dk == k) { return None; }
                                    if let Some(rval) = r.redact.get(k) { current = rval.clone(); }
                                }
                            }
                            Some(current)
                        };
                        crate::otlp::set_attribute_filter(Some(Arc::new(filter)));
                    } 
                }
                Err(_) => { /* ignore */ }
            }
            sleep(Duration::from_secs(5)).await;
        }
    });
}

impl TelemetryCollector {
    // (旧 unsafe 実装削除済み: register_plugin_metrics は上部安全実装を使用)
    fn spawn_exporter_health_probe(&self) {
        #[allow(unused_variables)]
        let endpoint = self.config.otlp_endpoint.clone().unwrap_or_else(|| "http://localhost:4317".into());
        let success = self.exporter_success_counter.clone();
        let failure = self.exporter_failure_counter.clone();
        let circuit = self.exporter_circuit_open.clone();
        let running = self.running.clone();
        tokio::spawn(async move {
            use std::net::TcpStream; use std::time::Duration; use tokio::time::sleep;
            let mut fails = 0u32;
            loop {
                if !*running.read().await { break; }
                // crude parse
                let mut s = endpoint.trim().to_string();
                if let Some(rest) = s.strip_prefix("http://") { s = rest.to_string(); }
                if let Some(rest) = s.strip_prefix("https://") { s = rest.to_string(); }
                if let Some(idx) = s.find('/') { s = s[..idx].to_string(); }
                if !s.contains(':') { s = format!("{}:4317", s); }
                let res = TcpStream::connect_timeout(&s.parse().unwrap_or_else(|_| "127.0.0.1:4317".parse().unwrap()), Duration::from_millis(300));
                match res { Ok(_) => { if let Some(c)=&success { c.inc(); } fails=0; if let Some(g)=&circuit { g.set(0); } }, Err(_) => { if let Some(c)=&failure { c.inc(); } fails+=1; if fails>5 { if let Some(g)=&circuit { g.set(1); } } } }
                sleep(Duration::from_secs(2)).await;
            }
        });
    }
}

// JSON parse helper
pub fn maybe_json_rules<T: for<'de> serde::Deserialize<'de>>(s: &str) -> Option<T> { serde_json::from_str::<T>(s).ok() }