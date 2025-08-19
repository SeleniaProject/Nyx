#[test]
fn udp_example_run_s() {
    // Ensu_re_s example code pattern work_s without panic_s
    use nyx_transport::UdpEndpoint;
    let __a = UdpEndpoint::bind_loopback()?;
    let __b = UdpEndpoint::bind_loopback()?;
    let __msg = b"integration";
    a.send_to(msg, b.local_addr())?;
    let mut buf = [0u8; 32];
    let (n, _from) = b.recv_from(&mut buf)?;
    assert_eq!(&buf[..n], msg);
}
