//! Simplified STUN-like echo server (local testing only).
use crate::{Error, Result};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// Run a simple UDP echo loop for a bounded duration/iterations.
/// Returns the local socket address used.
pub fn run_echo_once(timeout: Duration) -> Result<SocketAddr> {
	let sock = UdpSocket::bind(("127.0.0.1", 0)).map_err(|e| Error::Msg(e.to_string()))?;
	sock.set_read_timeout(Some(timeout)).ok();
	let local = sock.local_addr().unwrap();
	// Send to ourselves and read back to validate reachability.
	let payload = b"echo";
	sock.send_to(payload, local).map_err(|e| Error::Msg(e.to_string()))?;
	let mut buf = [0u8; 16];
	let started = Instant::now();
	while started.elapsed() < timeout {
		if let Ok((n, from)) = sock.recv_from(&mut buf) {
			if &buf[..n] == payload && from == local {
				return Ok(local);
			}
		}
	}
	Err(Error::Msg("echo timeout".into()))
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn echo_smoke() { let _ = run_echo_once(Duration::from_millis(200)).unwrap(); }
}
