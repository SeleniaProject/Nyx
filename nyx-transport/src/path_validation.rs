//! Lightweight path/address validation utilities.
use crate::{Error, Result};
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

/// Validate a host:port pair and return a resolved SocketAddr if possible.
/// This avoids DNS queries for plain IP literals and ensures the port is valid.
pub fn validate_host_port(host: &str, port: u16) -> Result<SocketAddr> {
	// Try to parse as literal IP first to avoid DNS lookups.
	if let Ok(ip) = host.parse::<IpAddr>() {
		return Ok(SocketAddr::from((ip, port)));
	}
	// Fallback: attempt resolution via ToSocketAddrs; this may perform DNS.
	let mut iter = (host, port)
		.to_socket_addrs()
		.map_err(|e| Error::Msg(format!("invalid address {host}:{port}: {e}")))?;
	iter.next()
		.ok_or_else(|| Error::Msg(format!("unable to resolve {host}:{port}")))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_ipv4_literal() {
		let addr = validate_host_port("127.0.0.1", 8080).unwrap();
		assert_eq!(addr.to_string(), "127.0.0.1:8080");
	}
}
