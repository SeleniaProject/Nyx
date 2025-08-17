
use nyx_control as ctl;

#[test]
fn parse_cfg() {
	let c = ctl::parse_config("port = 0").unwrap();
	assert!(c.enable_http);
}

#[tokio::test]
async fn start_and_shutdown() {
	let h = ctl::start_control(ctl::ControlConfig::default()).await.unwrap();
	if let Some(ph) = h.probe { ph.shutdown().await; }
}

