#![forbid(unsafe_code)]

use std::{io, path::PathBuf, sync::Arc, time::Instant};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, warn};

use nyx_daemon::nyx_daemon_config::{ConfigManager, ConfigResponse, NyxConfig};
use nyx_daemon::nyx_daemon_events::{Event, EventSystem};

#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(windows)]
use tokio::net::windows::named_pipe::ServerOptions;

#[cfg(unix)]
const DEFAULT_ENDPOINT: &str = "/tmp/nyx.sock";
#[cfg(windows)]
const DEFAULT_ENDPOINT: &str = "\\\\.\\pipe\\nyx-daemon";

#[derive(Clone)]
struct DaemonState {
	start_time: Instant,
	node_id: [u8; 32],
	cfg: ConfigManager,
	events: EventSystem,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
	GetInfo,
	ReloadConfig,
	UpdateConfig { settings: serde_json::Map<String, serde_json::Value> },
	SubscribeEvents { types: Option<Vec<String>> },
}

#[derive(Debug, Serialize)]
struct Info {
	node_id: String,
	version: String,
	uptime_sec: u32,
}

#[derive(Debug, Serialize)]
#[serde(bound(serialize = "T: Serialize"))]
struct Response<T> {
	ok: bool,
	#[serde(skip_serializing_if = "Option::is_none")]
	data: Option<T>,
	#[serde(skip_serializing_if = "Option::is_none")]
	error: Option<String>,
}

impl<T: Serialize> Response<T> {
	fn ok(data: T) -> Self { Self { ok: true, data: Some(data), error: None } }
	fn err(msg: impl Into<String>) -> Self { Self { ok: false, data: None, error: Some(msg.into()) } }
}

#[tokio::main(worker_threads = 4)]
async fn main() -> io::Result<()> {
	// tracing init (env controlled)
	if std::env::var("RUST_LOG").is_err() {
		std::env::set_var("RUST_LOG", "info");
	}
	tracing_subscriber::fmt::init();

	let mut node_id = [0u8; 32];
	rand::thread_rng().fill_bytes(&mut node_id);
	let config_path = std::env::var("NYX_CONFIG").ok().map(PathBuf::from);
	let cfg_mgr = ConfigManager::new(NyxConfig::default(), config_path);
	let events = EventSystem::new(1024);
	let state = Arc::new(DaemonState { start_time: Instant::now(), node_id, cfg: cfg_mgr, events });

	info!("starting nyx-daemon (plain IPC) at {}", DEFAULT_ENDPOINT);

	#[cfg(unix)]
	{
		let _ = std::fs::remove_file(DEFAULT_ENDPOINT);
		let listener = UnixListener::bind(DEFAULT_ENDPOINT)?;
		loop {
			match listener.accept().await {
				Ok((stream, _addr)) => {
					let st = state.clone();
					tokio::spawn(async move {
						if let Err(e) = handle_unix_client(stream, st).await {
							warn!("client error: {}", e);
						}
					});
				}
				Err(e) => warn!("accept error: {}", e),
			}
		}
	}

	#[cfg(windows)]
	{
		loop {
			// Create a new pipe instance for each incoming client.
			let server = match ServerOptions::new().create(DEFAULT_ENDPOINT) {
				Ok(s) => s,
				Err(e) => {
					error!("failed to create named pipe: {}", e);
					tokio::time::sleep(std::time::Duration::from_millis(500)).await;
					continue;
				}
			};

			let st = state.clone();
			tokio::spawn(async move {
				let mut server = server; // take ownership in task
				if let Err(e) = server.connect().await {
					warn!("pipe connect error: {}", e);
					return;
				}
				if let Err(e) = handle_pipe_client(&mut server, st).await {
					warn!("client error: {}", e);
				}
				// Client disconnects when done; loop continues to create the next instance.
			});
		}
	}
}

#[cfg(unix)]
async fn handle_unix_client(mut stream: tokio::net::UnixStream, state: Arc<DaemonState>) -> io::Result<()> {
	let mut buf = Vec::with_capacity(1024);
	let mut tmp = [0u8; 256];
	loop {
		let n = stream.read(&mut tmp).await?;
		if n == 0 { break; }
		buf.extend_from_slice(&tmp[..n]);
		if buf.contains(&b'\n') { break; }
		if buf.len() > 64 * 1024 { break; }
	}
	let line_end = buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len());
	let req = std::str::from_utf8(&buf[..line_end]).unwrap_or("");
	let (resp, stream_back) = process_request(req, &state).await;
	let json = serde_json::to_vec(&resp).unwrap_or_else(|e| serde_json::to_vec(&Response::<serde_json::Value>::err(e.to_string())).unwrap());
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	// If client requested subscription, stream events until client disconnects
	if let Some(mut rx) = stream_back {
		while let Ok(ev) = rx.recv().await {
			if !state.events.matches(&ev, &None).await { continue; }
			let line = serde_json::to_vec(&ev).unwrap_or_default();
			if stream.write_all(&line).await.is_err() { break; }
			if stream.write_all(b"\n").await.is_err() { break; }
			if stream.flush().await.is_err() { break; }
		}
	}
	Ok(())
}

#[cfg(windows)]
async fn handle_pipe_client(stream: &mut tokio::net::windows::named_pipe::NamedPipeServer, state: Arc<DaemonState>) -> io::Result<()> {
	// Named pipes on Windows are byte streams; read until newline or EOF
	let mut buf = Vec::with_capacity(1024);
	let mut tmp = [0u8; 256];
	loop {
		let n = stream.read(&mut tmp).await?;
		if n == 0 { break; }
		buf.extend_from_slice(&tmp[..n]);
		if buf.contains(&b'\n') { break; }
		if buf.len() > 64 * 1024 { break; }
	}
	let line_end = buf.iter().position(|&b| b == b'\n').unwrap_or(buf.len());
	let req = std::str::from_utf8(&buf[..line_end]).unwrap_or("");
	let (resp, _stream_back) = process_request(req, &state).await;
	let json = serde_json::to_vec(&resp).unwrap_or_else(|e| serde_json::to_vec(&Response::<serde_json::Value>::err(e.to_string())).unwrap());
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	Ok(())
}

async fn process_request(req_line: &str, state: &DaemonState) -> (Response<serde_json::Value>, Option<tokio::sync::broadcast::Receiver<Event>>) {
	match serde_json::from_str::<Request>(req_line) {
		Ok(Request::GetInfo) => {
			let info = Info {
				node_id: hex::encode(state.node_id),
				version: env!("CARGO_PKG_VERSION").to_string(),
				uptime_sec: state.start_time.elapsed().as_secs() as u32,
			};
			(Response::ok(serde_json::to_value(info).unwrap()), None)
		}
		Ok(Request::ReloadConfig) => {
			let res = state.cfg.reload_from_file().await.unwrap_or_else(|e| ConfigResponse { success: false, message: e.to_string(), validation_errors: vec![] });
			if res.success { let _ = state.events.sender().send(Event { ty: "system".into(), detail: "config_reloaded".into() }); }
			(Response::ok(serde_json::to_value(res).unwrap()), None)
		}
		Ok(Request::UpdateConfig { settings }) => {
			let res = state.cfg.update_config(settings).await.unwrap_or_else(|e| ConfigResponse { success: false, message: e.to_string(), validation_errors: vec![] });
			if res.success { let _ = state.events.sender().send(Event { ty: "system".into(), detail: "config_updated".into() }); }
			(Response::ok(serde_json::to_value(res).unwrap()), None)
		}
		Ok(Request::SubscribeEvents { types }) => {
			if let Some(t) = types { state.events.set_default_types(t).await; }
			let rx = state.events.subscribe();
			(Response::ok(serde_json::json!({"subscribed": true})), Some(rx))
		}
		Err(e) => (Response::err(format!("invalid request: {e}")), None),
	}
}

