#![cfg(feature = "prometheus")]

#[tokio::test]
async fn http_metrics_server_smoke() -> anyhow::Result<()> {
    nyx_telemetry::init(&nyx_telemetry::Config::default())?;
    nyx_telemetry::record_counter("nyx_http_test_counter", 1);
    // Use warp test harness against the filter to avoid flakiness of real sockets on windows.
    let filter = nyx_telemetry::warp_metrics_filter();
    let resp = warp::test::request()
        .method("GET")
        .path("/metrics")
        .reply(&filter)
        .await;
    assert_eq!(resp.status(), 200);
    let body = String::from_utf8(resp.body().to_vec())?;
    assert!(body.contains("nyx_http_test_counter"), "body: {body}");
    Ok(())
}
