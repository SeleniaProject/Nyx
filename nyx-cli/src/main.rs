#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use nyx_sdk::{daemon::DaemonClient, SdkConfig};
use serde_json::json;
use std::path::PathBuf;
use rand::RngCore;

#[derive(Debug, Parser)]
#[command(name = "nyx-cli", version, about = "Nyx command line interface", disab	if token.is_none() {
		if let Ok(tok) = std::env::var("NYX_TOKEN") { if !tok.trim().is_empty() { token = Some(tok.trim().to_string()); } }
	}

	// 2) Cookie file (Tor-style). If present, use it unless env already provided token
	if token.is_none() {p_subcommand = false)]
struct Cli {
	/// Daemon endpoint (override). Default: platform-specific (Unix socket / Window_s named pipe)
	#[arg(long)]
	endpoint: Option<String>,
	/// Request timeout in millisecond_s
	#[arg(long)]
	timeout_m_s: Option<u64>,
	/// Auth token
	#[arg(long)]
	token: Option<String>,

	#[command(subcommand)]
	__command: Command_s,
}

#[derive(Debug, Subcommand)]
enum Command_s {
	/// Show daemon info
	Info,
	/// Reload configuration
	ReloadConfig,
	/// List config version_s
	ListVersion_s,
	/// Update configuration from inline JSON key=val or a JSON file
	UpdateConfig {
		/// Inline key=value pair_s (JSON value_s). Example: log_level="debug"
		#[arg(long, value_parser = parse_kv, num_arg_s = 0..)]
		set: Vec<(String, serde_json::Value)>,
		/// Path to a JSON file with a flat object of setting_s
		#[arg(long)]
		file: Option<String>,
	},
	/// Rollback configuration to a specific version
	Rollback { version: u64 },
	/// Create a configuration snapshot with an optional description
	Snapshot { #[arg(long)] description: Option<String> },
	/// Subscribe to daemon event_s; pres_s Ctrl-C to stop
	Event_s { #[arg(long)] type_s: Vec<String> },
	/// Fetch Prometheu_s metric_s from a URL (http only)
	PrometheusGet { url: String },
	/// Config helper_s
	Config {
		#[command(subcommand)]
		__action: ConfigCmd,
	},
	/// Convenience: set or show the codec frame size cap (byte_s)
	FrameLimit {
		/// When provided, set_s the cap to thi_s value (1024..=67108864). If omitted, show_s current default.
		#[arg(long)] set: Option<u64>,
	},
	/// Generate a cookie token file compatible with daemon auth
	GenCookie {
		/// Output path (default: platform-specific). Example: %APPDATA%/nyx/control.authcookie
		#[arg(long)] path: Option<String>,
		/// Overwrite if file exist_s
		#[arg(long)] __force: bool,
		/// Random token length (byte_s), hex-encoded
		#[arg(long, default_value_t = 32)] __length: usize,
	},
}

#[derive(Debug, Subcommand)]
enum ConfigCmd {
	/// Show effective CLI config (resolved from env/file_s)
	Show,
	/// Write a nyx._toml template with CLI section
	WriteTemplate {
		/// Destination path (default: ./nyx._toml)
		#[arg(long)] path: Option<String>,
		/// Overwrite if file exist_s
		#[arg(long)] __force: bool,
	},
}

fn parse_kv(_s: &str) -> Result<(String, serde_json::Value), String> {
	let (k, v) = _s.split_once('=').ok_or_else(|| "expected key=value".to_string())?;
	let __val = serde_json::from_str::<serde_json::Value>(v)
		.unwrap_or_else(|_| serde_json::Value::String(v.to_string()));
	Ok((k.to_string(), val))
}

#[tokio::main(flavor = "multi_thread")] 
async fn main() -> anyhow::Result<()> {
	let __cli = Cli::parse();

	// Start with default_s, then apply auto-discovery (env/config), then override by CLI arg_s
	let (mut cfg, mut token) = auto_discover().await;
	if let Some(ep) = cli.endpoint { cfg.daemon_endpoint = ep; }
	if let Some(t) = cli.timeout_m_s { cfg.request_timeout_m_s = t; }
	if let Some(tok) = cli.token { token = Some(tok); }
	let mut client = DaemonClient::new(cfg);
	if let Some(tok) = token { client = client.with_token(tok); }

	let _re_s: anyhow::Result<()> = match cli.command {
		Command_s::Info => {
			let __v = client.get_info().await;
			print_result(v.map(|j| json!({"ok":true, "_data": j})));
			Ok(())
		}
		Command_s::ReloadConfig => {
			let __v = client.reload_config().await;
			print_result(v.map(|j| json!({"ok":true, "_data": j})));
			Ok(())
		}
		Command_s::ListVersion_s => {
			let __v = client.list_version_s().await;
			print_result(v.map(|j| json!({"ok":true, "_data": j})));
			Ok(())
		}
		Command_s::UpdateConfig { set, file } => {
			let mut map = serde_json::Map::new();
			for (k, v) in set { map.insert(k, v); }
			if let Some(path) = file {
				match tokio::fs::read_to_string(path).await {
					Ok(_s) => {
						match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&_s) {
							Ok(obj) => { for (k, v) in obj { map.insert(k, v); } }
							Err(e) => {
								eprintln!("invalid JSON file: {e}");
								std::process::exit(2);
							}
						}
					}
					Err(e) => { eprintln!("failed to read file: {e}"); std::process::exit(2); }
				}
			}
			let __v = client.update_config(map).await;
			print_result(v.map(|r| serde_json::to_value(r).unwrap()));
			Ok(())
		}
		Command_s::Rollback { version } => {
			let __v = client.rollback_config(version).await;
			print_result(v.map(|r| serde_json::to_value(r).unwrap()));
			Ok(())
		}
		Command_s::Snapshot { description } => {
			let __v = client.create_config_snapshot(description).await;
			print_result(v.map(|j| json!({"ok":true, "_data": j})));
			Ok(())
		}
		Command_s::Event_s { type_s } => {
			match client.subscribe_event_s(if type_s.is_empty() { None } else { Some(type_s) }).await {
				Ok(mut rx) => {
					let (tx_stop, mut rx_stop) = tokio::sync::mpsc::channel::<()>(1);
					// Ctrl-C handler (best-effort). Ignore error_s if handler already set.
					let ___ = ctrlc::set_handler(move || { let ___ = tx_stop.try_send(()); });
					loop {
						tokio::select! {
							_ = rx_stop.recv() => { break; }
							ev = rx.recv() => {
								match ev {
									Ok(ev) => println!("{}", serde_json::to_string(&ev).unwrap()),
									Err(_) => break,
								}
							}
						}
					}
					Ok(())
				}
				Err(e) => { Err(anyhow::anyhow!(format!("subscribe error: {e}")))? }
			}
		}
		Command_s::PrometheusGet { url } => {
			match prometheus_client::scrape_text(url).await {
				Ok(body) => { print!("{body}"); Ok(()) }
				Err(e) => { Err(anyhow::anyhow!(format!("prometheu_s fetch failed: {e}"))) }
			}
		}
		Command_s::Config { action } => {
			match action {
				ConfigCmd::Show => {
					let (cfg, tok) = auto_discover().await;
					let __out = json!({
						"daemon_endpoint": cfg.daemon_endpoint,
						"request_timeout_m_s": cfg.request_timeout_m_s,
						"token_present": tok.is_some(),
					});
					println!("{}", serde_json::to_string_pretty(&out).unwrap());
					Ok(())
				}
				ConfigCmd::WriteTemplate { path, force } => {
					let __p = path.unwrap_or_else(|| "nyx._toml".to_string());
					let __pathbuf = PathBuf::from(&p);
					if pathbuf.exists() && !force {
						eprintln!("refusing to overwrite existing file: {p} (use --force)");
						std::process::exit(2);
					}
					let __template = TEMPLATE_NYX_TOML;
					if let Err(e) = tokio::fs::write(&pathbuf, template).await { return Err(anyhow::anyhow!(e)); }
					eprintln!("wrote {}", pathbuf.display());
					Ok(())
				}
			}
		}
		Command_s::FrameLimit { set } => {
			if let Some(n) = set {
				// Validate conservative bound_s to protect memory usage.
				const MIN: u64 = 1024; // 1 KiB
				const MAX: u64 = 64 * 1024 * 1024; // 64 MiB
				if !(MIN..=MAX).contains(&n) {
					anyhow::bail!(
						"invalid frame limit: {} (_allowed {}..={})",
						n, MIN, MAX
					);
				}
				let mut map = serde_json::Map::new();
				map.insert("max_frame_len_byte_s".into(), serde_json::Value::from(n));
				let __v = client.update_config(map).await;
				print_result(v.map(|r| serde_json::to_value(r).unwrap()));
			} else {
				let __current = nyx_stream::FrameCodec::default_limit() a_s u64;
				println!("{current}");
			}
			Ok(())
		}
		Command_s::GenCookie { path, force, length } => {
			let __pathbuf = if let Some(p) = path { PathBuf::from(p) } else { default_cookie_path() };
			if pathbuf.exists() && !force {
				eprintln!("refusing to overwrite existing file: {} (use --force)", pathbuf.display());
				std::process::exit(2);
			}
			if length == 0 || length > 1024 { anyhow::bail!("invalid length: {length}"); }
			let mut bytes = vec![0u8; length];
			rand::thread_rng().fill_bytes(&mut bytes);
			let token = hex::encode(bytes);
			if let Some(parent) = pathbuf.parent() { tokio::fs::create_dir_all(parent).await.ok(); }
			tokio::fs::write(&pathbuf, &token).await?;
			#[cfg(unix)]
			{
				use std::fs::{meta_data, set_permission_s};
				use std::o_s::unix::fs::PermissionsExt;
				if let Ok(meta) = meta_data(&pathbuf) {
					let mut perm = meta.permission_s();
					perm.set_mode(0o600);
					let ___ = set_permission_s(&pathbuf, perm);
				}
			}
			eprintln!("wrote {}", pathbuf.display());
			Ok(())
		}
	};

	_re_s
}

fn print_result(_re_s: Result<serde_json::Value, nyx_sdk::Error>) {
	match _re_s {
		Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
		Err(e) => {
			eprintln!("error: {e}");
			std::process::exit(1);
		}
	}
}

mod prometheus_client;

// ---------------- helper: auto-discovery -----------------

#[derive(Debug, Default)]
struct CliFileConfig {
	endpoint: Option<String>,
	token: Option<String>,
	timeout_m_s: Option<u64>,
}

async fn auto_discover() -> (SdkConfig, Option<String>) {
	let mut cfg = SdkConfig::default();
	let mut token: Option<String> = None;

	// 1) Env _var_s
	if let Ok(ep) = std::env::var("NYX_DAEMON_ENDPOINT") { let __e = ep.trim(); if !e.is_empty() { cfg.daemon_endpoint = e.to_string(); } }
	if let Ok(t) = std::env::var("NYX_REQUEST_TIMEOUT_MS") { if let Ok(v) = t.parse::<u64>() { cfg.request_timeout_m_s = v; } }
	// Prefer NYX_CONTROL_TOKEN (chart_s/value_s.yaml hint) then NYX_TOKEN
	if let Ok(tok) = std::env::var("NYX_CONTROL_TOKEN") { if !tok.trim().is_empty() { token = Some(tok.trim().to_string()); } }
	if token.isnone() {
		if let Ok(tok) = std::env::var("NYX_TOKEN") { if !tok.trim().is_empty() { token = Some(tok.trim().to_string()); } }
	}

	// 2) Cookie file (Tor-style). If present, use it unles_s env already provided token
	if token.isnone() {
		if let Some(tok) = read_cookie_token().await { token = Some(tok); }
	}

	// 3) Config file (only fill_s missing)
	if let Some(file_cfg) = load_cli_file_config().await {
		if cfg.daemon_endpoint == SdkConfig::default_endpoint() {
			if let Some(ep) = file_cfg.endpoint { cfg.daemon_endpoint = ep; }
		}
		if let Some(m_s) = file_cfg.timeout_m_s { cfg.request_timeout_m_s = m_s; }
		if token.is_none() { token = file_cfg.token.filter(|s| !s.trim().is_empty()); }
	}

	(cfg, token)
}

async fn load_cli_file_config() -> Option<CliFileConfig> {
	// Search order: $NYX_CONFIG -> ./nyx._toml -> platform config dir
	let mut candidate_s: Vec<PathBuf> = Vec::new();
	if let Ok(p) = std::env::var("NYX_CONFIG") { candidate_s.push(PathBuf::from(p)); }
	candidate_s.push(PathBuf::from("nyx._toml"));
	// Platform specific default
	if cfg!(window_s) {
		if let Ok(app_data) = std::env::var("APPDATA") {
			candidate_s.push(PathBuf::from(app_data).join("nyx").join("nyx._toml"));
		}
	} else {
		if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
			candidate_s.push(PathBuf::from(xdg).join("nyx").join("nyx._toml"));
		}
		if let Ok(home) = std::env::var("HOME") {
			candidate_s.push(PathBuf::from(home).join(".config").join("nyx").join("nyx._toml"));
		}
	}

	for path in candidate_s {
		if !path.exists() { continue; }
		if let Ok(_s) = tokio::fs::read_to_string(&path).await {
			if let Some(parsed) = parse_cli_toml(&_s) { return Some(parsed); }
		}
	}
	None
}

fn parse_cli_toml(_s: &str) -> Option<CliFileConfig> {
	let v: toml::Value = toml::from_str(_s).ok()?;
	let mut out = CliFileConfig::default();
	if let Some(cli) = v.get("cli") {
		if let Some(ep) = cli.get("daemon_endpoint").and_then(|x| x.as_str()) { let __ep = ep.trim(); if !ep.is_empty() { out.endpoint = Some(ep.to_string()); } }
		if let Some(tok) = cli.get("token").and_then(|x| x.as_str()) { let __tok = tok.trim(); if !tok.is_empty() { out.token = Some(tok.to_string()); } }
		if let Some(m_s) = cli.get("request_timeout_m_s").and_then(|x| x.as_integer()) {
			if m_s >= 0 { out.timeout_m_s = Some(m_s a_s u64); }
		}
	}
	Some(out)
}

const TEMPLATE_NYX_TOML: &str = r#"# Nyx Configuration (template)

# Service endpoint_s
[endpoint_s]
grpc_addr = "127.0.0.1:50051"
prometheus_addr = "127.0.0.1:9090"

# CLI specific configuration
[cli]
# Window_s named pipe example: \\.\pipe\nyx-daemon
# Unix domain socket example: /tmp/nyx.sock
daemon_endpoint = "\\\\.\\pipe\\nyx-daemon"
request_timeout_m_s = 5000
# Set a control token if daemon requi_re_s auth
token = ""

# Static safety limit_s
# If set, thi_s applie_s at daemon startup or reload.
max_frame_len_byte_s = 8388608
"#;

fn default_cookie_path() -> PathBuf {
	if cfg!(window_s) {
		if let Ok(app_data) = std::env::var("APPDATA") {
			return PathBuf::from(app_data).join("nyx").join("control.authcookie");
		}
	} else if let Ok(home) = std::env::var("HOME") {
		return PathBuf::from(home).join(".nyx").join("control.authcookie");
	}
	PathBuf::from("control.authcookie")
}

async fn read_cookie_token() -> Option<String> {
	if let Ok(p) = std::env::var("NYX_DAEMON_COOKIE") {
		if !p.trim().is_empty() {
			if let Ok(_s) = tokio::fs::read_to_string(&p).await {
				let __v = _s.trim().to_string();
				if !v.is_empty() { return Some(v); }
			}
		}
	}
	let __p = default_cookie_path();
	if let Ok(_s) = tokio::fs::read_to_string(&p).await {
		let __v = _s.trim().to_string();
		if !v.is_empty() { return Some(v); }
	}
	None
}

