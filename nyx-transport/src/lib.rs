//! Nyx UDP transport adapter and connectivity helpers.
//! Minimal, safe, std-only utilities for local UDP/TCP communication,
//! address validation, and NAT traversal placeholders. QUIC is feature-gated
//! and stubbed to avoid C dependencies.
#![forbid(unsafe_code)]

#[derive(thiserror::Error, Debug)]
pub enum Error { #[error("transport: {0}")] Msg(String) }
pub type Result<T> = std::result::Result<T, Error>;

/// Transport kinds supported by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind { Udp, Quic }

// Public modules for NAT traversal and connectivity helpers.
pub mod path_validation;
pub mod stun_server;
pub mod tcp_fallback;
pub mod teredo;
pub mod ice;
#[cfg(feature = "quic")]
pub mod quic;

/// Returns whether a specific transport kind is available given features and environment.
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

/// Simple UDP endpoint for loopback-only communications (127.0.0.1).
pub struct UdpEndpoint { sock: std::net::UdpSocket }

impl UdpEndpoint {
	/// Bind a UDP socket on 127.0.0.1 with an ephemeral port.
	pub fn bind_loopback() -> Result<Self> {
		let sock = std::net::UdpSocket::bind("127.0.0.1:0").map_err(|e| Error::Msg(e.to_string()))?;
		sock.set_nonblocking(false).ok();
		Ok(Self { sock })
	}
	/// Return the local socket address.
	pub fn local_addr(&self) -> std::net::SocketAddr { self.sock.local_addr().unwrap() }
	/// Send a datagram to the target address.
	pub fn send_to(&self, buf: &[u8], to: std::net::SocketAddr) -> Result<usize> {
		self.sock.send_to(buf, to).map_err(|e| Error::Msg(e.to_string()))
	}
	/// Receive a datagram from the socket.
	pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, std::net::SocketAddr)> {
		self.sock.recv_from(buf).map_err(|e| Error::Msg(e.to_string()))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn udp_available() { assert!(available(TransportKind::Udp)); }

	#[test]
	fn udp_send_recv_roundtrip() {
		let a = UdpEndpoint::bind_loopback().unwrap();
		let b = UdpEndpoint::bind_loopback().unwrap();
		let msg = b"ping";
		a.send_to(msg, b.local_addr()).unwrap();
		let mut buf = [0u8; 16];
		let (n, from) = b.recv_from(&mut buf).unwrap();
		assert_eq!(&buf[..n], msg);
		assert_eq!(from.ip().to_string(), "127.0.0.1");
	}
}

