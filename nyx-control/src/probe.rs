use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::{Notify, RwLock},
};

/// Handle to stop the probe server.
pub struct ProbeHandle {
    __addr: SocketAddr,
    __stop: Arc<Notify>,
    __task: tokio::task::JoinHandle<()>,
}

impl ProbeHandle {
    /// The bound socket addres_s.
    pub fn addr(&self) -> SocketAddr {
        self.__addr
    }

    /// Abort background task to guarantee prompt shutdown in test_s.
    pub async fn shutdown(self) {
        // Best-effort graceful signal (ignored if no waiter yet)
        self.__stop.notify_waiters();
        // Hard abort to avoid Notify race
        self.__task.abort();
        let _task_result = self.__task.await;
    }
}

/// Start_s a minimal HTTP probe server serving /healthz and /ready returning 200 OK.
/// Return_s the bound addres_s (useful when port 0 wa_s passed) and a shutdown handle.
pub async fn start_probe(port: u16) -> crate::Result<ProbeHandle> {
    // Bind only on loopback to avoid platform-specific firewall prompt_s in test_s
    let addr: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .map_err(|e| crate::Error::Invalid(format!("Invalid address: {e}")))?;
    let __listener = TcpListener::bind(addr).await?;
    let __local_addr = __listener.local_addr()?;
    let __stop = Arc::new(Notify::new());
    let __stop2 = __stop.clone();

    let __task = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = __stop2.notified() => break,
                acc = __listener.accept() => {
                    match acc {
                        Ok((mut sock, _peer)) => {
                            // Handle a single HTTP/1.1 request in-place, keep-alive not supported
                            tokio::spawn(async move {
                                let mut buf = [0u8; 1024];
                                let __n = match sock.read(&mut buf).await { Ok(n) => n, Err(_) => return };
                                let __req = String::from_utf8_lossy(&buf[..__n]);
                                let __path = parse_path(&__req);
                                let (__statu_s, body) = match __path.as_deref() {
                                    Some("/healthz") | Some("/ready") | Some("/livez") => ("200 OK", "ok"),
                                    _ => ("404 Not Found", "not found"),
                                };
                                let __resp = format!(
                                    "HTTP/1.1 {__statu_s}\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                                    body.len()
                                );
                                let _write_result = sock.write_all(__resp.as_bytes()).await;
                                let _shutdown_result = sock.shutdown().await;
                            });
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });

    Ok(ProbeHandle {
        __addr: __local_addr,
        __stop,
        __task,
    })
}

fn parse_path(req: &str) -> Option<String> {
    // Very small HTTP parser: expect_s first line like "GET /path HTTP/1.1"
    let mut line_s = req.split('\n');
    let __line1 = line_s.next()?.trim();
    let mut it = __line1.split_whitespace();
    let ___method = it.next()?; // only GET used here
    let __path = it.next()?;
    Some(__path.to_string())
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[tokio::test]
    async fn probe_serves_health() -> crate::Result<()> {
        let __h = start_probe(0).await?;
        let __addr = __h.addr();
        let __resp = tiny_http_get(__addr, "/healthz").await;
        assert!(__resp.contains("200 OK"));
        __h.shutdown().await;
        Ok(())
    }

    async fn tiny_http_get(__addr: SocketAddr, path: &str) -> String {
        use tokio::net::TcpStream;
        let mut _s = match TcpStream::connect(__addr).await {
            Ok(s) => s,
            Err(_) => return "connection failed".to_string(),
        };
        let __req = format!("GET {path} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n");
        let _ = _s.write_all(__req.as_bytes()).await;
        let mut out = Vec::new();
        let _ = _s.read_to_end(&mut out).await;
        String::from_utf8_lossy(&out).to_string()
    }
}

// ==================== Network Path Probing ====================

/// Network probe metrics for path quality measurement
/// 
/// These metrics feed into PathBuilder and multipath scheduler
/// to enable dynamic path selection based on real-time network conditions.
#[derive(Debug, Clone)]
pub struct NetworkProbeMetrics {
    /// Round-trip time
    pub rtt: Duration,
    /// Packet loss rate (0.0 to 1.0)
    pub loss_rate: f64,
    /// Jitter (RTT variance)
    pub jitter: Duration,
    /// Estimated bandwidth (bytes/sec)
    pub bandwidth: u64,
    /// Timestamp of measurement
    pub timestamp: Instant,
    /// Number of probes in this measurement
    pub sample_count: usize,
}

impl Default for NetworkProbeMetrics {
    fn default() -> Self {
        Self {
            rtt: Duration::from_millis(100),
            loss_rate: 0.0,
            jitter: Duration::from_millis(10),
            bandwidth: 1_000_000, // 1 MB/s default
            timestamp: Instant::now(),
            sample_count: 0,
        }
    }
}

/// Network path prober for multipath scheduling
/// 
/// Continuously measures path quality and feeds metrics to:
/// - PathBuilder for route selection
/// - Multipath scheduler for traffic distribution
/// - Connection manager for failover decisions
pub struct NetworkPathProber {
    /// Metrics per path
    metrics: Arc<RwLock<HashMap<SocketAddr, NetworkProbeMetrics>>>,
    /// Probe interval
    probe_interval: Duration,
    /// Active probe tasks
    active_tasks: Arc<RwLock<HashMap<SocketAddr, tokio::task::JoinHandle<()>>>>,
}

impl NetworkPathProber {
    /// Create new network path prober
    /// 
    /// # Arguments
    /// * `probe_interval` - How often to probe each path (default: 5 seconds)
    pub fn new(probe_interval: Duration) -> Self {
        Self {
            metrics: Arc::new(RwLock::new(HashMap::new())),
            probe_interval,
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start probing a specific path
    /// 
    /// Launches a background task that:
    /// 1. Periodically sends probe packets
    /// 2. Measures RTT, packet loss, jitter
    /// 3. Updates metrics for PathBuilder consumption
    /// 
    /// # Arguments
    /// * `local_addr` - Local bind address for probes
    /// * `target` - Target path to probe
    pub async fn start_probing(&self, local_addr: SocketAddr, target: SocketAddr) {
        let metrics = Arc::clone(&self.metrics);
        let interval = self.probe_interval;

        let task = tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);
            let mut rtt_samples = Vec::with_capacity(10);
            let mut loss_count = 0usize;
            let mut total_probes = 0usize;

            loop {
                interval_timer.tick().await;
                total_probes += 1;

                // Perform probe (simple UDP echo with timeout)
                let success = match Self::udp_probe(local_addr, target, Duration::from_secs(2)).await {
                    Ok(rtt) => {
                        rtt_samples.push(rtt);
                        // Keep last 10 samples for jitter calculation
                        if rtt_samples.len() > 10 {
                            rtt_samples.remove(0);
                        }
                        true
                    }
                    Err(_) => {
                        loss_count += 1;
                        false
                    }
                };

                // Calculate metrics
                if success && !rtt_samples.is_empty() {
                    let avg_rtt = rtt_samples.iter().sum::<Duration>() / rtt_samples.len() as u32;
                    
                    // Calculate jitter (standard deviation of RTT)
                    let jitter = if rtt_samples.len() > 1 {
                        let mean = avg_rtt.as_secs_f64();
                        let variance: f64 = rtt_samples.iter()
                            .map(|rtt| {
                                let diff = rtt.as_secs_f64() - mean;
                                diff * diff
                            })
                            .sum::<f64>() / (rtt_samples.len() - 1) as f64;
                        Duration::from_secs_f64(variance.sqrt())
                    } else {
                        Duration::ZERO
                    };

                    let loss_rate = loss_count as f64 / total_probes as f64;
                    
                    // Estimate bandwidth (simplified: 1500 bytes / avg_rtt)
                    let bandwidth = if avg_rtt > Duration::ZERO {
                        ((1500.0 / avg_rtt.as_secs_f64()) as u64).max(1000)
                    } else {
                        1_000_000
                    };

                    let new_metrics = NetworkProbeMetrics {
                        rtt: avg_rtt,
                        loss_rate,
                        jitter,
                        bandwidth,
                        timestamp: Instant::now(),
                        sample_count: rtt_samples.len(),
                    };

                    // Update shared metrics
                    {
                        let mut m = metrics.write().await;
                        m.insert(target, new_metrics);
                    }
                }
            }
        });

        // Store task handle
        {
            let mut tasks = self.active_tasks.write().await;
            tasks.insert(target, task);
        }
    }

    /// Stop probing a specific path
    pub async fn stop_probing(&self, target: &SocketAddr) {
        {
            let mut tasks = self.active_tasks.write().await;
            if let Some(task) = tasks.remove(target) {
                task.abort();
            }
        }

        // Remove metrics
        {
            let mut m = self.metrics.write().await;
            m.remove(target);
        }
    }

    /// Stop all probing
    pub async fn stop_all(&self) {
        {
            let mut tasks = self.active_tasks.write().await;
            for (_, task) in tasks.drain() {
                task.abort();
            }
        }

        {
            let mut m = self.metrics.write().await;
            m.clear();
        }
    }

    /// Get current metrics for a path
    /// 
    /// Returns metrics suitable for feeding to PathBuilder or multipath scheduler
    pub async fn get_metrics(&self, target: &SocketAddr) -> Option<NetworkProbeMetrics> {
        self.metrics.read().await.get(target).cloned()
    }

    /// Get all path metrics
    /// 
    /// Returns map of all monitored paths and their current metrics
    pub async fn get_all_metrics(&self) -> HashMap<SocketAddr, NetworkProbeMetrics> {
        self.metrics.read().await.clone()
    }

    /// Simple UDP probe implementation
    /// 
    /// Sends a small packet and measures RTT to target.
    /// This is a simplified implementation; production should use
    /// nyx-transport's PathValidator for proper PATH_CHALLENGE/RESPONSE.
    async fn udp_probe(
        local_addr: SocketAddr,
        target: SocketAddr,
        timeout: Duration,
    ) -> crate::Result<Duration> {
        use tokio::net::UdpSocket;

        let socket = UdpSocket::bind(local_addr).await?;
        let probe_data = b"PROBE";
        
        let start = Instant::now();
        socket.send_to(probe_data, target).await?;

        // Try to receive response (may timeout)
        let mut buf = [0u8; 64];
        match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
            Ok(Ok(_)) => Ok(start.elapsed()),
            Ok(Err(e)) => Err(crate::Error::Io(e)),
            Err(_) => Err(crate::Error::Invalid("Probe timeout".to_string())),
        }
    }

    /// Get path quality score (0.0 to 1.0)
    /// 
    /// Combines RTT, loss rate, and jitter into a single quality metric
    /// suitable for path ranking in multipath scheduling.
    /// 
    /// Score calculation:
    /// - Base score: 1.0
    /// - Penalty for RTT: -0.3 * (rtt / 500ms)
    /// - Penalty for loss: -0.5 * loss_rate
    /// - Penalty for jitter: -0.2 * (jitter / 50ms)
    pub async fn get_path_quality(&self, target: &SocketAddr) -> f64 {
        let metrics = match self.get_metrics(target).await {
            Some(m) => m,
            None => return 0.0, // No data = poor quality
        };

        let mut score = 1.0;

        // RTT penalty (normalize to 500ms baseline)
        let rtt_penalty = 0.3 * (metrics.rtt.as_millis() as f64 / 500.0).min(1.0);
        score -= rtt_penalty;

        // Loss penalty (direct impact)
        score -= 0.5 * metrics.loss_rate;

        // Jitter penalty (normalize to 50ms baseline)
        let jitter_penalty = 0.2 * (metrics.jitter.as_millis() as f64 / 50.0).min(1.0);
        score -= jitter_penalty;

        score.max(0.0) // Clamp to [0.0, 1.0]
    }
}

impl Default for NetworkPathProber {
    fn default() -> Self {
        Self::new(Duration::from_secs(5))
    }
}

#[cfg(test)]
mod network_probe_tests {
    use super::*;

    #[tokio::test]
    async fn network_prober_creation() {
        let prober = NetworkPathProber::new(Duration::from_secs(1));
        assert_eq!(prober.probe_interval, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn network_metrics_default() {
        let metrics = NetworkProbeMetrics::default();
        assert_eq!(metrics.rtt, Duration::from_millis(100));
        assert_eq!(metrics.loss_rate, 0.0);
        assert_eq!(metrics.bandwidth, 1_000_000);
    }

    #[tokio::test]
    async fn prober_get_metrics_empty() {
        let prober = NetworkPathProber::new(Duration::from_secs(1));
        let target: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        
        let metrics = prober.get_metrics(&target).await;
        assert!(metrics.is_none());
    }

    #[tokio::test]
    async fn prober_path_quality_no_data() {
        let prober = NetworkPathProber::new(Duration::from_secs(1));
        let target: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        
        let quality = prober.get_path_quality(&target).await;
        assert_eq!(quality, 0.0);
    }

    #[tokio::test]
    async fn prober_path_quality_good_metrics() {
        let prober = NetworkPathProber::new(Duration::from_secs(1));
        let target: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        
        // Insert good metrics
        {
            let mut metrics = prober.metrics.write().await;
            metrics.insert(target, NetworkProbeMetrics {
                rtt: Duration::from_millis(50),
                loss_rate: 0.0,
                jitter: Duration::from_millis(5),
                bandwidth: 10_000_000,
                timestamp: Instant::now(),
                sample_count: 10,
            });
        }
        
        let quality = prober.get_path_quality(&target).await;
        assert!(quality > 0.8); // Should be high quality
    }

    #[tokio::test]
    async fn prober_path_quality_poor_metrics() {
        let prober = NetworkPathProber::new(Duration::from_secs(1));
        let target: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        
        // Insert poor metrics
        {
            let mut metrics = prober.metrics.write().await;
            metrics.insert(target, NetworkProbeMetrics {
                rtt: Duration::from_millis(1000),
                loss_rate: 0.5,
                jitter: Duration::from_millis(200),
                bandwidth: 100_000,
                timestamp: Instant::now(),
                sample_count: 10,
            });
        }
        
        let quality = prober.get_path_quality(&target).await;
        assert!(quality < 0.3); // Should be poor quality
    }

    #[tokio::test]
    async fn prober_stop_all() {
        let prober = NetworkPathProber::new(Duration::from_secs(1));
        let target: SocketAddr = "127.0.0.1:9999".parse().unwrap();
        
        // Add metrics manually
        {
            let mut metrics = prober.metrics.write().await;
            metrics.insert(target, NetworkProbeMetrics::default());
        }
        
        assert!(prober.get_metrics(&target).await.is_some());
        
        // Stop all
        prober.stop_all().await;
        
        assert!(prober.get_metrics(&target).await.is_none());
    }

    #[tokio::test]
    async fn prober_get_all_metrics() {
        let prober = NetworkPathProber::new(Duration::from_secs(1));
        let target1: SocketAddr = "127.0.0.1:8001".parse().unwrap();
        let target2: SocketAddr = "127.0.0.1:8002".parse().unwrap();
        
        // Add multiple metrics
        {
            let mut metrics = prober.metrics.write().await;
            metrics.insert(target1, NetworkProbeMetrics::default());
            metrics.insert(target2, NetworkProbeMetrics::default());
        }
        
        let all_metrics = prober.get_all_metrics().await;
        assert_eq!(all_metrics.len(), 2);
        assert!(all_metrics.contains_key(&target1));
        assert!(all_metrics.contains_key(&target2));
    }
}
