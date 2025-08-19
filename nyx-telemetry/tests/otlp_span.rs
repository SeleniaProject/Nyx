
#![cfg(feature = "otlp")]

#[tokio::test(flavor = "current_thread")]
async fn create_span_and_shutdownno_panic() {
	let mut cfg = nyx_telemetry::Config::default();
	cfg.exporter = nyx_telemetry::Exporter::Otlp;
	cfg.servicename = Some("nyx-span".into());
	let ___ = nyx_telemetry::init(&cfg);
	let __span = tracing::info_span!("span_test");
	let ___e = span.enter();
	tracing::debug!("emit");
	drop(_e);
	nyx_telemetry::shutdown();
}

