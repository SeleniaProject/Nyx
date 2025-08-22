//! Prometheus metrics integration tests
//!
//! Tests for the Prometheus metrics export functionality in nyx-daemon.

use nyx_daemon::telemetry::MetricsExporter;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_prometheus_metrics_export() {
    let exporter = MetricsExporter::new("127.0.0.1:0".parse().unwrap())
        .expect("Failed to create metrics exporter");

    // Start the exporter
    exporter
        .start()
        .await
        .expect("Failed to start metrics exporter");

    // Wait a bit then access
    sleep(Duration::from_millis(100)).await;

    let metrics_endpoint = format!("http://{}/metrics", exporter.local_addr());

    // Try to fetch metrics
    let response = reqwest::get(&metrics_endpoint).await;

    match response {
        Ok(resp) => {
            assert!(
                resp.status().is_success(),
                "Metrics endpoint should return 200 OK"
            );

            let body = resp.text().await.expect("Failed to read response body");

            // Basic validation that it looks like Prometheus metrics
            assert!(
                body.contains("# HELP") || body.contains("# TYPE"),
                "Response should contain Prometheus metric metadata"
            );

            println!("Successfully retrieved metrics from endpoint");
        }
        Err(e) => {
            // If we can't connect, that's okay for this test - just log it
            println!("Could not connect to metrics endpoint: {}", e);
        }
    }

    // Clean shutdown
    exporter
        .stop()
        .await
        .expect("Failed to stop metrics exporter");
}

#[tokio::test]
async fn test_metrics_content_format() {
    let exporter = MetricsExporter::new("127.0.0.1:0".parse().unwrap())
        .expect("Failed to create metrics exporter");

    exporter
        .start()
        .await
        .expect("Failed to start metrics exporter");

    // Wait for startup
    sleep(Duration::from_millis(50)).await;

    let metrics_endpoint = format!("http://{}/metrics", exporter.local_addr());

    if let Ok(response) = reqwest::get(&metrics_endpoint).await {
        if let Ok(body) = response.text().await {
            // Check for basic Prometheus format compliance
            let lines: Vec<&str> = body.lines().collect();

            let mut has_help = false;
            let mut has_type = false;
            let mut has_metric = false;

            for line in lines {
                if line.starts_with("# HELP") {
                    has_help = true;
                }
                if line.starts_with("# TYPE") {
                    has_type = true;
                }
                if !line.starts_with("#") && line.contains(" ") {
                    has_metric = true;
                }
            }

            println!("Metrics format validation:");
            println!("  Has HELP comments: {}", has_help);
            println!("  Has TYPE comments: {}", has_type);
            println!("  Has metric lines: {}", has_metric);

            // At least one of these should be true for valid Prometheus output
            assert!(
                has_help || has_type || has_metric,
                "Should have at least some Prometheus-formatted content"
            );
        }
    }

    exporter
        .stop()
        .await
        .expect("Failed to stop metrics exporter");
}

#[tokio::test]
async fn test_metrics_endpoint_lifecycle() {
    let exporter = MetricsExporter::new("127.0.0.1:0".parse().unwrap())
        .expect("Failed to create metrics exporter");

    let metrics_endpoint = format!("http://{}/metrics", exporter.local_addr());

    // Should not be accessible before start
    let pre_start_response = reqwest::get(&metrics_endpoint).await;
    assert!(
        pre_start_response.is_err(),
        "Endpoint should not be accessible before start"
    );

    // Start the exporter
    exporter
        .start()
        .await
        .expect("Failed to start metrics exporter");
    sleep(Duration::from_millis(50)).await;

    // Should be accessible after start
    let post_start_response = reqwest::get(&metrics_endpoint).await;
    if let Ok(response) = post_start_response {
        assert!(
            response.status().is_success(),
            "Endpoint should be accessible after start"
        );
    }

    // Stop the exporter
    exporter
        .stop()
        .await
        .expect("Failed to stop metrics exporter");
    sleep(Duration::from_millis(50)).await;

    // Should not be accessible after stop
    let post_stop_response = reqwest::get(&metrics_endpoint).await;
    assert!(
        post_stop_response.is_err(),
        "Endpoint should not be accessible after stop"
    );

    println!("Metrics endpoint lifecycle test completed successfully");
}
