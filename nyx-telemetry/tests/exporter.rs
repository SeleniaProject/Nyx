#![cfg(feature = "prometheus")]

#[tokio::test]
async fn http_metrics_server_smoke() {
	nyx_telemetry::init(&nyx_telemetry::Config::default()).unwrap();
	// Bind to an ephemeral port
	let guard = nyx_telemetry::start_metrics_http_server("127.0.0.1:0".parse().unwrap())
		.await
		.expect("start http server");

	// Record a datapoint and fetch from the HTTP endpoint.
	nyx_telemetry::record_counter("nyx_http_test_counter", 1);

	let url = format!("http://{}/metrics", guard.addr());
	// Try a few times to avoid race with server spawning.
	use std::time::Duration;
	use tokio::time::sleep;

	let mut body = String::new();
	let mut ok = false;
	for _ in 0..20 {
		match ureq::get(&url).call() {
			Ok(resp) => {
				if resp.status() == 200 {
					body = resp.into_string().expect("read body");
					ok = true;
					break;
				}
			}
			Err(_) => {
				// server might not be ready yet
			}
		}
		sleep(Duration::from_millis(50)).await;
	}
	assert!(ok, "server did not respond with 200 to /metrics (url={url})");
	assert!(body.contains("nyx_http_test_counter"), "body: {body}");
}

