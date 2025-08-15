#![forbid(unsafe_code)]

#[tokio::test]
async fn early_data_counter_increments() {
    use nyx_telemetry::{TelemetryCollector, TelemetryConfig};
    use prometheus::proto::MetricType as PType;

    // Initialize telemetry (light) to register counters
    let collector = TelemetryCollector::new(TelemetryConfig::default()).expect("collector");
    collector.init_light().await.expect("init_light");

    // Increment early data acceptance a couple of times
    nyx_telemetry::inc_early_data_accept();
    nyx_telemetry::inc_early_data_accept();

    // Gather metrics and find the counter
    let mfs = prometheus::gather();
    let mut found = false;
    for mf in mfs {
        if mf.get_name() == "nyx_early_data_accept_total" && mf.get_field_type() == PType::COUNTER {
            if let Some(metric) = mf.get_metric().get(0) {
                let val = metric.get_counter().get_value();
                assert!(val >= 2.0, "expected counter >= 2.0, got {}", val);
                found = true;
                break;
            }
        }
    }
    assert!(found, "early data acceptance counter not found");
}
