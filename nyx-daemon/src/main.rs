#![forbid(unsafe_code)]

use std::{io, path::PathBuf, sync::Arc, time::Instant};

use rand::RngCore;
use serde::{Deserialize, Serialize};
mod json_util;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, warn};
#[cfg(feature = "telemetry")]
use nyx_telemetry as telemetry;

use nyx_daemon::nyx_daemon_config::{ConfigManager, ConfigResponse, NyxConfig, VersionSummary};
use nyx_daemon::event_system::{Event, EventSystem};
use nyx_daemon::metrics::MetricsCollector;
#[cfg(feature = "low_power")]
use nyx_daemon::low_power::LowPowerBridge;
// use nyx_daemon::prometheus_exporter::maybe_start_prometheus;
use nyx_core::sandbox::{apply_policy as apply_os_sandbox, SandboxPolicy, SandboxStatus};

#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(windows)]
use tokio::net::windows::named_pipe::ServerOptions;

#[cfg(unix)]
const DEFAULT_ENDPOINT: &str = "/tmp/nyx.sock";
#[cfg(windows)]
const DEFAULT_ENDPOINT: &str = "\\\\.\\pipe\\nyx-daemon";

const INITIAL_READ_TIMEOUT_MS: u64 = 2000;

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
	GetSystemInfo,
	Health,
	#[cfg(feature = "low_power")]
	SetPowerState { state: u32 },
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
struct SystemInfo {
	uptime: u64,
	version: String,
	build_time: String,
	memory_usage: u64,
}

#[derive(Debug, Serialize)]
struct HealthStatus {
	healthy: bool,
	timestamp: chrono::DateTime<chrono::Utc>,
	components: std::collections::HashMap<String, String>,
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
	// If a config file is configured, attempt an initial reload to apply static settings (e.g., max_frame_len_bytes)
	if cfg_mgr.getconfig().await != NyxConfig::default() {
		let _ = cfg_mgr.reload_from_file().await; // best-effort; logs on failure
	} else if let Some(path) = std::env::var("NYX_CONFIG").ok().map(PathBuf::from) {
		// Best-effort manual read if initial config is default but path is set
		if let Ok(content) = tokio::fs::read_to_string(&path).await {
			if let Ok(parsed) = toml::from_str::<NyxConfig>(&content) {
				if let Some(n) = parsed.max_frame_len_byte_s {
					nyx_stream::FrameCodec::set_default_limit(n as usize);
					std::env::set_var("NYX_FRAME_MAX_LEN", n.to_string());
				}
			}
		}
	}
	let events = EventSystem::new(1024);
	let token = ensure_token_from_env_or_cookie();
	let state = Arc::new(DaemonState { start_time: Instant::now(), node_id, cfg: cfg_mgr, events, token });

	// Try to apply minimal OS-level sandboxing (no-op on unsupported platforms/features)
	match apply_os_sandbox(SandboxPolicy::Minimal) {
		SandboxStatus::Applied => info!("OS sandbox applied (Minimal)"),
		SandboxStatus::Unsupported => {
			// Keep noise low in logs; sandbox may be disabled intentionally
			tracing::debug!("OS sandbox unsupported or disabled on this build/platform")
		}
	}

	// Start low-power bridge (mobile FFI) if enabled, keep handle alive in a detached task holder.
	#[cfg(feature = "low_power")]
	let _lp_guard: Option<LowPowerBridge> = {
		// clone events for the bridge
		let ev = state.events.clone();
		match LowPowerBridge::start(ev) {
			Ok(h) => Some(h),
			Err(e) => { warn!("failed to start LowPowerBridge: {e}"); None }
		}
	};

	info!("starting nyx-daemon (plain IPC) at {}", DEFAULT_ENDPOINT);

	// Optionally start Prometheus exporter if environment is set
	if std::env::var("NYX_PROMETHEUS_ADDR").is_ok() {
		let _collector = Arc::new(MetricsCollector::new());
		// match maybe_start_prometheus(collector).await {
		// 	Some((srv, addr, coll)) => info!("Prometheus exporter listening at http://{}/metrics", addr),
		// 	None => warn!("failed to start Prometheus exporter from env"),
		// }
		warn!("Prometheus exporter not implemented yet");
	}

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
			// 1 outstanding instance waiting for ConnectNamedPipe at any time.
			let server = match ServerOptions::new().create(DEFAULT_ENDPOINT) {
				Ok(s) => s,
				Err(e) => {
					error!("failed to create named pipe: {}", e);
					tokio::time::sleep(std::time::Duration::from_millis(500)).await;
					continue;
				}
			};

			// Await connection before spawning handler to avoid unbounded instance creation
			match server.connect().await {
				Ok(()) => {
					let st = state.clone();
					// Move the connected server into a task to handle this client
					tokio::spawn(async move {
						let mut server = server;
						if let Err(e) = handle_pipe_client(&mut server, st).await {
							warn!("client error: {}", e);
						}
					});
				}
				Err(e) => {
					warn!("pipe connect error: {}", e);
					// brief backoff to avoid tight error loop
					tokio::time::sleep(std::time::Duration::from_millis(200)).await;
				}
			}
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
	#[cfg(windows)]
	{
		// Best-effort on windows without unsafe: mark the cookie as read-only.
		// Files under %APPDATA% are already private to the current user by default ACLs.
		if let Ok(meta) = std::fs::metadata(&cookie_path) {
			let mut perm = meta.permissions();
			perm.set_readonly(true);
			let _ = std::fs::set_permissions(&cookie_path, perm);
		}
	}
	info!("generated control auth cookie at {}", cookie_path.display());
	Some(tok)
}

fn default_cookie_path() -> std::path::PathBuf {
	#[cfg(windows)]
	{
		if let Ok(app_data) = std::env::var("APPDATA") {
			return std::path::Path::new(&app_data).join("nyx").join("control.authcookie");
		}
		std::path::PathBuf::from("control.authcookie")
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
	match read_one_line_with_timeout(&mut stream, &mut buf, INITIAL_READ_TIMEOUT_MS).await {
		Ok(_) => {},
		Err(_) => { return Ok(()); } // drop slow/idle client silently
	}
	let req = std::str::from_utf8(&buf).unwrap_or("");
	let (resp, stream_back, filter) = process_request(req, &state).await;
	let resp_id = resp.id.clone();
	let json = json_util::encode_to_vec(&resp).unwrap_or_else(|e| {
		#[cfg(feature = "telemetry")]
		telemetry::record_counter("nyx_daemon_serde_error", 1);
		serde_json::to_vec(&Response::<serde_json::Value>::err_with_id(resp_id, 500, e))?
	});
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	// If client requested subscription, stream events until client disconnects
	if let Some(mut rx) = stream_back {
		while let Ok(ev) = rx.recv().await {
			if !state.events.matches(&ev, &filter).await { continue; }
			let line = match json_util::encode_to_vec(&ev) {
				Ok(v) => v,
				Err(e) => { warn!("failed to serialize event: {}", e); continue; }
			};
			if stream.write_all(&line).await.is_err() { break; }
			if stream.write_all(b"\n").await.is_err() { break; }
			if stream.flush().await.is_err() { break; }
		}
	}
	Ok(())
}

#[cfg(windows)]
async fn handle_pipe_client(stream: &mut tokio::net::windows::named_pipe::NamedPipeServer, state: Arc<DaemonState>) -> io::Result<()> {
	// Named pipes on windows are byte streams; read until newline or timeout
	let mut buf = Vec::with_capacity(1024);
	match read_one_line_with_timeout(stream, &mut buf, INITIAL_READ_TIMEOUT_MS).await {
		Ok(_) => {},
		Err(_) => { return Ok(()); }
	}
	let req = std::str::from_utf8(&buf).unwrap_or("");
	let (resp, stream_back, filter) = process_request(req, &state).await;
	let resp_id = resp.id.clone();
	let json = json_util::encode_to_vec(&resp).unwrap_or_else(|e| {
		#[cfg(feature = "telemetry")]
		telemetry::record_counter("nyx_daemon_serde_error", 1);
		serde_json::to_vec(&Response::<serde_json::Value>::err_with_id(resp_id, 500, e.to_string())).unwrap_or_default()
	});
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	// Stream events if subscribed until client disconnects
	if let Some(mut rx) = stream_back {
		while let Ok(ev) = rx.recv().await {
			if !state.events.matches(&ev, &filter).await { continue; }
			let line = match json_util::encode_to_vec(&ev) { Ok(v) => v, Err(_) => continue };
			if stream.write_all(&line).await.is_err() { break; }
			if stream.write_all(b"\n").await.is_err() { break; }
			if stream.flush().await.is_err() { break; }
		}
	}
	Ok(())
}

// Minimal 1-line reader with timeout and CRLF handling (mirrors SDK behavior)
async fn read_one_line_with_timeout<R: tokio::io::AsyncRead + Unpin>(reader: &mut R, out: &mut Vec<u8>, timeout_ms: u64) -> io::Result<()> {
	use tokio::time::{timeout, Duration, Instant};
	let deadline = Duration::from_millis(timeout_ms);
	let start = Instant::now();
	out.clear();
	let mut tmp = [0u8; 256];
	loop {
		let remain = deadline.saturating_sub(start.elapsed());
		if remain.is_zero() { break; }
		let n = match timeout(remain, reader.read(&mut tmp)).await {
			Ok(Ok(n)) => n,
			Ok(Err(e)) => return Err(e),
			Err(_) => break,
		};
		if n == 0 { break; }
		out.extend_from_slice(&tmp[..n]);
		if out.contains(&b'\n') { break; }
		if out.len() > 64 * 1024 { break; }
	}
	if let Some(pos) = memchr::memchr(b'\n', out) { out.truncate(pos); }
	if out.last().copied() == Some(b'\r') { out.pop(); }
	Ok(())
}

async fn process_request(req_line: &str, state: &DaemonState) -> (Response<serde_json::Value>, Option<tokio::sync::broadcast::Receiver<Event>>, Option<Vec<String>>) {
	match json_util::decode_from_str::<RpcRequest>(req_line) {
	Ok(RpcRequest { id, auth: _, req: Request::GetInfo }) => {
			let info = Info {
				node_id: hex::encode(state.node_id),
				version: env!("CARGO_PKG_VERSION").to_string(),
				uptime_sec: state.start_time.elapsed().as_secs() as u32,
			};
			match serde_json::to_value(info) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::ReloadConfig }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let res = state.cfg.reload_from_file().await.unwrap_or_else(|e| ConfigResponse { __succes_s: false, _message: e.to_string(), __validation_error_s: vec![] });
			#[cfg(feature = "telemetry")]
			if !res.__succes_s { telemetry::record_counter("nyx_daemon_reload_fail", 1); }
			if res.__succes_s { let _ = state.events.sender().send(Event { _ty: "system".into(), _detail: "config_reloaded".into() }); }
			match serde_json::to_value(res) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::UpdateConfig { settings } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let res = state.cfg.updateconfig(settings).await.unwrap_or_else(|e| ConfigResponse { __succes_s: false, _message: e.to_string(), __validation_error_s: vec![] });
			#[cfg(feature = "telemetry")]
			if !res.__succes_s { telemetry::record_counter("nyx_daemon_update_fail", 1); }
			if res.__succes_s { let _ = state.events.sender().send(Event { _ty: "system".into(), _detail: "config_updated".into() }); }
			match serde_json::to_value(res) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::SubscribeEvents { types } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let rx = state.events.subscribe();
			(Response::ok_with_id(id, serde_json::json!({"subscribed": true})), Some(rx), types)
		}
		Ok(RpcRequest { id, auth, req: Request::ListConfigVersions }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let list: Vec<VersionSummary> = state.cfg.listversion_s().await;
			match serde_json::to_value(list) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::RollbackConfig { version } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let res = state.cfg.rollback(version).await.unwrap_or_else(|e| ConfigResponse { __succes_s: false, _message: e.to_string(), __validation_error_s: vec![] });
			#[cfg(feature = "telemetry")]
			if !res.__succes_s { telemetry::record_counter("nyx_daemon_rollback_fail", 1); }
			if res.__succes_s { let _ = state.events.sender().send(Event { _ty: "system".into(), _detail: format!("config_rolled_back:{version}") }); }
			match serde_json::to_value(res) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::CreateConfigSnapshot { description } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			match state.cfg.snapshot(description.as_deref().unwrap_or("manual_snapshot")).await {
				Ok(ver) => (Response::ok_with_id(id, serde_json::json!({"version": ver})), None, None),
				Err(e) => {
					#[cfg(feature = "telemetry")]
					telemetry::record_counter("nyx_daemon_snapshot_fail", 1);
					(Response::err_with_id(id, 500, e.to_string()), None, None)
				},
			}
		}
		#[cfg(feature = "low_power")]
		Ok(RpcRequest { id, auth, req: Request::SetPowerState { state: s } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let rc = nyx_mobile_ffi::nyx_power_set_state(s);
			if rc == nyx_mobile_ffi::NyxStatus::Ok as i32 {
				(Response::ok_with_id(id, serde_json::json!({"set": true, "state": s})), None, None)
			} else {
				(Response::err_with_id(id, 400, format!("ffi_error:{rc}")), None, None)
			}
		}
		Ok(RpcRequest { id, req: Request::GetSystemInfo, .. }) => {
			let info = SystemInfo {
				uptime: state.start_time.elapsed().as_secs(),
				version: env!("CARGO_PKG_VERSION").to_string(),
				build_time: option_env!("BUILD_TIME").unwrap_or("unknown").to_string(),
				memory_usage: get_memory_usage(),
			};
			match serde_json::to_value(info) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, req: Request::Health, .. }) => {
			let status = HealthStatus {
				healthy: true,
				timestamp: chrono::Utc::now(),
				components: get_component_status(state).await,
			};
			match serde_json::to_value(status) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Err(e) => {
			#[cfg(feature = "telemetry")]
			telemetry::record_counter("nyx_daemon_bad_request", 1);
			(Response::err_with_id(None, 400, format!("invalid request: {e}")), None, None)
		},
	}
}

fn is_authorized(state: &DaemonState, auth: Option<&str>) -> bool {
	// Strict auth mode: if NYX_DAEMON_STRICT_AUTH=1, require token to be set and provided
	let strict = std::env::var("NYX_DAEMON_STRICT_AUTH").ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
	// Treat empty or whitespace-only token as not set (disabled auth)
	let effective = state
		.token
		.as_deref()
		.map(|s| s.trim())
		.filter(|s| !s.is_empty());

	if effective.is_none() {
		if strict {
			warn!("authorization failed in strict mode: token not configured");
			return false;
		}
		// if no token is set, allow all (development default)
		// Emit a one-time startup warning to make the posture explicit
		static ONCE: std::sync::Once = std::sync::Once::new();
		ONCE.call_once(|| warn!("daemon started without auth token; NYX_DAEMON_STRICT_AUTH=1 will enforce token"));
		return true;
	}

	let expected = effective.unwrap();
	match auth {
		Some(provided) => {
			let ok = provided == expected;
			if !ok { warn!("authorization failed: wrong token"); }
			ok
		}
		None => {
			warn!("authorization failed: missing token");
			false
		}
	}
}

fn get_memory_usage() -> u64 {
	#[cfg(unix)]
	{
		if let Ok(contents) = std::fs::read_to_string("/proc/self/status") {
			for line in contents.lines() {
				if let Some(value) = line.strip_prefix("VmRSS:") {
					if let Some(kb_str) = value.trim().split_whitespace().next() {
						if let Ok(kb) = kb_str.parse::<u64>() {
							return kb * 1024; // Convert KB to bytes
						}
					}
				}
			}
		}
	}
	0
}

async fn get_component_status(_state: &DaemonState) -> std::collections::HashMap<String, String> {
	let mut components = std::collections::HashMap::new();
	components.insert("config".to_string(), "ok".to_string());
	components.insert("events".to_string(), "ok".to_string());
	components.insert("daemon".to_string(), "ok".to_string());
	components
}

#[cfg(test)]
mod tests {
	use super::*;
	use tempfile::tempdir;
	use std::sync::{Mutex, OnceLock};

	fn with_env_lock<F: FnOnce() -> R, R>(f: F) -> R {
		static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
		let m = LOCK.get_or_init(|| Mutex::new(()));
		let _g = m.lock().unwrap();
		f()
	}

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
	async fn list_versions_after_snapshot() -> Result<(), Box<dyn std::error::Error>> {
		let state = make_state_with_token(None);
		let _ = state.cfg.snapshot("t").await?;
		let req = serde_json::json!({
			"id": "v1",
			"op": "list_config_versions"
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		let v: Vec<VersionSummary> = serde_json::from_value(resp.data.unwrap())?;
		assert!(!v.is_empty());
		Ok(())
	}

	#[tokio::test]
	async fn rollback_succeeds_with_valid_version() -> Result<(), Box<dyn std::error::Error>> {
		let state = make_state_with_token(Some("t"));
		let ver = state.cfg.snapshot("before").await?;
		let req = serde_json::json!({
			"id": "rb1",
			"auth": "t",
			"op": "rollback_config",
			"version": ver
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok, "{resp:?}");
		let cr: ConfigResponse = serde_json::from_value(resp.data.unwrap())?;
		assert!(cr.__succes_s);
		Ok(())
	}

	#[tokio::test]
	async fn manual_snapshot_returns_version() -> Result<(), Box<dyn std::error::Error>> {
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
		Ok(())
	}

	#[tokio::test]
	async fn invalid_request_returns_400() {
		let state = make_state_with_token(None);
		let req = "{ not_json }"; // Parse error
		let (resp, _rx, _filter) = process_request(req, &state).await;
		assert!(!resp.ok);
		assert_eq!(resp.code, 400);
		assert!(resp.error.unwrap().contains("invalid request"));
	}

	#[tokio::test]
	async fn empty_env_token_is_treated_as_disabled() -> Result<(), Box<dyn std::error::Error>> {
		// Serialize env-dependent behavior across tests
		let _ = with_env_lock(|| {
			// Ensure strict mode is not enabled (tests may run in parallel)
			std::env::remove_var("NYX_DAEMON_STRICT_AUTH");
		});
		// Keep the rest under the same lock by immediately reacquiring
		let _guard = {
			use std::sync::{Mutex, OnceLock};
			static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
			LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
		};
		// Simulate daemon started with empty token ("   ") which should disable auth
		let state = make_state_with_token(Some("   "));
		// ensure internal state reflects token provided but authorization treats it as None
		let req = serde_json::json!({
			"id": "r1",
			"op": "reload_config"
		}).to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok, "auth should be disabled when token is empty/whitespace");
		Ok(())
	}

	#[test]
	fn cookie_is_created_when_env_and_file_missing() -> Result<(), Box<dyn std::error::Error>> {
		with_env_lock(|| {
			// override default cookie path to temp
			let dir = tempdir()?;
			let cookie = dir.path().join("control.authcookie");
			std::env::set_var("NYX_DAEMON_COOKIE", &cookie);
			std::env::remove_var("NYX_DAEMON_TOKEN");
			let tok = ensure_token_from_env_or_cookie();
			assert!(tok.is_some());
			assert!(cookie.exists());
			Ok(())
		})
	}

	#[test]
	fn strict_auth_blocks_without_token() {
		with_env_lock(|| {
			std::env::set_var("NYX_DAEMON_STRICT_AUTH", "1");
			let st = make_state_with_token(None);
			let ok = is_authorized(&st, None);
			assert!(!ok);
			std::env::remove_var("NYX_DAEMON_STRICT_AUTH");
		});
	}
}

