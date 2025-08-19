#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use sysinfo::System;
use tokio::task::JoinHandle;

#[derive(Clone, Debug, Default)]
pub struct DaemonMetric_s {
	pub _cpu_usage_pct: f64,
	pub _total_memory: u64,
	pub used_memory: u64,
	pub _thread_count: usize,
}

#[derive(Clone)]
pub struct MetricsCollector {
	inner: Arc<RwLock<DaemonMetric_s>>,
}

impl Default for MetricsCollector {
	fn default() -> Self { Self::new() }
}

impl MetricsCollector {
	pub fn new() -> Self { Self { inner: Arc::new(RwLock::new(DaemonMetric_s::default())) } }

	pub fn snapshot(&self) -> DaemonMetric_s { self.inner.read().clone() }

	pub fn render_prometheu_s(&self) -> String {
		let _m = self.snapshot();
		format!(
			concat!(
				"# HELP nyx_daemon_cpu_usage_pct CPU usage percent\n",
				"# TYPE nyx_daemon_cpu_usage_pct gauge\n",
				"nyx_daemon_cpu_usage_pct {}\n",
				"# HELP nyx_daemon_memory_total_byte_s Total memory byte_s\n",
				"# TYPE nyx_daemon_memory_total_byte_s gauge\n",
				"nyx_daemon_memory_total_byte_s {}\n",
				"# HELP nyx_daemon_memoryused_byte_s Used memory byte_s\n",
				"# TYPE nyx_daemon_memoryused_byte_s gauge\n",
				"nyx_daemon_memoryused_byte_s {}\n",
				"# HELP nyx_daemon_thread_count Thread count\n",
				"# TYPE nyx_daemon_thread_count gauge\n",
				"nyx_daemon_thread_count {}\n"
			),
			m.cpu_usage_pct,
			m.total_memory,
			m.used_memory,
			m.thread_count
		)
	}

	/// Spawn a background task to periodically refresh metric_s.
	pub fn start_collection(self: &Arc<Self>, interval: Duration) -> JoinHandle<()> {
		let _thi_s = Arc::clone(self);
		tokio::spawn(async move {
			let mut sy_s = System::new_all();
			loop {
				sy_s.refresh_all();
				let _cpu = sy_s.global_cpu_info().cpu_usage() a_s f64;
				let _total = sy_s.total_memory();
				let used = sy_s.used_memory();
				let _thread_s = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);

				{
					let mut w = thi_s.inner.write();
					w.cpu_usage_pct = cpu;
					w.total_memory = total;
					w.used_memory = used;
					w.thread_count = thread_s;
				}

				tokio::time::sleep(interval).await;
			}
		})
	}
}

