use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{SystemTime, Duration, Instant};
use tokio::sync::{RwLock, Mutex};
use tokio::time::interval;

// 共有パス性能モニタモジュール (CLI/SDK/Daemon で再利用予定)

const PERFORMANCE_SAMPLE_WINDOW: usize = 100;
const MONITORING_INTERVAL_SECS: u64 = 5;
const PERFORMANCE_ALERT_THRESHOLD: f64 = 0.30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PerformanceTrend { Ascending, Descending, Stable, Volatile, Unknown }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PathPerformanceMetrics {
    pub current_latency_ms: f64,
    pub avg_latency_ms: f64,
    pub current_bandwidth_mbps: f64,
    pub avg_bandwidth_mbps: f64,
    pub packet_loss_rate: f64,
    pub reliability_score: f64,
    pub throughput_efficiency: f64,
    pub successful_transmissions: u64,
    pub failed_transmissions: u64,
    pub bytes_transmitted: u64,
    pub bytes_received: u64,
    pub last_updated: SystemTime,
    pub performance_trend: PerformanceTrend,
}
impl Default for PathPerformanceMetrics { fn default() -> Self { Self { current_latency_ms:0.0, avg_latency_ms:0.0, current_bandwidth_mbps:0.0, avg_bandwidth_mbps:0.0, packet_loss_rate:0.0, reliability_score:1.0, throughput_efficiency:1.0, successful_transmissions:0, failed_transmissions:0, bytes_transmitted:0, bytes_received:0, last_updated:SystemTime::now(), performance_trend:PerformanceTrend::Stable } } }

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PerformanceDataPoint { pub timestamp: SystemTime, pub latency_ms: f64, pub bandwidth_mbps: f64, pub packet_loss_rate: f64, pub reliability_score: f64 }

pub struct PathPerformanceMonitor {
    metrics: Arc<RwLock<PathPerformanceMetrics>>,
    history: Arc<RwLock<VecDeque<PerformanceDataPoint>>>,
    latency_samples: Arc<RwLock<VecDeque<f64>>>,
    bandwidth_samples: Arc<RwLock<VecDeque<f64>>>,
    monitoring_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    alert_callback: Arc<Mutex<Option<Box<dyn Fn(&PathPerformanceMetrics)+Send+Sync>>>>,
    enabled: Arc<std::sync::atomic::AtomicBool>,
    path_id: String,
}

impl PathPerformanceMonitor {
    pub fn new(path_id: impl Into<String>) -> Self { Self { metrics:Arc::new(RwLock::new(PathPerformanceMetrics::default())), history:Arc::new(RwLock::new(VecDeque::with_capacity(PERFORMANCE_SAMPLE_WINDOW))), latency_samples:Arc::new(RwLock::new(VecDeque::with_capacity(PERFORMANCE_SAMPLE_WINDOW))), bandwidth_samples:Arc::new(RwLock::new(VecDeque::with_capacity(PERFORMANCE_SAMPLE_WINDOW))), monitoring_task:Arc::new(Mutex::new(None)), alert_callback:Arc::new(Mutex::new(None)), enabled:Arc::new(std::sync::atomic::AtomicBool::new(false)), path_id: path_id.into() } }

    pub async fn start_monitoring(&self) -> Result<(), Box<dyn std::error::Error+Send+Sync>> {
        if self.enabled.swap(true, std::sync::atomic::Ordering::SeqCst) { return Ok(()); }
        let metrics = self.metrics.clone();
        let history = self.history.clone();
        let _latency_samples = self.latency_samples.clone(); // 予約: 将来の詳細統計用
        let _bandwidth_samples = self.bandwidth_samples.clone(); // 予約
        let alert_cb = self.alert_callback.clone();
        let enabled = self.enabled.clone();
        let _path_id = self.path_id.clone();
        let handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(MONITORING_INTERVAL_SECS));
            while enabled.load(std::sync::atomic::Ordering::SeqCst) {
                ticker.tick().await;
                let mut m = metrics.write().await;
                let hist = history.read().await;
                if hist.len() >= 5 {
                    let recent: Vec<_> = hist.iter().rev().take(10).collect();
                    let mut latency_delta = 0.0;
                    let mut reliability_delta = 0.0;
                    let mut rel_vals = Vec::new();
                    for w in recent.windows(2) {
                        let (older, newer) = (w[1], w[0]);
                        latency_delta += newer.latency_ms - older.latency_ms;
                        reliability_delta += newer.reliability_score - older.reliability_score;
                        rel_vals.push(newer.reliability_score);
                    }
                    let volatility = if rel_vals.len() > 1 {
                        let mean = rel_vals.iter().sum::<f64>() / rel_vals.len() as f64;
                        (rel_vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / rel_vals.len() as f64).sqrt()
                    } else { 0.0 };
                    let overall = reliability_delta - latency_delta / 100.0;
                    m.performance_trend = if volatility > 0.3 { PerformanceTrend::Volatile } else if overall > 0.1 { PerformanceTrend::Ascending } else if overall < -0.1 { PerformanceTrend::Descending } else { PerformanceTrend::Stable };
                    if m.reliability_score < PERFORMANCE_ALERT_THRESHOLD {
                        if let Some(cb) = alert_cb.lock().await.as_ref() { cb(&m); }
                    }
                    let point = super::path_monitor::PerformanceDataPoint { timestamp: SystemTime::now(), latency_ms: m.current_latency_ms, bandwidth_mbps: m.current_bandwidth_mbps, packet_loss_rate: m.packet_loss_rate, reliability_score: m.reliability_score };
                    drop(m); drop(hist);
                    let mut hw = history.write().await; hw.push_back(point); if hw.len() > PERFORMANCE_SAMPLE_WINDOW { hw.pop_front(); }
                }
            }
        });
        *self.monitoring_task.lock().await = Some(handle);
        Ok(())
    }
    pub async fn stop_monitoring(&self) { self.enabled.store(false, std::sync::atomic::Ordering::SeqCst); if let Some(h)=self.monitoring_task.lock().await.take(){ h.abort(); } }
    pub async fn record_latency(&self, latency_ms: f64) { let mut s = self.latency_samples.write().await; s.push_back(latency_ms); if s.len()>PERFORMANCE_SAMPLE_WINDOW { s.pop_front(); } let avg = s.iter().sum::<f64>()/s.len() as f64; let mut m = self.metrics.write().await; m.current_latency_ms = latency_ms; m.avg_latency_ms = avg; m.last_updated = SystemTime::now(); }
    pub async fn record_bandwidth(&self, bandwidth_mbps: f64) { let mut s = self.bandwidth_samples.write().await; s.push_back(bandwidth_mbps); if s.len()>PERFORMANCE_SAMPLE_WINDOW { s.pop_front(); } let avg = s.iter().sum::<f64>()/s.len() as f64; let mut m = self.metrics.write().await; m.current_bandwidth_mbps = bandwidth_mbps; m.avg_bandwidth_mbps = avg; m.last_updated = SystemTime::now(); }
    pub async fn record_transmission(&self, bytes_tx:u64, bytes_rx:u64, success: bool){ let mut m = self.metrics.write().await; m.bytes_transmitted += bytes_tx; m.bytes_received += bytes_rx; if success { m.successful_transmissions +=1; } else { m.failed_transmissions +=1; } let total = m.successful_transmissions + m.failed_transmissions; if total>0 { m.reliability_score = m.successful_transmissions as f64 / total as f64; m.packet_loss_rate = 1.0 - m.reliability_score; m.throughput_efficiency = m.reliability_score; } m.last_updated = SystemTime::now(); }
    pub async fn set_alert_callback<F:Fn(&PathPerformanceMetrics)+Send+Sync+'static>(&self, cb:F){ *self.alert_callback.lock().await = Some(Box::new(cb)); }
    pub async fn get_metrics(&self) -> PathPerformanceMetrics { self.metrics.read().await.clone() }
    pub async fn get_history(&self) -> Vec<PerformanceDataPoint> { self.history.read().await.iter().cloned().collect() }
    pub async fn analyze_performance_trend(&self) -> PerformanceTrend { self.get_metrics().await.performance_trend }
    pub async fn generate_performance_report(&self) -> String { let m = self.get_metrics().await; format!("Path Performance Report - {}\nLatency: {:.2}ms (avg {:.2})\nBandwidth: {:.2}Mbps (avg {:.2})\nReliability: {:.2}%\nThroughputEff: {:.2}%\nSuccess: {} Failed: {}\nTxBytes: {} RxBytes: {}\nTrend: {:?}", self.path_id, m.current_latency_ms, m.avg_latency_ms, m.current_bandwidth_mbps, m.avg_bandwidth_mbps, m.reliability_score*100.0, m.throughput_efficiency*100.0, m.successful_transmissions, m.failed_transmissions, m.bytes_transmitted, m.bytes_received, m.performance_trend) }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GlobalPathStats { pub active_paths:u32, pub avg_performance_score:f64, pub global_packet_loss_rate:f64, pub total_successful_transmissions:u64, pub total_failed_transmissions:u64, pub monitoring_uptime_secs:u64, pub best_performing_path:Option<String>, pub worst_performing_path:Option<String>, pub last_updated:SystemTime }
impl Default for GlobalPathStats { fn default() -> Self { Self { active_paths:0, avg_performance_score:1.0, global_packet_loss_rate:0.0, total_successful_transmissions:0, total_failed_transmissions:0, monitoring_uptime_secs:0, best_performing_path:None, worst_performing_path:None, last_updated:SystemTime::now() } } }

pub struct PathPerformanceRegistry { monitors: RwLock<HashMap<String, Arc<PathPerformanceMonitor>>>, started_at: Instant }
impl PathPerformanceRegistry { pub fn new()->Self{ Self { monitors:RwLock::new(HashMap::new()), started_at:Instant::now() } } pub async fn get_or_create(&self, id:&str)->Arc<PathPerformanceMonitor>{ if let Some(m)=self.monitors.read().await.get(id){ return m.clone(); } let mut w=self.monitors.write().await; let e=w.entry(id.to_string()).or_insert_with(||Arc::new(PathPerformanceMonitor::new(id.to_string()))); e.clone() } pub async fn global_stats(&self)->GlobalPathStats { let map=self.monitors.read().await; let mut sum_score=0.0; let mut sum_loss=0.0; let mut total=0u64; let mut best:Option<(String,f64)>=None; let mut worst:Option<(String,f64)>=None; let mut success=0u64; let mut fail=0u64; for (id,m) in map.iter(){ let met=m.get_metrics().await; let score=(met.reliability_score + (1.0/(1.0+met.avg_latency_ms/100.0)))/2.0; sum_score += score; sum_loss += met.packet_loss_rate; total += 1; success += met.successful_transmissions; fail += met.failed_transmissions; match best { Some((_,b)) if score <= b => {}, _ => best = Some((id.clone(),score)) } match worst { Some((_,w)) if score >= w => {}, _ => worst = Some((id.clone(),score)) } } GlobalPathStats { active_paths: total as u32, avg_performance_score: if total>0 { sum_score/total as f64 } else {1.0}, global_packet_loss_rate: if total>0 { sum_loss/total as f64 } else {0.0}, total_successful_transmissions: success, total_failed_transmissions: fail, monitoring_uptime_secs: self.started_at.elapsed().as_secs(), best_performing_path: best.map(|(k,_)|k), worst_performing_path: worst.map(|(k,_)|k), last_updated:SystemTime::now() } } }

#[cfg(test)]
mod tests { use super::*; #[tokio::test] async fn basic_flow(){ let m=PathPerformanceMonitor::new("p1"); m.start_monitoring().await.unwrap(); m.record_latency(10.0).await; m.record_bandwidth(50.0).await; m.record_transmission(100,100,true).await; let met=m.get_metrics().await; assert_eq!(met.current_latency_ms,10.0); assert_eq!(met.current_bandwidth_mbps,50.0); m.stop_monitoring().await; } }
