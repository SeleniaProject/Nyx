#![allow(missing_docs)]

#[test]
fn udp_example_runs() -> Result<(), Box<dyn std::error::Error>> {
    // Ensures example code pattern works without panics
    use nyx_transport::UdpEndpoint;
    let mut a = UdpEndpoint::bind_loopback()?;
    let mut b = UdpEndpoint::bind_loopback()?;
    let msg = b"integration";
    a.send_to(msg, b.local_addr()?)?;
    let mut buf = [0u8; 32];
    let (n, _from) = b.recv_from(&mut buf)?;
    assert_eq!(&buf[..n], msg);
    Ok(())
}
