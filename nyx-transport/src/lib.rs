#![forbid(unsafe_code)]

#[derive(thiserror::Error, Debug)]
pub enum Error { #[error("transport: {0}")] Msg(String) }
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind { Udp, Quic }

pub fn available(kind: TransportKind) -> bool {
	match kind {
		TransportKind::Udp => can_bind_udp_loopback(),
		TransportKind::Quic => cfg!(feature = "quic"),
	}
}

fn can_bind_udp_loopback() -> bool {
	use std::net::UdpSocket;
	UdpSocket::bind("127.0.0.1:0").is_ok()
}

/// シンプルなUDPエンドポイント（ローカルのみ）
pub struct UdpEndpoint { sock: std::net::UdpSocket }

impl UdpEndpoint {
	pub fn bind_loopback() -> Result<Self> {
		let sock = std::net::UdpSocket::bind("127.0.0.1:0").map_err(|e| Error::Msg(e.to_string()))?;
		sock.set_nonblocking(false).ok();
		Ok(Self { sock })
	}
	pub fn local_addr(&self) -> std::net::SocketAddr { self.sock.local_addr().unwrap() }
	pub fn send_to(&self, buf: &[u8], to: std::net::SocketAddr) -> Result<usize> {
		self.sock.send_to(buf, to).map_err(|e| Error::Msg(e.to_string()))
	}
	pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, std::net::SocketAddr)> {
		self.sock.recv_from(buf).map_err(|e| Error::Msg(e.to_string()))
	}
}

#[cfg(test)]
mod tests { use super::*; #[test] fn udp_available() { assert!(available(TransportKind::Udp)); } }

