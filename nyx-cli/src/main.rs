#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};
use nyx_sdk::{daemon::DaemonClient, SdkConfig};
use rand::RngCore;
use serde_json::json;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "nyx-cli",
    version,
    about = "Nyx command line interface",
    disable_help_subcommand = false
)]
struct Cli {
    /// Daemon endpoint (override). Default: platform-specific (Unix socket / windows named pipe)
    #[arg(long)]
    endpoint: Option<String>,
    /// Request timeout in milliseconds
    #[arg(long)]
    timeout_ms: Option<u64>,
    /// Auth token
    #[arg(long)]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Show daemon info
    Info,
    /// Reload configuration
    ReloadConfig,
    /// List config versions
    ListVersions,
    /// Update configuration from inline JSON key=val or a JSON file
    UpdateConfig {
        /// Inline key=value pairs (JSON values). Example: log_level="debug"
        #[arg(long, value_parser = parse_kv, num_args = 0..)]
        set: Vec<(String, serde_json::Value)>,
        /// Path to a JSON file with a flat object of settings
        #[arg(long)]
        file: Option<String>,
    },
    /// Rollback configuration to a specific version
    Rollback { version: u64 },
    /// Create a configuration snapshot with an optional description
    Snapshot {
        #[arg(long)]
        description: Option<String>,
    },
    /// Subscribe to daemon events; press Ctrl-C to stop
    Events {
        #[arg(long)]
        types: Vec<String>,
    },
    /// Fetch Prometheus metrics from a URL (http only)
    PrometheusGet { url: String },
    /// Config helpers
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Convenience: set or show the codec frame size cap (bytes)
    FrameLimit {
        /// When provided, sets the cap to this value (1024..=67108864). If omitted, shows current default.
        #[arg(long)]
        set: Option<u64>,
    },
    /// Generate a cookie token file compatible with daemon auth
    GenCookie {
        /// Output path (default: platform-specific). Example: %APPDATA%/nyx/control.authcookie
        #[arg(long)]
        path: Option<String>,
        /// Overwrite if file exists
        #[arg(long)]
        force: bool,
        /// Random token length (bytes), hex-encoded
        #[arg(long, default_value_t = 32)]
        length: usize,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCmd {
    /// Show effective CLI config (resolved from env/files)
    Show,
    /// Write a nyx.toml template with CLI section
    WriteTemplate {
        /// Destination path (default: ./nyx.toml)
        #[arg(long)]
        path: Option<String>,
        /// Overwrite if file exists
        #[arg(long)]
        force: bool,
    },
}

fn parse_kv(s: &str) -> Result<(String, serde_json::Value), String> {
    let (k, v) = s
        .split_once('=')
        .ok_or_else(|| "expected key=value".to_string())?;
    let val = serde_json::from_str::<serde_json::Value>(v)
        .unwrap_or_else(|_| serde_json::Value::String(v.to_string()));
    Ok((k.to_string(), val))
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Start with defaults, then apply auto-discovery (env/config), then override by CLI args
    let (mut cfg, mut token) = auto_discover().await;
    if let Some(ep) = cli.endpoint {
        cfg.daemon_endpoint = ep;
    }
    if let Some(t) = cli.timeout_ms {
        cfg.request_timeout_ms = t;
    }
    if let Some(tok) = cli.token {
        token = Some(tok);
    }
    let mut client = DaemonClient::new(cfg);
    if let Some(tok) = token {
        client = client.with_token(tok);
    }

    let res: anyhow::Result<()> = match cli.command {
        Commands::Info => {
            let v = client.get_info().await;
            print_result(v.map(|j| json!({"ok":true, "data": j})));
            Ok(())
        }
        Commands::ReloadConfig => {
            let v = client.reload_config().await;
            print_result(v.map(|j| json!({"ok":true, "data": j})));
            Ok(())
        }
        Commands::ListVersions => {
            let v = client.list_versions().await;
            print_result(v.map(|j| json!({"ok":true, "data": j})));
            Ok(())
        }
        Commands::UpdateConfig { set, file } => {
            let mut map = serde_json::Map::new();
            for (k, v) in set {
                map.insert(k, v);
            }
            if let Some(path) = file {
                match tokio::fs::read_to_string(path).await {
                    Ok(s) => {
                        match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&s)
                        {
                            Ok(obj) => {
                                for (k, v) in obj {
                                    map.insert(k, v);
                                }
                            }
                            Err(e) => {
                                eprintln!("invalid JSON file: {e}");
                                std::process::exit(2);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("failed to read file: {e}");
                        std::process::exit(2);
                    }
                }
            }
            let v = client.update_config(map).await;
            print_result(v.map(|r| serde_json::to_value(r).unwrap()));
            Ok(())
        }
        Commands::Rollback { version } => {
            let v = client.rollback_config(version).await;
            print_result(v.map(|r| serde_json::to_value(r).unwrap()));
            Ok(())
        }
        Commands::Snapshot { description } => {
            let v = client.create_config_snapshot(description).await;
            print_result(v.map(|j| json!({"ok":true, "data": j})));
            Ok(())
        }
        Commands::Events { types } => {
            match client
                .subscribe_events(if types.is_empty() { None } else { Some(types) })
                .await
            {
                Ok(mut rx) => {
                    let (tx_stop, mut rx_stop) = tokio::sync::mpsc::channel::<()>(1);
                    // Ctrl-C handler (best-effort). Ignore errors if handler already set.
                    let _ = ctrlc::set_handler(move || {
                        let _ = tx_stop.try_send(());
                    });
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
                Err(e) => Err(anyhow::anyhow!(format!("subscribe error: {e}"))),
            }
        }
        Commands::PrometheusGet { url } => match prometheus_client::scrape_text(url).await {
            Ok(body) => {
                print!("{body}");
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(format!("prometheus fetch failed: {e}"))),
        },
        Commands::Config { action } => match action {
            ConfigCmd::Show => {
                let (cfg, tok) = auto_discover().await;
                let out = json!({
                    "daemon_endpoint": cfg.daemon_endpoint,
                    "request_timeout_ms": cfg.request_timeout_ms,
                    "token_present": tok.is_some(),
                });
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
                Ok(())
            }
            ConfigCmd::WriteTemplate { path, force } => {
                let p = path.unwrap_or_else(|| "nyx.toml".to_string());
                let pathbuf = PathBuf::from(&p);
                if pathbuf.exists() && !force {
                    eprintln!(
                        "refusing to overwrite existing file: {} (use --force)",
                        pathbuf.display()
                    );
                    std::process::exit(2);
                }
                let template = TEMPLATE_NYX_TOML;
                if let Err(e) = tokio::fs::write(&pathbuf, template).await {
                    return Err(anyhow::anyhow!(e));
                }
                eprintln!("wrote {}", pathbuf.display());
                Ok(())
            }
        },
        Commands::FrameLimit { set } => {
            if let Some(n) = set {
                // Validate conservative bounds to protect memory usage.
                const MIN: u64 = 1024; // 1 KiB
                const MAX: u64 = 64 * 1024 * 1024; // 64 MiB
                if !(MIN..=MAX).contains(&n) {
                    anyhow::bail!("invalid frame limit: {} (allowed {}..={})", n, MIN, MAX);
                }
                let mut map = serde_json::Map::new();
                map.insert("max_frame_len_bytes".into(), serde_json::Value::from(n));
                let v = client.update_config(map).await;
                print_result(v.map(|r| serde_json::to_value(r).unwrap()));
            } else {
                let current = nyx_stream::FrameCodec::default_limit() as u64;
                println!("{current}");
            }
            Ok(())
        }
        Commands::GenCookie {
            path,
            force,
            length,
        } => {
            let pathbuf = if let Some(p) = path {
                PathBuf::from(p)
            } else {
                default_cookie_path()
            };
            if pathbuf.exists() && !force {
                eprintln!(
                    "refusing to overwrite existing file: {} (use --force)",
                    pathbuf.display()
                );
                std::process::exit(2);
            }
            if length == 0 || length > 1024 {
                anyhow::bail!("invalid length: {length}");
            }
            let mut bytes = vec![0u8; length];
            rand::thread_rng().fill_bytes(&mut bytes);
            let token = hex::encode(bytes);
            if let Some(parent) = pathbuf.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            tokio::fs::write(&pathbuf, &token).await?;
            #[cfg(unix)]
            {
                use std::fs::{metadata, set_permissions};
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = metadata(&pathbuf) {
                    let mut perm = meta.permissions();
                    perm.set_mode(0o600);
                    let _ = set_permissions(&pathbuf, perm);
                }
            }
            eprintln!("wrote {}", pathbuf.display());
            Ok(())
        }
    };

    res
}

fn print_result(res: Result<serde_json::Value, nyx_sdk::Error>) {
    match res {
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
    timeout_ms: Option<u64>,
}

async fn auto_discover() -> (SdkConfig, Option<String>) {
    let mut cfg = SdkConfig::default();
    let mut token: Option<String> = None;

    // 1) Env vars
    if let Ok(ep) = std::env::var("NYX_DAEMON_ENDPOINT") {
        let e = ep.trim();
        if !e.is_empty() {
            cfg.daemon_endpoint = e.to_string();
        }
    }
    if let Ok(t) = std::env::var("NYX_REQUEST_TIMEOUT_MS") {
        if let Ok(v) = t.parse::<u64>() {
            cfg.request_timeout_ms = v;
        }
    }
    // Prefer NYX_CONTROL_TOKEN (charts/values.yaml hint) then NYX_TOKEN
    if let Ok(tok) = std::env::var("NYX_CONTROL_TOKEN") {
        if !tok.trim().is_empty() {
            token = Some(tok.trim().to_string());
        }
    }
    if token.is_none() {
        if let Ok(tok) = std::env::var("NYX_TOKEN") {
            if !tok.trim().is_empty() {
                token = Some(tok.trim().to_string());
            }
        }
    }

    // 2) Cookie file (Tor-style). If present, use it unless env already provided token
    if token.is_none() {
        if let Some(tok) = read_cookie_token().await {
            token = Some(tok);
        }
    }

    // 3) Config file (only fills missing)
    if let Some(file_cfg) = load_cli_file_config().await {
        if cfg.daemon_endpoint == SdkConfig::default_endpoint() {
            if let Some(ep) = file_cfg.endpoint {
                cfg.daemon_endpoint = ep;
            }
        }
        if let Some(ms) = file_cfg.timeout_ms {
            cfg.request_timeout_ms = ms;
        }
        if token.is_none() {
            token = file_cfg.token.filter(|s| !s.trim().is_empty());
        }
    }

    (cfg, token)
}

async fn load_cli_file_config() -> Option<CliFileConfig> {
    // Search order: $NYX_CONFIG -> ./nyx.toml -> platform config dir
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("NYX_CONFIG") {
        candidates.push(PathBuf::from(p));
    }
    candidates.push(PathBuf::from("nyx.toml"));
    // Platform specific default
    if cfg!(windows) {
        if let Ok(app_data) = std::env::var("APPDATA") {
            candidates.push(PathBuf::from(app_data).join("nyx").join("nyx.toml"));
        }
    } else {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            candidates.push(PathBuf::from(xdg).join("nyx").join("nyx.toml"));
        }
        if let Ok(home) = std::env::var("HOME") {
            candidates.push(
                PathBuf::from(home)
                    .join(".config")
                    .join("nyx")
                    .join("nyx.toml"),
            );
        }
    }

    for path in candidates {
        if !path.exists() {
            continue;
        }
        if let Ok(s) = tokio::fs::read_to_string(&path).await {
            if let Some(parsed) = parse_cli_toml(&s) {
                return Some(parsed);
            }
        }
    }
    None
}

fn parse_cli_toml(s: &str) -> Option<CliFileConfig> {
    let v: toml::Value = toml::from_str(s).ok()?;
    let mut out = CliFileConfig::default();
    if let Some(cli) = v.get("cli") {
        if let Some(ep) = cli.get("daemon_endpoint").and_then(|x| x.as_str()) {
            let ep = ep.trim();
            if !ep.is_empty() {
                out.endpoint = Some(ep.to_string());
            }
        }
        if let Some(tok) = cli.get("token").and_then(|x| x.as_str()) {
            let tok = tok.trim();
            if !tok.is_empty() {
                out.token = Some(tok.to_string());
            }
        }
        if let Some(ms) = cli.get("request_timeout_ms").and_then(|x| x.as_integer()) {
            if ms >= 0 {
                out.timeout_ms = Some(ms as u64);
            }
        }
    }
    Some(out)
}

const TEMPLATE_NYX_TOML: &str = r#"# Nyx Configuration (template)

# Service endpoints
[endpoints]
grpc_addr = "127.0.0.1:50051"
prometheus_addr = "127.0.0.1:9090"

# CLI specific configuration
[cli]
# windows named pipe example: \\.\pipe\nyx-daemon
# Unix domain socket example: /tmp/nyx.sock
daemon_endpoint = "\\\\.\\pipe\\nyx-daemon"
request_timeout_ms = 5000
# Set a control token if daemon requires auth
token = ""

# Static safety limits
# If set, this applies at daemon startup or reload.
max_frame_len_bytes = 8388608
"#;

fn default_cookie_path() -> PathBuf {
    if cfg!(windows) {
        if let Ok(app_data) = std::env::var("APPDATA") {
            return PathBuf::from(app_data)
                .join("nyx")
                .join("control.authcookie");
        }
    } else if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".nyx").join("control.authcookie");
    }
    PathBuf::from("control.authcookie")
}

async fn read_cookie_token() -> Option<String> {
    if let Ok(p) = std::env::var("NYX_DAEMON_COOKIE") {
        if !p.trim().is_empty() {
            if let Ok(s) = tokio::fs::read_to_string(&p).await {
                let v = s.trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    let p = default_cookie_path();
    if let Ok(s) = tokio::fs::read_to_string(&p).await {
        let v = s.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    None
}
