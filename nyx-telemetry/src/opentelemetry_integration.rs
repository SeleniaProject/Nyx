//! OpenTelemetry integration for tracing and metrics (feature = "otlp").

#[cfg(feature = "otlp")]
use opentelemetry::KeyValue;
#[cfg(feature = "otlp")]
use opentelemetry_sdk::{runtime, trace as sdktrace, Resource};
#[cfg(feature = "otlp")]
use tracing_subscriber::{layer::SubscriberExt, Registry};

#[cfg(feature = "otlp")]
pub fn init_tracing(service_name: Option<String>) -> anyhow::Result<()> {
	// Build resource with service.name and basic attributes.
	let mut res = vec![KeyValue::new(
		opentelemetry_semantic_conventions::resource::SERVICE_NAME,
		service_name.unwrap_or_else(|| "nyx".to_string()),
	)];
	if let Ok(ver) = std::env::var("NYX_VERSION") {
		res.push(KeyValue::new(
			opentelemetry_semantic_conventions::resource::SERVICE_VERSION,
			ver,
		));
	}
	let resource = Resource::new(res);

	// Create OTLP exporter via tonic using env vars (OTEL_EXPORTER_OTLP_*).
	let otlp_exporter = opentelemetry_otlp::new_exporter().tonic();

	// Batch span processor with Tokio runtime.
	let tracer = opentelemetry_otlp::new_pipeline()
		.tracing()
		.with_trace_config(sdktrace::Config::default().with_resource(resource.clone()))
		.with_exporter(otlp_exporter.clone())
		.install_batch(runtime::Tokio)?;

	// Wire tracing -> OpenTelemetry bridge.
	let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

	// Optionally enable OTLP metrics (no-op if not configured by env in 0.29).
	let _meter_provider = {
		let metrics_exporter = opentelemetry_otlp::new_exporter().tonic();
		let provider = opentelemetry_otlp::new_pipeline()
			.metrics(runtime::Tokio)
			.with_resource(resource)
			.with_exporter(metrics_exporter)
			.build()?;
		opentelemetry::global::set_meter_provider(provider.clone());
		provider
	};

	// Install subscriber if not already installed. We add a simple fmt layer for local logs.
	let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);

	// Try setting a default subscriber; if already set, don't error.
	let subscriber = Registry::default().with(fmt_layer).with(otel_layer);
	if tracing::dispatcher::has_been_set() {
		// Respect existing global subscriber; attach our OTLP via set_default for current scope.
		tracing::dispatcher::set_default(&tracing::Dispatch::new(subscriber), || {});
	} else {
		tracing::subscriber::set_global_default(subscriber)?;
	}

	Ok(())
}

