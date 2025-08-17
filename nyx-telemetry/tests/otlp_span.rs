
#![cfg(feature = "otlp")]

#[tokio::test(flavor = "current_thread")]
async fn create_span_and_shutdown_no_panic() {
	let mut cfg = nyx_telemetry::Config::default();
	cfg.exporter = nyx_telemetry::Exporter::Otlp;
	cfg.service_name = Some("nyx-span".into());
	let _ = nyx_telemetry::init(&cfg);
	let span = tracing::info_span!("span_test");
	let _e = span.enter();
	tracing::debug!("emit");
	drop(_e);
	nyx_telemetry::shutdown();
}

