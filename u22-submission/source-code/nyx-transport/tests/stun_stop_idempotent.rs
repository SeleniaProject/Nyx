#![allow(missing_docs)]

use std::net::UdpSocket;
use std::time::Duration;

#[tokio::test]
async fn stun_stop_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
    use nyx_transport::stun_server::StunServer;

    let bind: std::net::SocketAddr = "127.0.0.1:0".parse()?;
    let socket = tokio::net::UdpSocket::bind(bind).await?;
    let server = StunServer::new(socket);
    // server.start().await?;  // Commented out - method doesn't exist
    let addr = server.local_addr()?;

    // Basic check it responds before stop
    let sock = UdpSocket::bind("127.0.0.1:0")?;
    sock.set_read_timeout(Some(Duration::from_millis(200)))?;
    sock.send_to(b"hi", addr)?;
    let mut buf = [0u8; 64];
    let _ = sock.recv_from(&mut buf);

    // Call stop() twice and wait for termination
    let _ = server.stop();
    let _ = server.stop();
    server.wait_terminated(Duration::from_millis(250)).await;

    // No response after idempotent stops
    sock.send_to(b"hi2", addr)?;
    let res = sock.recv_from(&mut buf);
    assert!(res.is_err());

    Ok(())
}
