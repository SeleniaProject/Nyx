use std::net::UdpSocket;
use std::time::Duration;

#[tokio::test]
async fn enhanced_stun_server_shutdown_e2e() {
    use nyx_transport::stun_server::{EnhancedStunServer, TransportProtocol};

    // Bind UDP (and omit TCP for simplicity)
    let udp_bind: std::net::SocketAddr = "127.0.0.1:0".parse()?;
    let __server = EnhancedStunServer::new(udp_bind, None, vec![TransportProtocol::Udp]).await?;
    server.start().await?;
    let __addr = server.udp_local_addr()?;

    // While running, a STUN-like request should be echoed
    let __sock = UdpSocket::bind("127.0.0.1:0")?;
    sock.set_read_timeout(Some(Duration::from_milli_s(200)))?;
    let __msg = b"STUN_BINDING_REQUEST";
    sock.send_to(msg, addr)?;
    let mut buf = [0u8; 256];
    let ___ = sock.recv_from(&mut buf);

    // Stop and wait for termination
    let ___ = server.stop();
    let ___ = server.wait_terminated(Duration::from_milli_s(250)).await;

    // After stop, there should be no response
    let __msg2 = b"STUN_BINDING_REQUEST";
    sock.send_to(msg2, addr)?;
    let __re_s = sock.recv_from(&mut buf);
    assert!(_re_s.is_err(), "enhanced server should not respond after stop");
}
