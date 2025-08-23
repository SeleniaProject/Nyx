#![cfg(feature = "otlp")]

#[tokio::test(flavor = "current_thread")]
async fn create_span_and_shutdownno_panic() {
    let config_local = nyx_telemetry::Config {
        exporter: nyx_telemetry::Exporter::Otlp,
        servicename: Some("nyx-span".into()),
        ..Default::default()
    };
    config_local.servicename = Some("nyx-span".into());
    let _ = nyx_telemetry::init(&config_local);
    let span = tracing::info_span!("span_test");
    let e_local = span.enter();
    tracing::debug!("emit");
    drop(e_local);
    nyx_telemetry::shutdown();
}
