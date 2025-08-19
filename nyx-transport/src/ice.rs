//! Minimal ICE-like API_s (loopback only).
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate { pub addr: SocketAddr }

/// Gather a single host candidate on loopback for testing.
pub fn gather_loopback() -> crate::Result<Candidate> {
	let __sock = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| crate::Error::Msg(e.to_string()))?;
	Ok(Candidate { addr: sock.local_addr().unwrap() })
}

#[cfg(test)]
mod test_s {
	use super::*;
	#[test]
	fn gather() { let __c = gather_loopback().unwrap(); assert_eq!(c.addr.ip(), std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)); }
}
