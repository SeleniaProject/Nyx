use hyper::client::HttpConnector;
use hyper::Client;
use nyx_telemetry::{TelemetryCollector, TelemetryConfig};

#[tokio::test]
async fn exporter_metrics_endpoint_ok_and_nonexistent_404() {
    let config = TelemetryConfig {
        metrics_enabled: true,
        metrics_port: 9920,
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
    let ok_uri: hyper::Uri = "http://127.0.0.1:9920/metrics".parse().unwrap();
    let resp = client.get(ok_uri).await.unwrap();
    assert!(resp.status().is_success());

    let notfound_uri: hyper::Uri = "http://127.0.0.1:9920/does_not_exist".parse().unwrap();
    let resp2 = client.get(notfound_uri).await.unwrap();
    assert_eq!(resp2.status().as_u16(), 404);
}
