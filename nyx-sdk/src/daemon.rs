#![forbid(unsafe_code)]

use crate::{
    config::SdkConfig,
    error::{Error, Result},
    events::Event,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::time::{timeout, Duration, Instant};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::broadcast,
};

#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request<'a> {
    GetInfo,
    ReloadConfig,
    UpdateConfig {
        settings: &'a serde_json::Map<String, serde_json::Value>,
    },
    SubscribeEvents {
        types: Option<Vec<String>>,
    },
    ListConfigVersions,
    RollbackConfig {
        version: u64,
    },
    CreateConfigSnapshot {
        description: Option<String>,
    },
}

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<&'a str>,
    #[serde(flatten)]
    req: Request<'a>,
}

#[derive(Debug, Deserialize)]
struct RpcResponseValue {
    ok: bool,
    code: u16,
    id: Option<String>,
    #[serde(default)]
    _data: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
}

/// Mirror of daemon's ConfigResponse for caller convenience
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigResponse {
    pub success: bool,
    pub message: String,
    #[serde(default)]
    pub validation_errors: Vec<String>,
}

pub struct DaemonClient {
    cfg: SdkConfig,
    auth_token: Option<String>,
}

impl DaemonClient {
    pub fn new(cfg: SdkConfig) -> Self {
        Self {
            cfg,
            auth_token: None,
        }
    }
    /// Set an auth token; whitespace-only tokens are treated as absent.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        let t = token.into();
        let t = t.trim();
        if t.is_empty() {
            self.auth_token = None;
        } else {
            self.auth_token = Some(t.to_string());
        }
        self
    }

    /// Construct a client and auto-discover token from env/cookie (non-blocking config stays as provided).
    pub async fn new_with_auto_token(cfg: SdkConfig) -> Self {
        let tok = auto_discover_token().await;
        Self {
            cfg,
            auth_token: tok,
        }
    }

    /// Try to auto-discover an auth token from env/cookie and set it. Whitespace is ignored.
    pub async fn with_auto_token(mut self) -> Self {
        self.auth_token = auto_discover_token().await;
        self
    }

    pub async fn get_info(&self) -> Result<serde_json::Value> {
        self.rpc_json(&RpcRequest {
            id: None,
            auth: self.auth_token.as_deref(),
            req: Request::GetInfo,
        })
        .await
    }

    pub async fn reload_config(&self) -> Result<serde_json::Value> {
        self.rpc_json(&RpcRequest {
            id: None,
            auth: self.auth_token.as_deref(),
            req: Request::ReloadConfig,
        })
        .await
    }

    pub async fn list_versions(&self) -> Result<serde_json::Value> {
        self.rpc_json(&RpcRequest {
            id: None,
            auth: self.auth_token.as_deref(),
            req: Request::ListConfigVersions,
        })
        .await
    }

    /// Update daemon configuration with the provided settings
    /// 
    /// # Arguments
    /// * `settings` - Key-value pairs of configuration options to update
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Failed to serialize settings to JSON
    /// - Communication with daemon fails
    /// - Daemon rejects the configuration update
    pub async fn update_config(
        &self,
        settings: serde_json::Map<String, serde_json::Value>,
    ) -> Result<ConfigResponse> {
        self.rpc_json::<ConfigResponse>(&RpcRequest {
            id: None,
            auth: self.auth_token.as_deref(),
            req: Request::UpdateConfig {
                settings: &settings,
            },
        })
        .await
    }

    /// Rollback daemon configuration to a previous version
    /// 
    /// # Arguments
    /// * `version` - The configuration version to rollback to
    /// 
    /// # Errors
    /// Returns an error if:
    /// - The specified version does not exist
    /// - Communication with daemon fails
    /// - Rollback operation fails on the daemon side
    pub async fn rollback_config(&self, version: u64) -> Result<ConfigResponse> {
        self.rpc_json::<ConfigResponse>(&RpcRequest {
            id: None,
            auth: self.auth_token.as_deref(),
            req: Request::RollbackConfig { version },
        })
        .await
    }

    /// Create a snapshot of the current daemon configuration
    /// 
    /// # Arguments
    /// * `description` - Optional description for the snapshot
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Communication with daemon fails
    /// - Snapshot creation fails on the daemon side
    /// - Serialization of configuration fails
    pub async fn create_config_snapshot(
        &self,
        description: Option<String>,
    ) -> Result<serde_json::Value> {
        self.rpc_json(&RpcRequest {
            id: None,
            auth: self.auth_token.as_deref(),
            req: Request::CreateConfigSnapshot { description },
        })
        .await
    }

    /// Subscribe to daemon events of specific types
    /// 
    /// # Arguments
    /// * `types` - Optional list of event types to subscribe to. If None, subscribes to all events
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Communication with daemon fails
    /// - Event subscription setup fails
    /// - Invalid event types are specified
    pub async fn subscribe_events(
        &self,
        types: Option<Vec<String>>,
    ) -> Result<broadcast::Receiver<Event>> {
        let mut stream = connect(&self.cfg).await?;
        let req = RpcRequest {
            id: None,
            auth: self.auth_token.as_deref(),
            req: Request::SubscribeEvents { types },
        };
        let line = serde_json::to_string(&req).map_err(|e| Error::Protocol(e.to_string()))? + "\n";
        timeout(
            Duration::from_millis(self.cfg.request_timeout_ms),
            stream.write_all(line.as_bytes()),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        timeout(
            Duration::from_millis(self.cfg.request_timeout_ms),
            stream.flush(),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        // Drop first response line
        let mut buf = Vec::with_capacity(1024);
        read_one_line_with_timeout(&mut stream, &mut buf, self.cfg.request_timeout_ms).await?;
        // Now events stream follows line-delimited JSON
        let (tx, rx) = broadcast::channel(128);
        tokio::spawn(async move {
            let mut stream_inner = stream; // move into task
            let mut tmp = Vec::with_capacity(1024);
            loop {
                tmp.clear();
                if let Err(e) = read_one_line(&mut stream_inner, &mut tmp).await {
                    let _ = tx.send(Event {
                        event_type: "system".into(),
                        detail: format!("events_stream_closed:{e}"),
                    });
                    break;
                }
                if tmp.is_empty() {
                    continue;
                }
                match serde_json::from_slice::<Event>(&tmp) {
                    Ok(ev) => {
                        if tx.send(ev).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if tx
                            .send(Event {
                                event_type: "system".into(),
                                detail: format!("events_decode_error:{e}"),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            }
        });
        Ok(rx)
    }

    async fn rpc_json<T: for<'de> Deserialize<'de>>(&self, req: &RpcRequest<'_>) -> Result<T> {
        let mut stream = connect(&self.cfg).await?;
        let line = serde_json::to_string(req).map_err(|e| Error::Protocol(e.to_string()))? + "\n";
        timeout(
            Duration::from_millis(self.cfg.request_timeout_ms),
            stream.write_all(line.as_bytes()),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        timeout(
            Duration::from_millis(self.cfg.request_timeout_ms),
            stream.flush(),
        )
        .await
        .map_err(|_| Error::Timeout)??;
        let mut buf = Vec::with_capacity(1024);
        read_one_line_with_timeout(&mut stream, &mut buf, self.cfg.request_timeout_ms).await?;
        let resp: RpcResponseValue =
            serde_json::from_slice(&buf).map_err(|e| Error::Protocol(e.to_string()))?;
        if resp.ok {
            // Optionally validate success code and note id for diagnostics
            let _resp_id = resp.id.as_deref();
            let _resp_code = resp.code;
            let v = resp._data.ok_or_else(|| Error::protocol("missing _data"))?;
            let t = serde_json::from_value(v).map_err(|e| Error::Protocol(e.to_string()))?;
            Ok(t)
        } else {
            let code = resp.code;
            let id_suffix = resp
                .id
                .as_deref()
                .map(|s| format!(" id={s}"))
                .unwrap_or_default();
            let msg = resp.error.unwrap_or_else(|| "unknown error".into());
            Err(Error::protocol(format!("{msg} (code={code}){id_suffix}")))
        }
    }
}

// ---------- token auto-discovery (env -> cookie) ----------

async fn auto_discover_token() -> Option<String> {
    // Prefer NYX_CONTROL_TOKEN (charts hint) then NYX_TOKEN
    if let Ok(tok) = std::env::var("NYX_CONTROL_TOKEN") {
        let t = tok.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    if let Ok(tok) = std::env::var("NYX_TOKEN") {
        let t = tok.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    // Cookie (Tor-style)
    read_cookie_token().await
}

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

async fn read_one_line<R: AsyncRead + Unpin>(reader: &mut R, out: &mut Vec<u8>) -> Result<()> {
    let mut tmp = [0u8; 256];
    out.clear();
    loop {
        let n = reader
            .read(&mut tmp)
            .await
            .map_err(|e| Error::Stream(e.to_string()))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&tmp[..n]);
        if out.contains(&b'\n') {
            break;
        }
        if out.len() > 64 * 1024 {
            return Err(Error::protocol("response too large"));
        }
    }
    if let Some(pos) = memchr::memchr(b'\n', out) {
        out.truncate(pos);
    }
    // Trim a trailing CR if present (handle CRLF)
    if out.last().copied() == Some(b'\r') {
        out.pop();
    }
    Ok(())
}

async fn read_one_line_with_timeout<R: AsyncRead + Unpin>(
    reader: &mut R,
    out: &mut Vec<u8>,
    timeout_ms: u64,
) -> Result<()> {
    let deadline = Duration::from_millis(timeout_ms);
    let start = Instant::now();
    out.clear();
    let mut buf = [0u8; 256];
    loop {
        let remain = deadline.saturating_sub(start.elapsed());
        if remain.is_zero() {
            return Err(Error::Timeout);
        }
        let n = timeout(remain, reader.read(&mut buf))
            .await
            .map_err(|_| Error::Timeout)?
            .map_err(|e| Error::Stream(e.to_string()))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        if out.contains(&b'\n') {
            break;
        }
        if out.len() > 64 * 1024 {
            return Err(Error::protocol("response too large"));
        }
    }
    if let Some(pos) = memchr::memchr(b'\n', out) {
        out.truncate(pos);
    }
    if out.last().copied() == Some(b'\r') {
        out.pop();
    }
    Ok(())
}

#[cfg(unix)]
async fn connect(cfg: &SdkConfig) -> Result<tokio::net::UnixStream> {
    let stream = tokio::net::UnixStream::connect(cfg.daemon_endpoint.clone()).await?;
    Ok(stream)
}

#[cfg(windows)]
async fn connect(cfg: &SdkConfig) -> Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use tokio::net::windows::named_pipe::ClientOptions;
    let client = ClientOptions::new().open(cfg.daemon_endpoint.clone())?;
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{duplex, AsyncWriteExt};

    #[test]
    fn request_serialization_shapes() {
        // Ensure enum tagging matches daemon expectations
        let req = RpcRequest {
            id: Some("x"),
            auth: Some("t"),
            req: Request::GetInfo,
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"op\":\"get_info\""));

        let mut map = serde_json::Map::new();
        map.insert("log_level".into(), json!("debug"));
        let req = RpcRequest {
            id: None,
            auth: Some("t"),
            req: Request::UpdateConfig { settings: &map },
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"op\":\"update_config\""));
        assert!(s.contains("\"log_level\":"));
    }

    #[tokio::test]
    async fn read_one_line_handles_newline_and_truncation() -> Result<()> {
        let (mut a, mut b) = duplex(64);
        // Write two lines; reader should only read first line
        tokio::spawn(async move {
            let _ = b.write_all(b"{\"ok\":true}\n{\"ok\":false}\n").await;
        });
        let mut buf = Vec::new();
        read_one_line(&mut a, &mut buf).await?;
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "{\"ok\":true}");
        Ok(())
    }

    #[tokio::test]
    async fn read_one_line_trims_crlf() -> Result<()> {
        let (mut a, mut b) = duplex(64);
        tokio::spawn(async move {
            let _ = b.write_all(b"{\"ok\":true}\r\n").await;
        });
        let mut buf = Vec::new();
        read_one_line(&mut a, &mut buf).await?;
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "{\"ok\":true}");
        Ok(())
    }

    #[test]
    fn with_token_ignores_empty_whitespace() {
        let c = DaemonClient::new(SdkConfig::default()).with_token("   \t\n");
        assert!(c.auth_token.is_none());
        let c = DaemonClient::new(SdkConfig::default()).with_token(" abc ");
        assert_eq!(c.auth_token.as_deref(), Some("abc"));
    }

    #[tokio::test]
    async fn auto_discover_prefers_env_then_cookie_and_trims() -> Result<()> {
        // Ensure env is clear
        std::env::remove_var("NYX_CONTROL_TOKEN");
        std::env::remove_var("NYX_TOKEN");
        std::env::remove_var("NYX_DAEMON_COOKIE");

        // Cookie fallback
        let dir = tempfile::tempdir().unwrap();
        let cookie_path = dir.path().join("control.authcookie");
        tokio::fs::write(&cookie_path, "  cookietoken  ")
            .await
            .unwrap();
        std::env::set_var("NYX_DAEMON_COOKIE", &cookie_path);
        let t = auto_discover_token().await;
        assert_eq!(t.as_deref(), Some("cookietoken"));

        // Env overrides cookie
        std::env::set_var("NYX_TOKEN", "  envtok  ");
        let t2 = auto_discover_token().await;
        assert_eq!(t2.as_deref(), Some("envtok"));

        std::env::remove_var("NYX_TOKEN");
        std::env::remove_var("NYX_DAEMON_COOKIE");
        Ok(())
    }
}
