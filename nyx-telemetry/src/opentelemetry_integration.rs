//! OpenTelemetry integration (feature = "otlp").

#[cfg(feature = "otlp")]
use once_cell::sync::OnceCell;
#[cfg(feature = "otlp")]
use opentelemetry::{global, KeyValue};
use opentelemetry::trace::TracerProvider;
#[cfg(feature = "otlp")]
use opentelemetry_sdk::{self as sdk, Resource};
#[cfg(feature = "otlp")]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(feature = "otlp")]
static TRACER_PROVIDER: OnceCell<sdk::trace::SdkTracerProvider> = OnceCell::new();

#[cfg(feature = "otlp")]
pub fn init_tracing(service_name: Option<String>) -> anyhow::Result<()> {
	if tracing::dispatcher::has_been_set() {
		return Ok(());
	}

	// Resource attributes (service.name, service.version)
	let svc_name = service_name.unwrap_or_else(|| "nyx".to_string());
	let resource = Resource::builder_empty()
		.with_attributes([
			KeyValue::new(opentelemetry_semantic_conventions::resource::SERVICE_NAME, svc_name),
			KeyValue::new(
				opentelemetry_semantic_conventions::resource::SERVICE_VERSION,
				env!("CARGO_PKG_VERSION"),
			),
		])
		.build();

	// OTLP gRPC exporter via tonic (endpointなどは環境変数のデフォルトに従う)
	let exporter = opentelemetry_otlp::SpanExporter::builder()
		.with_tonic()
		.build()?;

	// TracerProvider 構築:
	// - 実運用では Batch + Tokio runtime を使用
	// - テストなど Tokio ランタイム外から呼ばれた場合は Simple にフォールバック
	let builder = sdk::trace::SdkTracerProvider::builder()
		.with_resource(resource);
	let provider = if tokio::runtime::Handle::try_current().is_ok() {
		builder.with_batch_exporter(exporter).build()
	} else {
		builder.with_simple_exporter(exporter).build()
	};

	// Set global provider for downstream code if needed
	global::set_tracer_provider(provider.clone());
	// Prefer provider.scoped tracer to keep type concrete for tracing-opentelemetry
	let tracer = provider.tracer("nyx");

	// tracing層: fmt + otlp
	let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
	let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
	tracing_subscriber::registry()
		.with(fmt_layer)
		.with(otel_layer)
		.try_init()?;

	let _ = TRACER_PROVIDER.set(provider);
	Ok(())
}

#[cfg(feature = "otlp")]
pub fn shutdown() {
	if let Some(p) = TRACER_PROVIDER.get() {
		// ベストエフォートでflushとshutdown（エラーは無視）
		let _ = p.shutdown();
	}
}

