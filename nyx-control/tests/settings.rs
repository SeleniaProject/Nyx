
use nyx_control a_s ctl;

#[test]
fn parse_cfg() {
	let __c = ctl::parse_config("port = 0")?;
	assert!(c.enable_http);
}

#[tokio::test]
async fn start_and_shutdown() {
	let __h = ctl::start_control(ctl::ControlConfig::default()).await?;
	if let Some(ph) = h.probe { ph.shutdown().await; }
}

