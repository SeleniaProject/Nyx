#![cfg(feature = "plugin")]

use base64::{engine::general_purpose, Engine as _};
use hyper::client::HttpConnector;
use hyper::{Body, Client, Method, Request};
use tokio::time::{sleep, Duration};

fn find_free_port() -> u16 {
    std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap()
}

fn locate_daemon_binary() -> String {
    // 1) Standard Cargo-provided env vars
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_nyx_daemon") {
        return p;
    }
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_nyx-daemon") {
        return p;
    }
    for (k, v) in std::env::vars() {
        // Accept both hyphenated and underscored suffixes
        if k.starts_with("CARGO_BIN_EXE_")
            && (k.ends_with("nyx-daemon") || k.ends_with("nyx_daemon"))
        {
            return v;
        }
    }

    // 2) Fallback: search under target/(debug|test|release)/ for nyx-daemon executable
    use std::path::{Path, PathBuf};
    let target_root_default = "target".to_string();
    let mut candidate_target_roots: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        candidate_target_roots.push(PathBuf::from(dir));
    }
    // Current crate dir
    candidate_target_roots.push(PathBuf::from(&target_root_default));
    // Workspace root dir (walk up)
    if let Ok(cwd) = std::env::current_dir() {
        let mut up = cwd.clone();
        for _ in 0..4 {
            if let Some(parent) = up.parent() {
                let mut t = parent.to_path_buf();
                t.push("target");
                candidate_target_roots.push(t);
                up = parent.to_path_buf();
            }
        }
    }
    let profiles = ["debug", "test", "release"]; // typical Cargo output directories
    let names = ["nyx-daemon", "nyx_daemon"]; // name variants
    let exe_suffix = if cfg!(windows) { ".exe" } else { "" };

    for root in &candidate_target_roots {
        for prof in &profiles {
            for name in &names {
                let mut p = root.clone();
                p.push(prof);
                p.push(format!("{}{}", name, exe_suffix));
                if p.exists() {
                    return p.to_string_lossy().into_owned();
                }
            }
        }
    }

    // 3) Fallback: scan target/(debug|test|release) for files starting with nyx-daemon*
    for root in &candidate_target_roots {
        for prof in &profiles {
            let mut dir = root.clone();
            dir.push(prof);
            if let Ok(rd) = std::fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let path = e.path();
                    if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                        let is_candidate =
                            fname.starts_with("nyx-daemon") || fname.starts_with("nyx_daemon");
                        let exe_ok = if cfg!(windows) {
                            fname.ends_with(".exe")
                        } else {
                            true
                        };
                        if is_candidate && exe_ok && path.is_file() {
                            return path.to_string_lossy().into_owned();
                        }
                    }
                }
            }
        }
    }

    panic!("Could not locate nyx-daemon binary. Ensure the bin target is built.");
}

async fn spawn_daemon_on(port: u16) -> std::process::Child {
    let exe = locate_daemon_binary();
    let mut cmd = std::process::Command::new(exe);
    cmd.env("NYX_HTTP_ADDR", format!("127.0.0.1:{}", port));
    cmd.env_remove("RUST_LOG");
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    cmd.spawn().expect("spawn daemon")
}

async fn http_get_json(client: &Client<HttpConnector>, url: &str) -> serde_json::Value {
    let uri: hyper::Uri = url.parse().unwrap();
    let resp = client.get(uri).await.unwrap();
    assert!(resp.status().is_success());
    let bytes = hyper::body::to_bytes(resp.into_body()).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn http_post_json(
    client: &Client<HttpConnector>,
    url: &str,
    json: &serde_json::Value,
) -> serde_json::Value {
    let uri: hyper::Uri = url.parse().unwrap();
    let body = serde_json::to_vec(json).unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    let resp = client.request(req).await.unwrap();
    assert!(resp.status().is_success());
    let bytes = hyper::body::to_bytes(resp.into_body()).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn http_post_bytes(
    client: &Client<HttpConnector>,
    url: &str,
    body: Vec<u8>,
    content_type: &str,
) -> serde_json::Value {
    let uri: hyper::Uri = url.parse().unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header("content-type", content_type)
        .body(Body::from(body))
        .unwrap();
    let resp = client.request(req).await.unwrap();
    assert!(resp.status().is_success());
    let bytes = hyper::body::to_bytes(resp.into_body()).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn wasm_handshake_success_autopilot_like() {
    let port = find_free_port();
    let mut child = spawn_daemon_on(port).await;
    let base = format!("http://127.0.0.1:{}", port);
    let client: Client<HttpConnector> = Client::new();

    // wait for readiness
    for _ in 0..50u8 {
        if let Ok(uri) = format!("{}/api/v1/info", base).parse::<hyper::Uri>() {
            if client.get(uri).await.is_ok() {
                break;
            }
        }
        sleep(Duration::from_millis(50)).await;
    }

    // required plugins: empty
    let empty_ids: Vec<u32> = Vec::new();
    let mut cbor = Vec::new();
    ciborium::ser::into_writer(&empty_ids, &mut cbor).unwrap();
    let body =
        serde_json::json!({"required_cbor_b64": general_purpose::URL_SAFE_NO_PAD.encode(&cbor)});
    let _ = http_post_json(
        &client,
        &format!("{}/api/v1/wasm/handshake/required", base),
        &body,
    )
    .await;

    // start
    let start = http_post_json(
        &client,
        &format!("{}/api/v1/wasm/handshake/start", base),
        &serde_json::json!({}),
    )
    .await;
    assert!(start["started"].as_bool().unwrap_or(false));

    // complete
    let complete = http_post_json(
        &client,
        &format!("{}/api/v1/wasm/handshake/complete", base),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(complete["ok"], true);

    let _ = child.kill();
}

#[tokio::test]
async fn wasm_handshake_fail_on_unsupported_required() {
    let port = find_free_port();
    let mut child = spawn_daemon_on(port).await;
    let base = format!("http://127.0.0.1:{}", port);
    let client: Client<HttpConnector> = Client::new();

    // wait for readiness
    for _ in 0..50u8 {
        if let Ok(uri) = format!("{}/api/v1/info", base).parse::<hyper::Uri>() {
            if client.get(uri).await.is_ok() {
                break;
            }
        }
        sleep(Duration::from_millis(50)).await;
    }

    // set required ids to [424242]
    let ids = vec![424242u32];
    let mut cbor = Vec::new();
    ciborium::ser::into_writer(&ids, &mut cbor).unwrap();
    let body =
        serde_json::json!({"required_cbor_b64": general_purpose::URL_SAFE_NO_PAD.encode(&cbor)});
    let _ = http_post_json(
        &client,
        &format!("{}/api/v1/wasm/handshake/required", base),
        &body,
    )
    .await;

    // start
    let _ = http_post_json(
        &client,
        &format!("{}/api/v1/wasm/handshake/start", base),
        &serde_json::json!({}),
    )
    .await;

    // craft peer SETTINGS for required 424242 (format: count:u16, then id:u32, min_major:u16, min_minor:u16, cap:u8, cfg_len:u16)
    let mut peer = Vec::new();
    peer.extend_from_slice(&(1u16.to_be_bytes()));
    peer.extend_from_slice(&(424242u32.to_be_bytes()));
    peer.extend_from_slice(&(1u16.to_be_bytes()));
    peer.extend_from_slice(&(0u16.to_be_bytes()));
    peer.push(2u8); // Required
    peer.extend_from_slice(&(0u16.to_be_bytes())); // no config
    let _ = http_post_bytes(
        &client,
        &format!("{}/api/v1/wasm/handshake/process-peer-settings", base),
        peer,
        "application/octet-stream",
    )
    .await;

    // complete -> incompatible
    let complete = http_post_json(
        &client,
        &format!("{}/api/v1/wasm/handshake/complete", base),
        &serde_json::json!({}),
    )
    .await;
    assert_eq!(complete["ok"], false);
    assert_eq!(complete["result"], "incompatible");

    // CLOSE(0x07) explicit: build unsupported cap close and verify daemon decodes it
    let close_payload = nyx_stream::build_close_unsupported_cap(424242);
    let decoded = http_post_bytes(
        &client,
        &format!("{}/api/v1/wasm/close", base),
        close_payload,
        "application/nyx-close",
    )
    .await;
    assert_eq!(decoded["accepted"], true);
    assert_eq!(decoded["code"], 7);

    let _ = child.kill();
}
