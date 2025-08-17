//! Minimal TCP fallback helpers.
use crate::{Error, Result};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// Attempt a TCP connection with a timeout; returns Ok(true) if connected.
pub fn try_connect(addr: SocketAddr, timeout: Duration) -> Result<bool> {
	let stream = TcpStream::connect_timeout(&addr, timeout)
		.map_err(|e| Error::Msg(format!("tcp connect to {addr} failed: {e}")))?;
	stream.set_nodelay(true).ok();
	Ok(true)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::net::TcpListener;
	#[test]
	fn can_connect_localhost() {
		let listener = TcpListener::bind("127.0.0.1:0").unwrap();
		let addr = listener.local_addr().unwrap();
		let th = std::thread::spawn(move || listener.accept());
		let ok = try_connect(addr, Duration::from_millis(200)).unwrap();
		assert!(ok);
		let _ = th.join();
	}
}
