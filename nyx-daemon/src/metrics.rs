#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use sysinfo::System;
use tokio::task::JoinHandle;

#[derive(Clone, Debug, Default)]
pub struct DaemonMetrics {
    pub cpu_usage_pct: f64,
    pub total_memory: u64,
    pub used_memory: u64,
    pub thread_count: usize,
}

#[derive(Clone)]
pub struct MetricsCollector {
    inner: Arc<RwLock<DaemonMetrics>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(DaemonMetrics::default())),
        }
    }

    pub fn snapshot(&self) -> DaemonMetrics {
        self.inner.read().clone()
    }

    pub fn render_prometheus(&self) -> String {
        let m = self.snapshot();
        format!(
            concat!(
                "# HELP nyx_daemon_cpu_usage_pct CPU usage percent\n",
                "# TYPE nyx_daemon_cpu_usage_pct gauge\n",
                "nyx_daemon_cpu_usage_pct {}\n",
                "# HELP nyx_daemon_memory_total_bytes Total memory bytes\n",
                "# TYPE nyx_daemon_memory_total_bytes gauge\n",
                "nyx_daemon_memory_total_bytes {}\n",
                "# HELP nyx_daemon_memory_used_bytes Used memory bytes\n",
                "# TYPE nyx_daemon_memory_used_bytes gauge\n",
                "nyx_daemon_memory_used_bytes {}\n",
                "# HELP nyx_daemon_thread_count Thread count\n",
                "# TYPE nyx_daemon_thread_count gauge\n",
                "nyx_daemon_thread_count {}\n"
            ),
            m.cpu_usage_pct, m.total_memory, m.used_memory, m.thread_count
        )
    }

    /// Spawn a background task to periodically refresh metrics.
    pub fn start_collection(self: &Arc<Self>, interval: Duration) -> JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut sys = System::new_all();
            loop {
                sys.refresh_all();
                let cpu = sys.global_cpu_info().cpu_usage() as f64;
                let total = sys.total_memory();
                let used = sys.used_memory();
                let threads = std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1);

                {
                    let mut w = this.inner.write();
                    w.cpu_usage_pct = cpu;
                    w.total_memory = total;
                    w.used_memory = used;
                    w.thread_count = threads;
                }

                tokio::time::sleep(interval).await;
            }
        })
    }
}
