#![cfg(feature = "otlp")]

#[tokio::test(flavor = "current_thread")]
async fn otlp_init_smoke() {
	let mut cfg = nyx_telemetry::Config::default();
	cfg.exporter = nyx_telemetry::Exporter::Otlp;
	cfg.servicename = Some("nyx-test".into());
	// Should not panic; may fail if feature wired wrongly.
	nyx_telemetry::init(&cfg)?;
}

