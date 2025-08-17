#![forbid(unsafe_code)]

// --- Public error/result types -------------------------------------------------

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("telemetry init failed: {0}")]
	Init(String),
}
pub type Result<T> = std::result::Result<T, Error>;

// --- Configuration -------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exporter {
	None,
	Prometheus,
	Otlp,
}

#[derive(Debug, Clone)]
pub struct Config {
	pub exporter: Exporter,
	/// Optional service name used for OTLP tracing when feature="otlp".
	pub service_name: Option<String>,
}

impl Default for Config {
	fn default() -> Self {
		Self { exporter: Exporter::None, service_name: None }
	}
}

// --- Modules ------------------------------------------------------------------

mod metrics;
#[cfg(feature = "otlp")]
mod opentelemetry_integration;
#[cfg(feature = "otlp")]
mod otlp;
mod sampling;

// --- Public API surface kept stable -------------------------------------------

/// Initialize telemetry according to the provided configuration.
/// - Prometheus path is a no-op setup because metrics are lazy-registered.
/// - OTLP path wires tracing + OpenTelemetry when the feature is enabled.
pub fn init(cfg: &Config) -> Result<()> {
	match cfg.exporter {
		Exporter::None => Ok(()),
		Exporter::Prometheus => Ok(()),
		Exporter::Otlp => {
			#[cfg(feature = "otlp")]
			{
				opentelemetry_integration::init_tracing(cfg.service_name.clone())
					.map_err(|e| Error::Init(e.to_string()))
			}
			#[cfg(not(feature = "otlp"))]
			{
				Err(Error::Init("otlp feature not enabled".to_string()))
			}
		}
	}
}

/// Increase an IntCounter by the provided value. The counter is lazily
/// created and registered to the shared Prometheus registry upon first use.
pub fn record_counter(name: &str, v: u64) {
	metrics::record_counter(name, v)
}

/// Dump Prometheus metrics in text exposition format.
pub fn dump_prometheus() -> String {
	metrics::dump_prometheus()
}

/// Gracefully shutdown OTLP exporters and flush spans. No-op without feature = "otlp".
#[cfg(feature = "otlp")]
pub fn shutdown() {
	opentelemetry_integration::shutdown();
}

/// Build a Warp filter that serves Prometheus metrics at path "/metrics".
/// Enabled only with feature = "prometheus". Useful for embedding into an
/// existing HTTP server.
#[cfg(feature = "prometheus")]
pub fn warp_metrics_filter(
) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
	metrics::warp_metrics_filter()
}

/// Start a standalone Prometheus metrics HTTP server on the given address.
/// Returns a guard that will gracefully stop the server when dropped.
#[cfg(feature = "prometheus")]
pub async fn start_metrics_http_server(
	addr: std::net::SocketAddr,
) -> Result<metrics::MetricsHttpServerGuard> {
	metrics::start_http_server(addr).await
}

// Small self-check
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn init_noop() {
		init(&Config::default()).unwrap();
	}

	#[test]
	fn counter_and_dump_smoke() {
		init(&Config { exporter: Exporter::Prometheus, service_name: None }).unwrap();
		record_counter("unit_counter", 3);
		let out = dump_prometheus();
		assert!(out.contains("unit_counter"));
	}
}

