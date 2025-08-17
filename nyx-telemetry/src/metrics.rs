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
	handle: Option<tokio::task::JoinHandle<()>>,
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
		if let Some(h) = self.handle.take() {
			h.abort();
		}
	}
}

/// Start a background Warp server that serves `/metrics` on the given address.
#[cfg(feature = "prometheus")]
pub async fn start_http_server(
	addr: std::net::SocketAddr,
) -> crate::Result<MetricsHttpServerGuard> {
	use hyper::{Body, Request, Response, Server, StatusCode};
	use hyper::service::{make_service_fn, service_fn};

	let (tx, rx) = tokio::sync::oneshot::channel::<()>();

	// Bind using std listener for compatibility and to obtain bound addr immediately.
	let std_listener = std::net::TcpListener::bind(addr)
		.map_err(|e| crate::Error::Init(format!("failed to bind metrics server: {e}")))?;
	std_listener
		.set_nonblocking(true)
		.map_err(|e| crate::Error::Init(format!("failed to set nonblocking: {e}")))?;
	let bound_addr = std_listener
		.local_addr()
		.map_err(|e| crate::Error::Init(format!("failed to get local addr: {e}")))?;

	// Define a tiny service that only serves /metrics
	let make_svc = make_service_fn(|_conn| async move {
		Ok::<_, hyper::Error>(service_fn(|req: Request<Body>| async move {
			if req.method() == hyper::Method::GET && req.uri().path() == "/metrics" {
				let body = dump_prometheus();
				let mut resp = Response::new(Body::from(body));
				resp.headers_mut().insert(
					hyper::header::CONTENT_TYPE,
					hyper::header::HeaderValue::from_static("text/plain; version=0.0.4"),
				);
				Ok::<_, hyper::Error>(resp)
			} else {
				let mut resp = Response::new(Body::from("Not Found"));
				*resp.status_mut() = StatusCode::NOT_FOUND;
				Ok::<_, hyper::Error>(resp)
			}
		}))
	});

	let server = Server::from_tcp(std_listener)
		.map_err(|e| crate::Error::Init(format!("server from_tcp failed: {e}")))?
		.serve(make_svc)
		.with_graceful_shutdown(async move { let _ = rx.await; });

	let handle = tokio::spawn(async move {
		let _ = server.await; // discard Result<(), hyper::Error>
	});

	// Readiness probe: ensure the socket accepts connections before returning.
	// Try small, fast attempts to avoid extending test runtime.
	use tokio::time::{sleep, Duration};
	for _ in 0..100u32 {
		if tokio::net::TcpStream::connect(bound_addr).await.is_ok() {
			break;
		}
		sleep(Duration::from_millis(10)).await;
	}

	Ok(MetricsHttpServerGuard { shutdown: Some(tx), handle: Some(handle), addr: bound_addr })
}
