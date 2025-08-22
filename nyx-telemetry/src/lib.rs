#![forbid(unsafe_code)]

// --- Public error/result type_s -------------------------------------------------

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
	pub servicename: Option<String>,
}

impl Default for Config {
	fn default() -> Self {
		Self { exporter: Exporter::None, servicename: None }
	}
}

// --- Module_s ------------------------------------------------------------------

pub mod metrics;
#[cfg(feature = "otlp")]
mod opentelemetry_integration;
#[cfg(not(feature = "otlp"))]
mod opentelemetry_integration {
	use anyhow::Result;
	
	pub fn init_tracing(_servicename: Option<String>) -> Result<()> {
		use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
		if tracing::dispatcher::has_been_set() {
			return Ok(());
		}
		let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
		tracing_subscriber::registry().with(fmt_layer).try_init()?;
		Ok(())
	}
	
	pub fn shutdown() {
		// No-op when otlp feature is disabled
	}
}
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
			opentelemetry_integration::init_tracing(cfg.servicename.clone())
				.map_err(|e| Error::Init(e.to_string()))
		}
	}
}

/// Increase an IntCounter by the provided value. The counter is lazily
/// created and registered to the shared Prometheus registry upon first use.
pub fn record_counter(name: &str, v: u64) {
	self::metrics::record_counter(name, v)
}

/// Dump Prometheus metrics in text exposition format.
pub fn dump_prometheus() -> String {
	// Access the global registry directly since module path resolution is problematic
	use prometheus::{Encoder, TextEncoder};
	let metric_families = self::metrics::REGISTRY.gather();
	let encoder = TextEncoder::new();
	let mut buffer = Vec::new();
	
	match encoder.encode(&metric_families, &mut buffer) {
		Ok(()) => String::from_utf8(buffer).unwrap_or_else(|_| {
			String::from("# Prometheus metrics export failed: UTF-8 conversion error\n")
		}),
		Err(_) => String::from("# Prometheus metrics export failed: encoding error\n"),
	}
}

/// Gracefully shutdown OTLP exporters and flush spans. No-op without feature = "otlp".
pub fn shutdown() {
	opentelemetry_integration::shutdown();
}

/// Build a Warp filter that serves Prometheus metrics at path "/metrics".
/// Enabled only with feature = "prometheus". Useful for embedding into an
/// existing HTTP server.
#[cfg(feature = "prometheus")]
pub fn warp_metrics_filter(
) -> impl warp::Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
	self::metrics::warp_metrics_filter()
}

/// Start a standalone Prometheus metrics HTTP server on the given address.
/// Returns a guard that will gracefully stop the server when dropped.
#[cfg(feature = "prometheus")]
pub async fn start_metrics_http_server(
	addr: std::net::SocketAddr,
) -> Result<metrics::MetricsHttpServerGuard> {
	self::metrics::start_http_server(addr).await
}

// Small self-check
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn initnoop() -> Result<()> {
		init(&Config::default())?;
		Ok(())
	}

	#[test]
	fn counter_and_dump_smoke() -> Result<()> {
		init(&Config { exporter: Exporter::Prometheus, servicename: None })?;
		record_counter("unit_counter", 3);
		let out = dump_prometheus();
		assert!(out.contains("unit_counter"));
		Ok(())
	}
}

