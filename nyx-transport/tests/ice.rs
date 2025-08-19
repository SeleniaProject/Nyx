#[test]
fn udp_is_available() { assert!(nyx_transport::available(nyx_transport::TransportKind::Udp)); }

#[test]
fn udp_loopback_send_recv() {
	use nyx_transport::UdpEndpoint;
	let __a = UdpEndpoint::bind_loopback()?;
	let __b = UdpEndpoint::bind_loopback()?;
	let __msg = b"hello";
	a.send_to(msg, b.local_addr())?;
	let mut buf = [0u8; 16];
	let (n, from) = b.recv_from(&mut buf)?;
	assert_eq!(&buf[..n], msg);
	assert_eq!(from.ip().to_string(), "127.0.0.1");
}

