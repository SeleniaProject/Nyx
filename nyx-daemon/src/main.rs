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
	token: Option<String>, // Optional static token for privileged ops
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
	GetInfo,
	ReloadConfig,
	UpdateConfig { settings: serde_json::Map<String, serde_json::Value> },
	SubscribeEvents { types: Option<Vec<String>> },
}

/// RPC request envelope carrying optional request id and auth token.
#[derive(Debug, Deserialize)]
struct RpcRequest {
	#[serde(default)]
	id: Option<String>,
	#[serde(default)]
	auth: Option<String>,
	#[serde(flatten)]
	req: Request,
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
	code: u16, // 0 = OK, non-zero = error code
	#[serde(skip_serializing_if = "Option::is_none")]
	id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	data: Option<T>,
	#[serde(skip_serializing_if = "Option::is_none")]
	error: Option<String>,
}

impl<T: Serialize> Response<T> {
	fn ok_with_id(id: Option<String>, data: T) -> Self { Self { ok: true, code: 0, id, data: Some(data), error: None } }
	fn err_with_id(id: Option<String>, code: u16, msg: impl Into<String>) -> Self { Self { ok: false, code, id, data: None, error: Some(msg.into()) } }
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
	let token = std::env::var("NYX_DAEMON_TOKEN").ok();
	let state = Arc::new(DaemonState { start_time: Instant::now(), node_id, cfg: cfg_mgr, events, token });

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
	let (resp, stream_back, filter) = process_request(req, &state).await;
	let json = serde_json::to_vec(&resp).unwrap_or_else(|e| serde_json::to_vec(&Response::<serde_json::Value>::err_with_id(None, 500, e.to_string())).unwrap());
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	// If client requested subscription, stream events until client disconnects
	if let Some(mut rx) = stream_back {
		while let Ok(ev) = rx.recv().await {
			if !state.events.matches(&ev, &filter).await { continue; }
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
	let (resp, _stream_back, _filter) = process_request(req, &state).await;
	let json = serde_json::to_vec(&resp).unwrap_or_else(|e| serde_json::to_vec(&Response::<serde_json::Value>::err_with_id(None, 500, e.to_string())).unwrap());
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	Ok(())
}

async fn process_request(req_line: &str, state: &DaemonState) -> (Response<serde_json::Value>, Option<tokio::sync::broadcast::Receiver<Event>>, Option<Vec<String>>) {
	match serde_json::from_str::<RpcRequest>(req_line) {
	Ok(RpcRequest { id, auth: _, req: Request::GetInfo }) => {
			let info = Info {
				node_id: hex::encode(state.node_id),
				version: env!("CARGO_PKG_VERSION").to_string(),
				uptime_sec: state.start_time.elapsed().as_secs() as u32,
			};
			(Response::ok_with_id(id, serde_json::to_value(info).unwrap()), None, None)
		}
		Ok(RpcRequest { id, auth, req: Request::ReloadConfig }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let res = state.cfg.reload_from_file().await.unwrap_or_else(|e| ConfigResponse { success: false, message: e.to_string(), validation_errors: vec![] });
			if res.success { let _ = state.events.sender().send(Event { ty: "system".into(), detail: "config_reloaded".into() }); }
			(Response::ok_with_id(id, serde_json::to_value(res).unwrap()), None, None)
		}
		Ok(RpcRequest { id, auth, req: Request::UpdateConfig { settings } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let res = state.cfg.update_config(settings).await.unwrap_or_else(|e| ConfigResponse { success: false, message: e.to_string(), validation_errors: vec![] });
			if res.success { let _ = state.events.sender().send(Event { ty: "system".into(), detail: "config_updated".into() }); }
			(Response::ok_with_id(id, serde_json::to_value(res).unwrap()), None, None)
		}
		Ok(RpcRequest { id, auth, req: Request::SubscribeEvents { types } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let rx = state.events.subscribe();
			(Response::ok_with_id(id, serde_json::json!({"subscribed": true})), Some(rx), types)
		}
		Err(e) => (Response::err_with_id(None, 400, format!("invalid request: {e}")), None, None),
	}
}

fn is_authorized(state: &DaemonState, auth: Option<&str>) -> bool {
	match (&state.token, auth) {
		(None, _) => true, // if no token is set, allow all (development default)
		(Some(expected), Some(provided)) => provided == expected,
		(Some(_), None) => false,
	}
}

