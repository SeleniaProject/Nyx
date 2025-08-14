#![forbid(unsafe_code)]
#![cfg(feature = "experimental-alerts")]

use std::time::Duration;

#[tokio::test]
async fn alerts_http_endpoints_respond() {
    use nyx_daemon::proto::NyxConfig;
    use nyx_daemon::spawn_http_server; // make public? fallback to calling main initializer
    // Since spawn_http_server is not public, mimic main's sequence minimally
    
    // Pick a free port and set NYX_HTTP_ADDR for the server
    let sock = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = sock.local_addr().unwrap();
    drop(sock);
    std::env::set_var("NYX_HTTP_ADDR", addr.to_string());

    // Start minimal service
    let config = NyxConfig::default();
    let service = std::sync::Arc::new(nyx_daemon::ControlService::new(config).await.expect("service"));
    nyx_daemon::spawn_http_server(service.clone()).await.expect("http");

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Manual HTTP/1.1 GET
    use tokio::net::TcpStream;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = TcpStream::connect(addr).await.unwrap();
    let req = format!("GET /api/v1/alerts/stats HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", addr);
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf);
    assert!(resp.starts_with("HTTP/1.1 200"));

    // Analysis endpoint
    let mut s2 = TcpStream::connect(addr).await.unwrap();
    let req2 = format!("GET /api/v1/alerts/analysis HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n", addr);
    s2.write_all(req2.as_bytes()).await.unwrap();
    let mut buf2 = Vec::new();
    s2.read_to_end(&mut buf2).await.unwrap();
    let resp2 = String::from_utf8_lossy(&buf2);
    assert!(resp2.starts_with("HTTP/1.1 200"));
}



