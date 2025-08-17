//! Metrics utilities and Prometheus exposition.

use once_cell::sync::Lazy;
use prometheus::{Encoder, IntCounter, Registry, TextEncoder};
use std::collections::HashMap;
use std::sync::Mutex;

pub(crate) static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);
static COUNTERS: Lazy<Mutex<HashMap<String, IntCounter>>> =
	Lazy::new(|| Mutex::new(HashMap::new()));

/// Record into an IntCounter, creating and registering it on first use.
pub fn record_counter(name: &str, v: u64) {
	let mut map = COUNTERS
		.lock()
		.expect("metrics::COUNTERS mutex poisoned");
	let ctr = map.entry(name.to_string()).or_insert_with(|| {
		// Create a counter with a simple help string; names should already be sanitized by callers.
		let c = IntCounter::new(name, format!("counter {name}")).expect("create IntCounter");
		// Best-effort register; ignore error if it was already registered with a compatible type.
		let _ = REGISTRY.register(Box::new(c.clone()));
		c
	});
	ctr.inc_by(v);
}

/// Dump metrics in Prometheus text exposition format.
pub fn dump_prometheus() -> String {
	let mf = REGISTRY.gather();
	let enc = TextEncoder::new();
	let mut buf = Vec::new();
	if enc.encode(&mf, &mut buf).is_ok() {
		String::from_utf8(buf).unwrap_or_default()
	} else {
		String::new()
	}
}

#[cfg(feature = "prometheus")]
use warp::{Filter, Rejection, Reply};

/// Provide a Warp filter that serves "/metrics" with text exposition.
#[cfg(feature = "prometheus")]
pub fn warp_metrics_filter(
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
	// Build route once; cloneable filter returned.
	warp::path("metrics").and(warp::get()).map(|| {
		let body = dump_prometheus();
		warp::reply::with_header(body, "Content-Type", "text/plain; version=0.0.4")
	})
}

/// A guard that stops the metrics HTTP server when dropped.
#[cfg(feature = "prometheus")]
pub struct MetricsHttpServerGuard {
	shutdown: Option<tokio::sync::oneshot::Sender<()>>,
	addr: std::net::SocketAddr,
}

#[cfg(feature = "prometheus")]
impl MetricsHttpServerGuard {
	/// Returns the bound address of the server.
	pub fn addr(&self) -> std::net::SocketAddr { self.addr }
}

#[cfg(feature = "prometheus")]
impl Drop for MetricsHttpServerGuard {
	fn drop(&mut self) {
		// Ignore send error if the server already shut down.
		if let Some(tx) = self.shutdown.take() {
			let _ = tx.send(());
		}
	}
}

/// Start a background Warp server that serves `/metrics` on the given address.
#[cfg(feature = "prometheus")]
pub async fn start_http_server(
	addr: std::net::SocketAddr,
) -> crate::Result<MetricsHttpServerGuard> {
	let (tx, rx) = tokio::sync::oneshot::channel::<()>();
	// Bind the server first to learn the actual address.
	let route = warp_metrics_filter();
	let make_server = warp::serve(route);

	// Warp cannot directly give us a graceful shutdown handle from serve().
	// We run it on a dedicated task and use oneshot to signal shutdown.
	let (bound_addr, server) = make_server.bind_with_graceful_shutdown(addr, async move {
		// Wait for shutdown signal
		let _ = rx.await;
	});
	tokio::spawn(server);
	Ok(MetricsHttpServerGuard { shutdown: Some(tx), addr: bound_addr })
}
