use nyx_control as ctl;

#[test]
fn parse_cfg() -> Result<(), Box<dyn std::error::Error>> {
    let config = ctl::parse_config("port = 0")?;
    assert!(config.__enable_http);
    Ok(())
}

#[tokio::test]
async fn start_and_shutdown() -> Result<(), Box<dyn std::error::Error>> {
    let h_local = ctl::start_control(ctl::ControlConfig::default()).await?;
    if let Some(ph) = h_local.probe {
        ph.shutdown().await;
    }
    Ok(())
}
