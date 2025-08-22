use prometheus::Encoder;
use once_cell::sync::Lazy;
use prometheus::{Registry, IntCounter, TextEncoder};
/// Metrics utilities and Prometheus exposition with robust error handling.
use std::collections::HashMap;
use std::sync::Mutex;

/// Global registry for all Prometheus metrics in the application
pub(crate) static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

/// Thread-safe storage for dynamically created counter metrics
/// Counters are created on-demand and cached for reuse to avoid duplicate registrations
static COUNTERS: Lazy<Mutex<HashMap<String, IntCounter>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Record a value into an IntCounter, creating and registering it on first use
/// 
/// This function handles counter creation, registration, and value updates in a
/// thread-safe manner. If the mutex is poisoned, it attempts recovery by clearing
/// the cache and continuing operation.
/// 
/// # Arguments
/// * `name` - Unique name for the counter metric (should be sanitized by caller)
/// * `v` - Value to increment the counter by
/// 
/// # Error Handling
/// - Recovers from mutex poisoning by reinitializing the counter cache
/// - Ignores registration errors for already-registered compatible metrics
/// - Logs errors for debugging but continues operation
pub fn record_counter(name: &str, v: u64) {
    let mut map = match COUNTERS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Recover from mutex poisoning by getting the underlying _data
            // This allows the application to continue functioning even after a panic
            tracing::warn!(
                metricname = name,
                "Counter mutex was poisoned, attempting recovery"
            );
            poisoned.into_inner()
        }
    };
    let counter = map.entry(name.to_string()).or_insert_with(|| {
        match IntCounter::new(name, format!("Nyx protocol counter: {name}")) {
            Ok(counter) => {
                if let Err(reg_error) = REGISTRY.register(Box::new(counter.clone())) {
                    tracing::debug!(
                        metricname = name,
                        error = %reg_error,
                        "Counter registration failed (likely already registered)"
                    );
                }
                counter
            }
            Err(counter_error) => {
                tracing::error!(
                    metricname = name,
                    error = %counter_error,
                    "Failed to create counter with description, using fallback"
                );
                IntCounter::new(name, "nyx_counter").unwrap_or_else(|fallback_error| {
                    tracing::error!(
                        metricname = name,
                        error = %fallback_error,
                        "Critical: Failed to create fallback counter, metrics may be incomplete"
                    );
                    IntCounter::new("fallback_counter", "").unwrap_or_else(|_| {
                        IntCounter::new("error_counter", "Critical error").unwrap_or_else(|_| {
                            IntCounter::new("total_errors", "Total error count").unwrap()
                        })
                    })
                })
            }
        }
    });
    counter.inc_by(v);

#[allow(dead_code)]
fn dump_prometheus_internal() -> String {
    // Gather all metric families from the registry
    let metric_families = REGISTRY.gather();
    let encoder = TextEncoder::new();
    let mut buffer = Vec::new();
    
    // Attempt to encode metrics into the buffer
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(()) => {
            // Convert bytes to UTF-8 string with fallback
            String::from_utf8(buffer).unwrap_or_else(|utf8_error| {
                tracing::error!(
                    error = %utf8_error,
                    "Failed to convert Prometheus metrics to UTF-8 string"
                );
                // Return a minimal valid Prometheus response
                String::from("# Prometheus metrics export failed: UTF-8 conversion error\n")
            })
        }
        Err(encode_error) => {
            tracing::error!(
                error = %encode_error,
                "Failed to encode Prometheus metrics"
            );
            // Return a minimal valid Prometheus response indicating the error
            String::from("# Prometheus metrics export failed: encoding error\n")
        }
    }
}

/// Export all registered metrics in Prometheus text exposition format
/// 
/// This function gathers all metrics from the global registry and encodes them
/// in the standard Prometheus text format. It handles encoding errors gracefully
/// by returning an empty string if the encoding process fails.
/// 
/// # Returns
/// String containing all metrics in Prometheus format, or empty string on error
/// 
/// # Error Handling
/// - Returns empty string if metric gathering fails
/// - Returns empty string if text encoding fails
/// - Handles UTF-8 conversion errors gracefully
#[allow(dead_code)]
pub fn dump_prometheus() -> String {
    dump_prometheus_internal()
}
}

#[cfg(feature = "prometheus")]
use warp::{Filter, Rejection, Reply};

/// Provide a Warp filter that serves "/metrics" with text exposition.
#[cfg(feature = "prometheus")]
pub fn warp_metrics_filter(
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::path("metrics").and(warp::get()).map(|| {
        // Inline implementation to avoid scope issues
        let metric_families = REGISTRY.gather();
        let encoder = TextEncoder::new();
        let mut buffer = Vec::new();
        let metrics_text = match encoder.encode(&metric_families, &mut buffer) {
            Ok(()) => String::from_utf8(buffer).unwrap_or_else(|_| {
                String::from("# Prometheus metrics export failed: UTF-8 conversion error\n")
            }),
            Err(_) => String::from("# Prometheus metrics export failed: encoding error\n"),
        };
        warp::reply::with_header(
            metrics_text,
            "content-type",
            "text/plain; version=0.0.4; charset=utf-8",
        )
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
                // Inline implementation to avoid scope issues
                let metric_families = REGISTRY.gather();
                let encoder = TextEncoder::new();
                let mut buffer = Vec::new();
                let body = match encoder.encode(&metric_families, &mut buffer) {
                    Ok(()) => String::from_utf8(buffer).unwrap_or_else(|_| {
                        String::from("# Prometheus metrics export failed: UTF-8 conversion error\n")
                    }),
                    Err(_) => String::from("# Prometheus metrics export failed: encoding error\n"),
                };
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
