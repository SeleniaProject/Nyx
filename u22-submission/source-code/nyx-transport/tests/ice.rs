#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[test]
fn udp_is_available() {
    assert!(nyx_transport::available(nyx_transport::TransportKind::Udp));
}

#[test]
fn udp_loopback_send_recv() -> Result<(), Box<dyn std::error::Error>> {
    use nyx_transport::UdpEndpoint;
    let mut a = UdpEndpoint::bind_loopback()?;
    let mut b = UdpEndpoint::bind_loopback()?;
    let msg = b"hello";
    a.send_to(msg, b.local_addr()?)?;
    let mut buf = [0u8; 16];
    let (n, from) = b.recv_from(&mut buf)?;
    assert_eq!(&buf[..n], msg);
    assert_eq!(from.ip().to_string(), "127.0.0.1");
    Ok(())
}
