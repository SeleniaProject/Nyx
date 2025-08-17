//! OpenTelemetry integration (feature = "otlp").

#[cfg(feature = "otlp")]
use once_cell::sync::OnceCell;
#[cfg(feature = "otlp")]
use opentelemetry::{global, KeyValue};
use opentelemetry::trace::TracerProvider;
#[cfg(feature = "otlp")]
use opentelemetry_sdk::{self as sdk, Resource};
#[cfg(feature = "otlp")]
use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor};
#[cfg(feature = "otlp")]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
#[cfg(feature = "otlp")]
use std::time::Duration;
#[cfg(feature = "otlp")]
use opentelemetry_otlp::WithExportConfig; // for .with_timeout()

#[cfg(feature = "otlp")]
static TRACER_PROVIDER: OnceCell<sdk::trace::SdkTracerProvider> = OnceCell::new();
#[cfg(feature = "otlp")]
static USED_SIMPLE_PROCESSOR: OnceCell<bool> = OnceCell::new();

#[cfg(feature = "otlp")]
#[inline]
fn env_duration_ms(key: &str, default_ms: u64) -> Duration {
	std::env::var(key)
		.ok()
		.and_then(|s| s.parse::<u64>().ok())
		.map(Duration::from_millis)
		.unwrap_or(Duration::from_millis(default_ms))
}

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

	// Exporter gRPC timeout (ms) from env with sensible defaults.
	let otlp_timeout = env_duration_ms(
		"OTEL_EXPORTER_OTLP_TRACES_TIMEOUT",
		std::env::var("OTEL_EXPORTER_OTLP_TIMEOUT")
			.ok()
			.and_then(|s| s.parse::<u64>().ok())
			.unwrap_or(5000),
	);
	let exporter = opentelemetry_otlp::SpanExporter::builder()
		.with_tonic()
		.with_timeout(otlp_timeout)
		.build()?;

	// TracerProvider builder
	let mut builder = sdk::trace::SdkTracerProvider::builder().with_resource(resource);

	// Batch config knobs (ms). When schedule_delay == 0, use SimpleSpanProcessor to
	// avoid shutdown waits in tests and minimal envs.
	let schedule_delay = env_duration_ms("OTEL_BSP_SCHEDULE_DELAY", 5000);

	let use_batch = tokio::runtime::Handle::try_current().is_ok() && schedule_delay > Duration::from_millis(0);
	eprintln!(
		"nyx-telemetry:init schedule_delay_ms={} use_batch={}",
		schedule_delay.as_millis(),
		use_batch
	);
	// SimpleSpanProcessor を使う場合、エクスポートで待たないようサンプラーを AlwaysOff に
	// 切り替えてテストや最小環境でのハングを防ぐ。
	if !use_batch {
		builder = builder.with_sampler(sdk::trace::Sampler::AlwaysOff);
	}

	let provider = if use_batch {
		let batch_cfg = BatchConfigBuilder::default()
			.with_scheduled_delay(schedule_delay)
			.build();
		let bsp = BatchSpanProcessor::builder(exporter)
			.with_batch_config(batch_cfg)
			.build();
		builder.with_span_processor(bsp).build()
	} else {
		// Fallback to SimpleSpanProcessor when no runtime or schedule_delay == 0
		builder.with_simple_exporter(exporter).build()
	};

	// Set global provider and init tracing layers
	global::set_tracer_provider(provider.clone());
	let tracer = provider.tracer("nyx");
	let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
	let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);
	tracing_subscriber::registry().with(fmt_layer).with(otel_layer).try_init()?;

	let _ = TRACER_PROVIDER.set(provider);
	let _ = USED_SIMPLE_PROCESSOR.set(!use_batch);
	eprintln!("nyx-telemetry:init used_simple_processor={}", !use_batch);
	Ok(())
}

#[cfg(feature = "otlp")]
pub fn shutdown() {
	if let Some(p) = TRACER_PROVIDER.get() {
	// SimpleSpanProcessor 下では長い待ちを避けて即時戻る。
	if let Some(true) = USED_SIMPLE_PROCESSOR.get() {
		eprintln!("nyx-telemetry:shutdown fast-return (SimpleSpanProcessor)");
		return;
	}
	let _ = p.shutdown();
	}
}

