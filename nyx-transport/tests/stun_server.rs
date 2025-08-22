use std::net::UdpSocket;
use std::time::Duration;

#[tokio::test]
async fn stun_server_shutdown_e2e() -> Result<(), Box<dyn std::error::Error>> {
    use nyx_transport::stun_server::StunServer;
    let bind: std::net::SocketAddr = "127.0.0.1:0".parse()?;
    let server = StunServer::new(bind).await?;
    server.start().await?;
    let addr = server.local_addr()?;

    // Basic echo works while running
    let sock = UdpSocket::bind("127.0.0.1:0")?;
    sock.set_read_timeout(Some(Duration::from_millis(200)))?;
    let msg = b"hi";
    sock.send_to(msg, addr)?;
    let mut buf = [0u8; 128];
    let _ = sock.recv_from(&mut buf); // Ignore exact contents

    // Stop server and ensure no further responses
    let _ = server.stop();
    // Wait for background task termination (best-effort within small timeout)
    server.wait_terminated(Duration::from_millis(250)).await;

    let msg2 = b"hi2";
    sock.send_to(msg2, addr)?;
    let res = sock.recv_from(&mut buf);
    assert!(res.is_err(), "server should not respond after stop");

    Ok(())
}
