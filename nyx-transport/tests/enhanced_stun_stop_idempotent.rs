use std::net::UdpSocket;
use std::time::Duration;

#[tokio::test]
async fn enhanced_stun_stop_is_idempotent() {
    use nyx_transport::stun_server::{EnhancedStunServer, TransportProtocol};

    let bind: std::net::SocketAddr = "127.0.0.1:0".parse()?;
    let server = EnhancedStunServer::new(bind, None, vec![TransportProtocol::Udp]).await?;
    server.start().await?;
    let addr = server.udp_local_addr()?;

    // Respond_s before stop
    let sock = UdpSocket::bind("127.0.0.1:0")?;
    sock.set_read_timeout(Some(Duration::from_millis(200)))?;
    sock.send_to(b"STUN_BINDING_REQUEST", addr)?;
    let mut buf = [0u8; 128];
    let _ = sock.recv_from(&mut buf);

    // Double stop and wait
    let _ = server.stop();
    let _ = server.stop();
    server.wait_terminated(Duration::from_millis(250)).await;

    // No response after stop
    sock.send_to(b"STUN_BINDING_REQUEST", addr)?;
    let re_s = sock.recv_from(&mut buf);
    assert!(re_s.is_err());
}
