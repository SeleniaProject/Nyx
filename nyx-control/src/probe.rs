use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, io::{AsyncReadExt, AsyncWriteExt}, sync::Notify};

/// Handle to stop the probe server.
pub struct ProbeHandle {
	addr: SocketAddr,
	stop: Arc<Notify>,
	task: tokio::task::JoinHandle<()>,
}

impl ProbeHandle {
	/// The bound socket address.
	pub fn addr(&self) -> SocketAddr { self.addr }

	/// Abort background task to guarantee prompt shutdown in tests.
	pub async fn shutdown(self) {
		// Best-effort graceful signal (ignored if no waiter yet)
		self.stop.notify_waiters();
		// Hard abort to avoid Notify race
		self.task.abort();
		let _ = self.task.await;
	}
}

/// Starts a minimal HTTP probe server serving /healthz and /ready returning 200 OK.
/// Returns the bound address (useful when port 0 was passed) and a shutdown handle.
pub async fn start_probe(port: u16) -> crate::Result<ProbeHandle> {
	// Bind only on loopback to avoid platform-specific firewall prompts in tests
	let addr: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
	let listener = TcpListener::bind(addr).await?;
	let local_addr = listener.local_addr()?;
	let stop = Arc::new(Notify::new());
	let stop2 = stop.clone();

	let task = tokio::spawn(async move {
		loop {
			tokio::select! {
				biased;
				_ = stop2.notified() => break,
				acc = listener.accept() => {
					match acc {
						Ok((mut sock, _peer)) => {
							// Handle a single HTTP/1.1 request in-place, keep-alive not supported
							tokio::spawn(async move {
								let mut buf = [0u8; 1024];
								let n = match sock.read(&mut buf).await { Ok(n) => n, Err(_) => return };
								let req = String::from_utf8_lossy(&buf[..n]);
								let path = parse_path(&req);
								let (status, body) = match path.as_deref() {
									Some("/healthz") | Some("/ready") | Some("/livez") => ("200 OK", "ok"),
									_ => ("404 Not Found", "not found"),
								};
								let resp = format!(
									"HTTP/1.1 {status}\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
									body.len()
								);
								let _ = sock.write_all(resp.as_bytes()).await;
								let _ = sock.shutdown().await;
							});
						}
						Err(_) => break,
					}
				}
			}
		}
	});

	Ok(ProbeHandle { addr: local_addr, stop, task })
}

fn parse_path(req: &str) -> Option<String> {
	// Very small HTTP parser: expects first line like "GET /path HTTP/1.1"
	let mut lines = req.split('\n');
	let line1 = lines.next()?.trim();
	let mut it = line1.split_whitespace();
	let _method = it.next()?; // only GET used here
	let path = it.next()?;
	Some(path.to_string())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[tokio::test]
	async fn probe_serves_health() {
		let h = start_probe(0).await.unwrap();
		let addr = h.addr();
		let resp = tiny_http_get(addr, "/healthz").await;
		assert!(resp.contains("200 OK"));
		h.shutdown().await;
	}

	async fn tiny_http_get(addr: SocketAddr, path: &str) -> String {
		use tokio::net::TcpStream;
		let mut s = TcpStream::connect(addr).await.unwrap();
		let req = format!("GET {path} HTTP/1.1\r\nhost: localhost\r\nconnection: close\r\n\r\n");
		s.write_all(req.as_bytes()).await.unwrap();
		let mut out = Vec::new();
		s.read_to_end(&mut out).await.unwrap();
		String::from_utf8_lossy(&out).to_string()
	}
}
