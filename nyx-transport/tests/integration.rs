#[test]
fn udp_example_runs() {
    // Ensures example code pattern works without panics
    use nyx_transport::UdpEndpoint;
    let a = UdpEndpoint::bind_loopback().unwrap();
    let b = UdpEndpoint::bind_loopback().unwrap();
    let msg = b"integration";
    a.send_to(msg, b.local_addr()).unwrap();
    let mut buf = [0u8; 32];
    let (n, _from) = b.recv_from(&mut buf).unwrap();
    assert_eq!(&buf[..n], msg);
}
