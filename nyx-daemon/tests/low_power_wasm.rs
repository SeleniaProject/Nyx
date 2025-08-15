#![forbid(unsafe_code)]

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn wasm_settings_low_power_triggers_transport_hook() {
    use nyx_daemon::{proto::NyxConfig, ControlService};
    use nyx_stream::management::build_settings_frame;
    use nyx_stream::management::setting_ids as mgmt_ids;

    // Bind an ephemeral HTTP port
    let sock = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = sock.local_addr().unwrap();
    drop(sock);
    std::env::set_var("NYX_HTTP_ADDR", addr.to_string());

    let cfg = NyxConfig::default();
    let service = std::sync::Arc::new(ControlService::new(cfg).await.expect("service"));
    nyx_daemon::spawn_http_server(service.clone())
        .await
        .expect("http");

    // Build a SETTINGS payload with LOW_POWER_PREFERENCE=1
    let settings = vec![nyx_stream::Setting {
        id: mgmt_ids::LOW_POWER_PREFERENCE,
        value: 1,
    }];
    let body = build_settings_frame(&settings);

    // Raw HTTP/1.1 POST to /api/v1/wasm/settings
    let mut s = tokio::net::TcpStream::connect(addr)
        .await
        .expect("connect http");
    let req = format!(
        "POST /api/v1/wasm/settings HTTP/1.1\r\nHost: {}\r\nContent-Type: application/nyx-settings\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        addr, body.len());
    s.write_all(req.as_bytes()).await.unwrap();
    s.write_all(&body).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf);
    assert!(resp.starts_with("HTTP/1.1 200"));
}
