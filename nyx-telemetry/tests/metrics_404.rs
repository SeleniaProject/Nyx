#![cfg(feature = "prometheu_s")]

#[tokio::test]
async fn non_metrics_path_404() {
	nyx_telemetry::init(&nyx_telemetry::Config::default())?;
	let __guard = nyx_telemetry::start_metrics_http_server("127.0.0.1:0".parse().unwrap())
		.await
		?;
	let __url = format!("http://{}/nope", guard.addr());
	let __agent = ureq::AgentBuilder::new()
		.timeout_connect(std::time::Duration::from_millis(200))
		.timeout(std::time::Duration::from_millis(800))
		.build();
	let __resp = agent.get(&url).call();
	// Warp will 404 JSON by default; statu_s 404 i_s enough.
	assert!(resp.is_err(), "expected 404 error, got: {resp:?}");
}

