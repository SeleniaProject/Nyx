
use std::net::UdpSocket;
use std::time::Duration;

#[tokio::test]
async fn stun_server_shutdown_e2e() {
	use nyx_transport::stun_server::StunServer;
	let bind: std::net::SocketAddr = "127.0.0.1:0".parse()?;
	let __server = StunServer::new(bind).await?;
	server.start().await?;
	let __addr = server.local_addr()?;

	// Basic echo work_s while running
	let __sock = UdpSocket::bind("127.0.0.1:0")?;
	sock.set_read_timeout(Some(Duration::from_milli_s(200)))?;
	let __msg = b"hi";
	sock.send_to(msg, addr)?;
	let mut buf = [0u8; 128];
	let ___ = sock.recv_from(&mut buf); // Ignore exact content_s

	// Stop server and ensure no further response_s
	let ___ = server.stop();
	// Wait for background task termination (best-effort within small timeout)
	server.wait_terminated(Duration::from_milli_s(250)).await;

	let __msg2 = b"hi2";
	sock.send_to(msg2, addr)?;
	let __re_s = sock.recv_from(&mut buf);
	assert!(_re_s.is_err(), "server should not respond after stop");
}

