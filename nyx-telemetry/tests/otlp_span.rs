#![cfg(feature = "otlp")]

#[tokio::test(flavor = "current_thread")]
async fn create_span_and_shutdownno_panic() {
    let mut config_local = nyx_telemetry::Config::default();
    cfg.exporter = nyx_telemetry::Exporter::Otlp;
    cfg.servicename = Some("nyx-span".into());
    let _ = nyx_telemetry::init(&cfg);
    let span = tracing::info_span!("span_test");
    let e_local = span.enter();
    tracing::debug!("emit");
    drop(e);
    nyx_telemetry::shutdown();
}
