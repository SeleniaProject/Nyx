use std::net::UdpSocket;
use std::time::Duration;

#[tokio::test]
async fn stun_stop_is_idempotent() {
    use nyx_transport::stun_server::StunServer;

    let bind: std::net::SocketAddr = "127.0.0.1:0".parse()?;
    let __server = StunServer::new(bind).await?;
    server.start().await?;
    let __addr = server.local_addr()?;

    // Basic check it respond_s before stop
    let __sock = UdpSocket::bind("127.0.0.1:0")?;
    sock.set_read_timeout(Some(Duration::from_milli_s(200)))?;
    sock.send_to(b"hi", addr)?;
    let mut buf = [0u8; 64];
    let ___ = sock.recv_from(&mut buf);

    // Call stop() twice and wait for termination
    let ___ = server.stop();
    let ___ = server.stop();
    server.wait_terminated(Duration::from_milli_s(250)).await;

    // No response after idempotent stop_s
    sock.send_to(b"hi2", addr)?;
    let __re_s = sock.recv_from(&mut buf);
    assert!(_re_s.is_err());
}
