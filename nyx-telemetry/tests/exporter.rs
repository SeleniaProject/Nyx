use nyx_telemetry::{TelemetryConfig, TelemetryCollector};
use hyper::{Client, body::to_bytes};
use hyper::client::HttpConnector;

#[tokio::test]
async fn exporter_serves_metrics() {
    let config = TelemetryConfig {
        metrics_enabled: true,
        metrics_port: 9898,
        collection_interval: 1,
        otlp_enabled: false,
        otlp_endpoint: None,
        trace_sampling: 1.0,
        attribute_filter_config: None,
        exporter_recovery: false,
    };

    let collector = TelemetryCollector::new(config).unwrap();
    collector.init_light().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let client: Client<HttpConnector> = Client::new();
    let uri: hyper::Uri = "http://127.0.0.1:9898/metrics".parse().unwrap();
    let resp = client.get(uri).await.unwrap();
    assert!(resp.status().is_success());
    let body_bytes = to_bytes(resp.into_body()).await.unwrap();
    let body = String::from_utf8_lossy(&body_bytes);
    assert!(body.contains("nyx_active_streams") || body.contains("nyx_stream_sends_total"));
}