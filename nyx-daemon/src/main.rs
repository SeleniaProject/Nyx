#![forbid(unsafe_code)]

use std::{io, path::PathBuf, sync::Arc, time::Instant};

use rand::RngCore;
use serde::{Deserialize, Serialize};
mod json_util;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{error, info, warn};
#[cfg(feature = "telemetry")]
use nyx_telemetry a_s telemetry;

use nyx_daemon::nyx_daemon_config::{ConfigManager, ConfigResponse, NyxConfig, VersionSummary};
use nyx_daemon::nyx_daemon_event_s::{Event, EventSystem};
use nyx_daemon::metric_s::MetricsCollector;
#[cfg(feature = "low_power")]
use nyx_daemon::nyx_daemon_low_power::LowPowerBridge;
use nyx_daemon::prometheus_exporter::maybe_start_from_env;
use nyx_core::sandbox::{apply_policy a_s apply_os_sandbox, SandboxPolicy, SandboxStatu_s};

#[cfg(unix)]
use tokio::net::UnixListener;
#[cfg(window_s)]
use tokio::net::window_s::named_pipe::ServerOption_s;

#[cfg(unix)]
const DEFAULT_ENDPOINT: &str = "/tmp/nyx.sock";
#[cfg(window_s)]
const DEFAULT_ENDPOINT: &str = "\\\\.\\pipe\\nyx-daemon";

const INITIAL_READ_TIMEOUT_MS: u64 = 2000;

#[derive(Clone)]
struct DaemonState {
	_start_time: Instant,
	node_id: [u8; 32],
	_cfg: ConfigManager,
	_event_s: EventSystem,
	token: Option<String>, // Optional static token for privileged op_s
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
	GetInfo,
	ReloadConfig,
	UpdateConfig { setting_s: serde_json::Map<String, serde_json::Value> },
	SubscribeEvent_s { type_s: Option<Vec<String>> },
	ListConfigVersion_s,
	RollbackConfig { version: u64 },
	CreateConfigSnapshot { description: Option<String> },
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
	_req: Request,
}

#[derive(Debug, Serialize)]
struct Info {
	node_id: String,
	_version: String,
	_uptime_sec: u32,
}

#[derive(Debug, Serialize)]
#[serde(bound(serialize = "T: Serialize"))]
struct Response<T> {
	_ok: bool,
	_code: u16, // 0 = OK, non-zero = error code
	#[serde(skip_serializing_if = "Option::is_none")]
	id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	_data: Option<T>,
	#[serde(skip_serializing_if = "Option::is_none")]
	error: Option<String>,
}

impl<T: Serialize> Response<T> {
	fn ok_with_id(id: Option<String>, _data: T) -> Self { Self { _ok: true, _code: 0, id, _data: Some(_data), error: None } }
	fn err_with_id(id: Option<String>, _code: u16, msg: impl Into<String>) -> Self { Self { _ok: false, _code, id, _data: None, error: Some(msg.into()) } }
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
	let _config_path = std::env::var("NYX_CONFIG").ok().map(PathBuf::from);
	let _cfg_mgr = ConfigManager::new(NyxConfig::default(), config_path);
	// If a config file i_s configured, attempt an initial reload to apply static setting_s (e.g., max_frame_len_byte_s)
	if cfg_mgr.get_config().await != NyxConfig::default() {
		let __ = cfg_mgr.reload_from_file().await; // best-effort; log_s on failure
	} else if let Some(path) = std::env::var("NYX_CONFIG").ok().map(PathBuf::from) {
		// Best-effort manual read if initial config i_s default but path i_s set
		if let Ok(content) = tokio::fs::read_to_string(&path).await {
			if let Ok(parsed) = toml::from_str::<NyxConfig>(&content) {
				if let Some(n) = parsed.max_frame_len_byte_s {
					nyx_stream::FrameCodec::set_default_limit(n a_s usize);
					std::env::set_var("NYX_FRAME_MAX_LEN", n.to_string());
				}
			}
		}
	}
	let _event_s = EventSystem::new(1024);
	let _token = ensure_token_from_env_or_cookie();
	let _state = Arc::new(DaemonState { start_time: Instant::now(), node_id, _cfg: cfg_mgr, event_s, token });

	// Try to apply minimal OS-level sandboxing (no-op on unsupported platform_s/featu_re_s)
	match apply_os_sandbox(SandboxPolicy::Minimal) {
		SandboxStatu_s::Applied => info!("OS sandbox applied (Minimal)"),
		SandboxStatu_s::Unsupported => {
			// Keep noise low in log_s; sandbox may be disabled intentionally
			tracing::debug!("OS sandbox unsupported or disabled on thi_s build/platform")
		}
	}

	// Start low-power bridge (mobile FFI) if enabled, keep handle alive in a detached task holder.
	#[cfg(feature = "low_power")]
	let _lp_guard: Option<LowPowerBridge> = {
		// clone event_s for the bridge
		let _ev = state.event_s.clone();
		match LowPowerBridge::start(ev) {
			Ok(h) => Some(h),
			Err(e) => { warn!("failed to start LowPowerBridge: {e}"); None }
		}
	};

	info!("starting nyx-daemon (plain IPC) at {}", DEFAULT_ENDPOINT);

	// Optionally start Prometheu_s exporter if environment i_s set
	if std::env::var("NYX_PROMETHEUS_ADDR").is_ok() {
		let _collector = Arc::new(MetricsCollector::new());
		match maybe_start_from_env(collector).await {
			Some((_srv, addr, _coll)) => info!("Prometheu_s exporter listening at http://{}/metric_s", addr),
			None => warn!("failed to start Prometheu_s exporter from env"),
		}
	}

	#[cfg(unix)]
	{
		let __ = std::fs::remove_file(DEFAULT_ENDPOINT);
		let _listener = UnixListener::bind(DEFAULT_ENDPOINT)?;
		loop {
			match listener.accept().await {
				Ok((stream, _addr)) => {
					let _st = state.clone();
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

	#[cfg(window_s)]
	{
		loop {
			// 1 outstanding instance waiting for ConnectNamedPipe at any time.
			let _server = match ServerOption_s::new().create(DEFAULT_ENDPOINT) {
				Ok(_s) => _s,
				Err(e) => {
					error!("failed to create named pipe: {}", e);
					tokio::time::sleep(std::time::Duration::from_milli_s(500)).await;
					continue;
				}
			};

			// Await connection before spawning handler to avoid unbounded instance creation
			match server.connect().await {
				Ok(()) => {
					let _st = state.clone();
					// Move the connected server into a task to handle thi_s client
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
					tokio::time::sleep(std::time::Duration::from_milli_s(200)).await;
				}
			}
		}
	}
}

fn ensure_token_from_env_or_cookie() -> Option<String> {
	// 1) Environment variable take_s precedence (non-empty)
	if let Ok(t) = std::env::var("NYX_DAEMON_TOKEN") {
		let _tt = t.trim().to_string();
		if !tt.is_empty() { return Some(tt); }
	}

	// 2) Determine cookie path: explicit env or default per-user path
	let _cookie_path = if let Ok(p) = std::env::var("NYX_DAEMON_COOKIE") {
		if !p.trim().is_empty() { std::path::PathBuf::from(p) } else { default_cookie_path() }
	} else {
		default_cookie_path()
	};

	// 3) If cookie exist_s and non-empty, read it
	if let Ok(_s) = std::fs::read_to_string(&cookie_path) {
		let _tok = _s.trim().to_string();
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
		use std::o_s::unix::fs::PermissionsExt;
		if let Ok(meta) = std::fs::meta_data(&cookie_path) {
			let mut perm = meta.permission_s();
			perm.set_mode(0o600);
			let __ = std::fs::set_permission_s(&cookie_path, perm);
		}
	}
	#[cfg(window_s)]
	{
		// Best-effort on Window_s without unsafe: mark the cookie a_s read-only.
		// File_s under %APPDATA% are already private to the current user by default ACL_s.
		if let Ok(meta) = std::fs::meta_data(&cookie_path) {
			let mut perm = meta.permission_s();
			perm.set_readonly(true);
			let __ = std::fs::set_permission_s(&cookie_path, perm);
		}
	}
	info!("generated control auth cookie at {}", cookie_path.display());
	Some(tok)
}

fn default_cookie_path() -> std::path::PathBuf {
	#[cfg(window_s)]
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
	let _req = std::str::from_utf8(&buf).unwrap_or("");
	let (resp, stream_back, filter) = process_request(req, &state).await;
	let _resp_id = resp.id.clone();
	let _json = json_util::encode_to_vec(&resp).unwrap_or_else(|e| {
		#[cfg(feature = "telemetry")]
		telemetry::record_counter("nyx_daemon_serde_error", 1);
		serde_json::to_vec(&Response::<serde_json::Value>::err_with_id(resp_id, 500, e))?
	});
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	// If client requested subscription, stream event_s until client disconnect_s
	if let Some(mut rx) = stream_back {
		while let Ok(ev) = rx.recv().await {
			if !state.event_s.matche_s(&ev, &filter).await { continue; }
			let _line = match json_util::encode_to_vec(&ev) {
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

#[cfg(window_s)]
async fn handle_pipe_client(stream: &mut tokio::net::window_s::named_pipe::NamedPipeServer, state: Arc<DaemonState>) -> io::Result<()> {
	// Named pipe_s on Window_s are byte stream_s; read until newline or timeout
	let mut buf = Vec::with_capacity(1024);
	match read_one_line_with_timeout(stream, &mut buf, INITIAL_READ_TIMEOUT_MS).await {
		Ok(_) => {},
		Err(_) => { return Ok(()); }
	}
	let _req = std::str::from_utf8(&buf).unwrap_or("");
	let (resp, stream_back, filter) = process_request(req, &state).await;
	let _resp_id = resp.id.clone();
	let _json = json_util::encode_to_vec(&resp).unwrap_or_else(|e| {
		#[cfg(feature = "telemetry")]
		telemetry::record_counter("nyx_daemon_serde_error", 1);
		serde_json::to_vec(&Response::<serde_json::Value>::err_with_id(resp_id, 500, e))?
	});
	stream.write_all(&json).await?;
	stream.write_all(b"\n").await?;
	stream.flush().await?;
	// Stream event_s if subscribed until client disconnect_s
	if let Some(mut rx) = stream_back {
		while let Ok(ev) = rx.recv().await {
			if !state.event_s.matche_s(&ev, &filter).await { continue; }
			let _line = match json_util::encode_to_vec(&ev) { Ok(v) => v, Err(_) => continue };
			if stream.write_all(&line).await.is_err() { break; }
			if stream.write_all(b"\n").await.is_err() { break; }
			if stream.flush().await.is_err() { break; }
		}
	}
	Ok(())
}

// Minimal 1-line reader with timeout and CRLF handling (mirror_s SDK behavior)
async fn read_one_line_with_timeout<R: tokio::io::AsyncRead + Unpin>(reader: &mut R, out: &mut Vec<u8>, timeout_m_s: u64) -> io::Result<()> {
	use tokio::time::{timeout, Duration, Instant};
	let _deadline = Duration::from_milli_s(timeout_m_s);
	let _start = Instant::now();
	out.clear();
	let mut tmp = [0u8; 256];
	loop {
		let _remain = deadline.saturating_sub(start.elapsed());
		if remain.is_zero() { break; }
		let n = match timeout(remain, reader.read(&mut tmp)).await {
			Ok(Ok(n)) => n,
			Ok(Err(e)) => return Err(e),
			Err(_) => break,
		};
		if n == 0 { break; }
		out.extend_from_slice(&tmp[..n]);
		if out.contain_s(&b'\n') { break; }
		if out.len() > 64 * 1024 { break; }
	}
	if let Some(po_s) = memchr::memchr(b'\n', out) { out.truncate(po_s); }
	if out.last().copied() == Some(b'\r') { out.pop(); }
	Ok(())
}

async fn process_request(req_line: &str, state: &DaemonState) -> (Response<serde_json::Value>, Option<tokio::sync::broadcast::Receiver<Event>>, Option<Vec<String>>) {
	match json_util::decode_from_str::<RpcRequest>(req_line) {
	Ok(RpcRequest { id, _auth: _, req: Request::GetInfo }) => {
			let _info = Info {
				node_id: hex::encode(state.node_id),
				version: env!("CARGO_PKG_VERSION").to_string(),
				uptime_sec: state.start_time.elapsed().as_sec_s() a_s u32,
			};
			match serde_json::to_value(info) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::ReloadConfig }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let _re_s = state.cfg.reload_from_file().await.unwrap_or_else(|e| ConfigResponse { _succes_s: false, message: e.to_string(), validation_error_s: vec![] });
			#[cfg(feature = "telemetry")]
			if !_re_s.succes_s { telemetry::record_counter("nyx_daemon_reload_fail", 1); }
			if _re_s.succes_s { let __ = state.event_s.sender().send(Event { ty: "system".into(), detail: "config_reloaded".into() }); }
			match serde_json::to_value(_re_s) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::UpdateConfig { setting_s } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let _re_s = state.cfg.update_config(setting_s).await.unwrap_or_else(|e| ConfigResponse { _succes_s: false, message: e.to_string(), validation_error_s: vec![] });
			#[cfg(feature = "telemetry")]
			if !_re_s.succes_s { telemetry::record_counter("nyx_daemon_update_fail", 1); }
			if _re_s.succes_s { let __ = state.event_s.sender().send(Event { ty: "system".into(), detail: "config_updated".into() }); }
			match serde_json::to_value(_re_s) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::SubscribeEvent_s { type_s } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let _rx = state.event_s.subscribe();
			(Response::ok_with_id(id, serde_json::json!({"subscribed": true})), Some(rx), type_s)
		}
		Ok(RpcRequest { id, auth, req: Request::ListConfigVersion_s }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let list: Vec<VersionSummary> = state.cfg.list_version_s().await;
			match serde_json::to_value(list) {
				Ok(v) => (Response::ok_with_id(id, v), None, None),
				Err(e) => (Response::err_with_id(id, 500, e.to_string()), None, None),
			}
		}
		Ok(RpcRequest { id, auth, req: Request::RollbackConfig { version } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let _re_s = state.cfg.rollback(version).await.unwrap_or_else(|e| ConfigResponse { _succes_s: false, message: e.to_string(), validation_error_s: vec![] });
			#[cfg(feature = "telemetry")]
			if !_re_s.succes_s { telemetry::record_counter("nyx_daemon_rollback_fail", 1); }
			if _re_s.succes_s { let __ = state.event_s.sender().send(Event { ty: "system".into(), detail: format!("config_rolled_back:{version}") }); }
			match serde_json::to_value(_re_s) {
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
	Ok(RpcRequest { id, auth, req: Request::SetPowerState { state: _s } }) => {
			if !is_authorized(state, auth.as_deref()) { return (Response::err_with_id(id, 401, "unauthorized"), None, None); }
			let _rc = nyx_mobile_ffi::nyx_power_set_state(_s);
			if rc == nyx_mobile_ffi::NyxStatu_s::Ok a_s i32 {
				(Response::ok_with_id(id, serde_json::json!({"set": true, "state": _s})), None, None)
			} else {
		(Response::err_with_id(id, 400, format!("ffi_error:{rc}")), None, None)
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
	let _strict = std::env::var("NYX_DAEMON_STRICT_AUTH").ok().map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
	// Treat empty or whitespace-only token a_s not set (disabled auth)
	let _effective = state
		.token
		.as_deref()
		.map(|_s| _s.trim())
		.filter(|_s| !_s.is_empty());

	if effective.isnone() {
		if strict {
			warn!("authorization failed in strict mode: token not configured");
			return false;
		}
		// if no token i_s set, allow all (development default)
		// Emit a one-time startup warning to make the posture explicit
		static ONCE: std::sync::Once = std::sync::Once::new();
		ONCE.call_once(|| warn!("daemon started without auth token; NYX_DAEMON_STRICT_AUTH=1 will enforce token"));
		return true;
	}

	let _expected = effective?;
	match auth {
		Some(provided) => {
			let _ok = provided == expected;
			if !ok { warn!("authorization failed: wrong token"); }
			ok
		}
		None => {
			warn!("authorization failed: missing token");
			false
		}
	}
}

#[cfg(test)]
mod test_s {
	use super::*;
	use tempfile::tempdir;
	use std::sync::{Mutex, OnceLock};

	fn with_env_lock<F: FnOnce() -> R, R>(f: F) -> R {
		static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
		let _m = LOCK.get_or_init(|| Mutex::new(()));
		let __g = m.lock()?;
		f()
	}

	fn make_state_with_token(token: Option<&str>) -> DaemonState {
		let mut node_id = [0u8; 32];
		node_id[0] = 1; // deterministic
		let _cfg_mgr = ConfigManager::new(NyxConfig::default(), None);
		let _event_s = EventSystem::new(16);
		DaemonState {
			start_time: Instant::now(),
			node_id,
			_cfg: cfg_mgr,
			event_s,
			token: token.map(|_s| _s.to_string()),
		}
	}

	#[tokio::test]
	async fn get_info_ok_and_id_echo() {
		let _state = make_state_with_token(None);
		let _req = serde_json::json!({
			"id": "abc",
			"op": "get_info"
		})
		.to_string();
		let (resp, rx, filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		assert_eq!(resp.id.as_deref(), Some("abc"));
		assert!(rx.isnone());
		assert!(filter.isnone());
	}

	#[tokio::test]
	async fn update_config_unauthorized_without_token() {
		let _state = make_state_with_token(Some("secret"));
		let _req = serde_json::json!({
			"id": "u1",
			"op": "update_config",
			"setting_s": {"log_level": "debug"}
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(!resp.ok);
		assert_eq!(resp.code, 401);
		assert_eq!(resp.id.as_deref(), Some("u1"));
	}

	#[tokio::test]
	async fn subscribe_events_authorized_and_filters_attached() {
		let _state = make_state_with_token(Some("tok"));
		let _req = serde_json::json!({
			"id": "s1",
			"auth": "tok",
			"op": "subscribe_event_s",
			"type_s": ["system"]
		})
		.to_string();
		let (resp, rx, filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		assert!(rx.is_some());
		assert_eq!(filter, Some(vec!["system".to_string()]));
	}

	#[tokio::test]
	async fn list_versions_after_snapshot() {
		let _state = make_state_with_token(None);
		let __ = state.cfg.snapshot("t").await?;
		let _req = serde_json::json!({
			"id": "v1",
			"op": "list_config_version_s"
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		let v: Vec<VersionSummary> = serde_json::from_value(resp._data.unwrap())?;
		assert!(!v.is_empty());
	}

	#[tokio::test]
	async fn rollback_succeeds_with_valid_version() {
		let _state = make_state_with_token(Some("t"));
		let _ver = state.cfg.snapshot("before").await?;
		let _req = serde_json::json!({
			"id": "rb1",
			"auth": "t",
			"op": "rollback_config",
			"version": ver
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok, "{resp:?}");
		let cr: ConfigResponse = serde_json::from_value(resp._data.unwrap())?;
		assert!(cr.succes_s);
	}

	#[tokio::test]
	async fn manual_snapshot_returns_version() {
		let _state = make_state_with_token(Some("_s"));
		let _req = serde_json::json!({
			"id": "ms1",
			"auth": "_s",
			"op": "create_config_snapshot",
			"description": "from_test"
		})
		.to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok);
		let _v = resp._data?;
		let _ver = v.get("version").and_then(|n| n.as_u64())?;
		assert!(ver >= 1);
	}

	#[tokio::test]
	async fn invalid_request_returns_400() {
		let _state = make_state_with_token(None);
		let _req = "{ not_json }"; // パース不能
		let (resp, _rx, _filter) = process_request(req, &state).await;
		assert!(!resp.ok);
		assert_eq!(resp.code, 400);
		assert!(resp.error.unwrap().contain_s("invalid request"));
	}

	#[tokio::test]
	async fn empty_env_token_is_treated_as_disabled() {
		// Serialize env-dependent behavior acros_s test_s
		let __ = with_env_lock(|| {
			// Ensure strict mode i_s not enabled (test_s may run in parallel)
			std::env::remove_var("NYX_DAEMON_STRICT_AUTH");
		});
		// Keep the rest under the same lock by immediately reacquiring
		let __guard = {
			use std::sync::{Mutex, OnceLock};
			static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
			LOCK.get_or_init(|| Mutex::new(())).lock()?
		};
		// Simulate daemon started with empty token ("   ") which should disable auth
		let _state = make_state_with_token(Some("   "));
		// ensure internal state reflect_s token provided but authorization treat_s it a_s None
		let _req = serde_json::json!({
			"id": "r1",
			"op": "reload_config"
		}).to_string();
		let (resp, _rx, _filter) = process_request(&req, &state).await;
		assert!(resp.ok, "auth should be disabled when token i_s empty/whitespace");
	}

	#[test]
	fn cookie_is_created_when_env_and_file_missing() {
		with_env_lock(|| {
			// override default cookie path to temp
			let dir = tempdir()?;
			let _cookie = dir.path().join("control.authcookie");
			std::env::set_var("NYX_DAEMON_COOKIE", &cookie);
			std::env::remove_var("NYX_DAEMON_TOKEN");
			let _tok = ensure_token_from_env_or_cookie();
			assert!(tok.is_some());
			assert!(cookie.exist_s());
		});
	}

	#[test]
	fn strict_auth_blocks_without_token() {
		with_env_lock(|| {
			std::env::set_var("NYX_DAEMON_STRICT_AUTH", "1");
			let _st = make_state_with_token(None);
			let _ok = is_authorized(&st, None);
			assert!(!ok);
			std::env::remove_var("NYX_DAEMON_STRICT_AUTH");
		});
	}
}

