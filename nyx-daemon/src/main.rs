#![forbid(unsafe_code)]

use std::{io, path::PathBuf, sync::Arc, time::Instant};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, warn};

use nyx_daemon::nyx_daemon_config::{ConfigManager, ConfigResponse, NyxConfig, VersionSummary};
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
	ListConfigVersions,
	RollbackConfig { version: u64 },
	CreateConfigSnapshot { description: Option<String> },
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
	let token = ensure_token_from_env_or_cookie();
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

fn ensure_token_from_env_or_cookie() -> Option<String> {
	// 1) Environment variable takes precedence (non-empty)
	if let Ok(t) = std::env::var("NYX_DAEMON_TOKEN") {
		let tt = t.trim().to_string();
		if !tt.is_empty() { return Some(tt); }
	}

	// 2) Determine cookie path: explicit env or default per-user path
	let cookie_path = if let Ok(p) = std::env::var("NYX_DAEMON_COOKIE") {
		if !p.trim().is_empty() { std::path::PathBuf::from(p) } else { default_cookie_path() }
	} else {
		default_cookie_path()
	};

	// 3) If cookie exists and non-empty, read it
	if let Ok(s) = std::fs::read_to_string(&cookie_path) {
		let tok = s.trim().to_string();
		if !tok.is_empty() { return Some(tok); }
	}

	// 4) Otherwise, auto-generate a cookie (Tor-like UX)
	let mut bytes = [0u8; 32];
	rand::thread_rng().fill_bytes(&mut bytes);
	let tok = hex::encode(bytes);
	if let Some(parent) = cookie_path.parent() {
		if let Err(e) = std::fs::create_dir_all(parent) { warn!("failed to create cookie dir: {e}"); return None; }
	}
	if let Err(e) = std::fs::write(&cookie_path, &tok) {
		warn!("failed to write cookie file {}: {e}", cookie_path.display());
		return None;
	}
	// Best-effort permission tightening (Unix only)
	#[cfg(unix)]
	{
		use std::os::unix::fs::PermissionsExt;
		if let Ok(meta) = std::fs::metadata(&cookie_path) {
			let mut perm = meta.permissions();
			perm.set_mode(0o600);
			let _ = std::fs::set_permissions(&cookie_path, perm);
		}
	}
	info!("generated control auth cookie at {}", cookie_path.display());
	Some(tok)
}

fn default_cookie_path() -> std::path::PathBuf {
	#[cfg(windows)]
	{
		if let Ok(appdata) = std::env::var("APPDATA") {
			return std::path::Path::new(&appdata).join("nyx").join("control.authcookie");
		}
		return std::path::PathBuf::from("control.authcookie");
	}
	#[cfg(unix)]
	{
		if let Ok(home) = std::env::var("HOME") {
			return std::path::Path::new(&home).join(".nyx").join("control.authcookie");
		}
		std::path::PathBuf::from("control.authcookie")
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
	let resp_id = resp.id.clone();
	let json = serde_json::to_vec(&resp).unwrap_or_else(|e| serde_json::to_vec(&Response::<serde_json::Value>::err_with_id(resp_id, 500, e.to_string())).unwrap());
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
	let (resp, stream_back, filter) = process_request(req, &state).await;
	let resp_id = resp.id.clone();
	let json = serde_json::to_vec(&resp).unwrap_or_else(|e| serde_json::to_vec(&Response::<serde_json::Value>::err_with_id(resp_id, 500, e.to_string())).unwrap());
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	// Stream events if subscribed until client disconnects
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
		Ok(RpcRequest { id, auth, req: Request::ListConfigVersions }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let list: Vec<VersionSummary> = state.cfg.list_versions().await;
			(Response::ok_with_id(id, serde_json::to_value(list).unwrap()), None, None)
		}
		Ok(RpcRequest { id, auth, req: Request::RollbackConfig { version } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let res = state.cfg.rollback(version).await.unwrap_or_else(|e| ConfigResponse { success: false, message: e.to_string(), validation_errors: vec![] });
			if res.success { let _ = state.events.sender().send(Event { ty: "system".into(), detail: format!("config_rolled_back:{version}") }); }
			(Response::ok_with_id(id, serde_json::to_value(res).unwrap()), None, None)
		}
		Ok(RpcRequest { id, auth, req: Request::CreateConfigSnapshot { description } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			match state.cfg.snapshot(description.as_deref().unwrap_or("manual_snapshot")).await {
				Ok(ver) => (Response::ok_with_id(id, serde_json::json!({"version": ver})), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Err(e) => (Response::err_with_id(None, 400, format!("invalid request: {e}")), None, None),
	}
}

fn is_authorized(state: &DaemonState, auth: Option<&str>) -> bool {
	// Treat empty or whitespace-only token as not set (disabled auth)
	let effective = state
		.token
		.as_deref()
		.map(|s| s.trim())
		.filter(|s| !s.is_empty());
	match (effective, auth) {
		(None, _) => true, // if no token is set, allow all (development default)
		(Some(expected), Some(provided)) => {
			let ok = provided == expected;
			if !ok { warn!("authorization failed: wrong token"); }
			ok
		}
		(Some(_), None) => {
			warn!("authorization failed: missing token");
			false
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn make_state_with_token(token: Option<&str>) -> DaemonState {
		let mut node_id = [0u8; 32];
		node_id[0] = 1; // deterministic
		let cfg_mgr = ConfigManager::new(NyxConfig::default(), None);
		let events = EventSystem::new(16);
		DaemonState {
			start_time: Instant::now(),
			node_id,
			cfg: cfg_mgr,
			events,
			token: token.map(|s| s.to_string()),
		}
	}

	#[tokio::test]
	async fn get_info_ok_and_id_echo() {
		let state = make_state_with_token(None);
		let req = serde_json::json!({
			"id": "abc",
			"op": "get_info"
		})
		.to_string();
		let (resp, rx, filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		assert_eq!(resp.id.as_deref(), Some("abc"));
		assert!(rx.is_none());
		assert!(filter.is_none());
	}

	#[tokio::test]
	async fn update_config_unauthorized_without_token() {
		let state = make_state_with_token(Some("secret"));
		let req = serde_json::json!({
			"id": "u1",
			"op": "update_config",
			"settings": {"log_level": "debug"}
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(!resp.ok);
		assert_eq!(resp.code, 401);
		assert_eq!(resp.id.as_deref(), Some("u1"));
	}

	#[tokio::test]
	async fn subscribe_events_authorized_and_filters_attached() {
		let state = make_state_with_token(Some("tok"));
		let req = serde_json::json!({
			"id": "s1",
			"auth": "tok",
			"op": "subscribe_events",
			"types": ["system"]
		})
		.to_string();
		let (resp, rx, filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		assert!(rx.is_some());
		assert_eq!(filter, Some(vec!["system".to_string()]));
	}

	#[tokio::test]
	async fn list_versions_after_snapshot() {
		let state = make_state_with_token(None);
		let _ = state.cfg.snapshot("t").await.unwrap();
		let req = serde_json::json!({
			"id": "v1",
			"op": "list_config_versions"
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		let v: Vec<VersionSummary> = serde_json::from_value(resp.data.unwrap()).unwrap();
		assert!(!v.is_empty());
	}

	#[tokio::test]
	async fn rollback_succeeds_with_valid_version() {
		let state = make_state_with_token(Some("t"));
		let ver = state.cfg.snapshot("before").await.unwrap();
		let req = serde_json::json!({
			"id": "rb1",
			"auth": "t",
			"op": "rollback_config",
			"version": ver
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok, "{resp:?}");
		let cr: ConfigResponse = serde_json::from_value(resp.data.unwrap()).unwrap();
		assert!(cr.success);
	}

	#[tokio::test]
	async fn manual_snapshot_returns_version() {
		let state = make_state_with_token(Some("s"));
		let req = serde_json::json!({
			"id": "ms1",
			"auth": "s",
			"op": "create_config_snapshot",
			"description": "from_test"
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		let v = resp.data.unwrap();
		let ver = v.get("version").and_then(|n| n.as_u64()).unwrap();
		assert!(ver >= 1);
	}

	#[tokio::test]
	async fn invalid_request_returns_400() {
		let state = make_state_with_token(None);
		let req = "{ not_json }"; // パース不能
		let (resp, _rx, _filter) = process_request(req, &state).await;
		assert!(!resp.ok);
		assert_eq!(resp.code, 400);
		assert!(resp.error.unwrap().contains("invalid request"));
	}

	#[tokio::test]
	async fn empty_env_token_is_treated_as_disabled() {
		// Simulate daemon started with empty token ("   ") which should disable auth
		let mut state = make_state_with_token(Some("   "));
		// ensure internal state reflects token provided
		// but authorization treats it as None
		let req = serde_json::json!({
			"id": "r1",
			"op": "reload_config"
		}).to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok, "auth should be disabled when token is empty/whitespace");
	}
}

