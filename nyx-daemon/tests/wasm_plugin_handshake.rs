#![cfg(feature = "plugin")]

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64_URL;
use base64::Engine;
use hyper::{Body, Client, Method, Request};
use hyper::client::HttpConnector;
use tokio::time::{sleep, Duration};

fn find_free_port() -> u16 {
    std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap()
}

fn locate_daemon_binary() -> String {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_nyx_daemon") { return p; }
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_nyx-daemon") { return p; }
    for (k,v) in std::env::vars() {
        if k.starts_with("CARGO_BIN_EXE_") && k.ends_with("nyx-daemon") || k.ends_with("nyx_daemon") { return v; }
    }
    panic!("CARGO_BIN_EXE for nyx-daemon not found");
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

async fn http_post_json(client: &Client<HttpConnector>, url: &str, json: &serde_json::Value) -> serde_json::Value {
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

async fn http_post_bytes(client: &Client<HttpConnector>, url: &str, body: Vec<u8>, content_type: &str) -> serde_json::Value {
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
            if client.get(uri).await.is_ok() { break; }
        }
        sleep(Duration::from_millis(50)).await;
    }

    // required plugins: empty
    let empty_ids: Vec<u32> = Vec::new();
    let mut cbor = Vec::new();
    ciborium::ser::into_writer(&empty_ids, &mut cbor).unwrap();
    let body = serde_json::json!({"required_cbor_b64": B64_URL.encode(&cbor)});
    let _ = http_post_json(&client, &format!("{}/api/v1/wasm/handshake/required", base), &body).await;

    // start
    let start = http_post_json(&client, &format!("{}/api/v1/wasm/handshake/start", base), &serde_json::json!({})).await;
    assert!(start["started"].as_bool().unwrap_or(false));

    // complete
    let complete = http_post_json(&client, &format!("{}/api/v1/wasm/handshake/complete", base), &serde_json::json!({})).await;
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
            if client.get(uri).await.is_ok() { break; }
        }
        sleep(Duration::from_millis(50)).await;
    }

    // set required ids to [424242]
    let ids = vec![424242u32];
    let mut cbor = Vec::new();
    ciborium::ser::into_writer(&ids, &mut cbor).unwrap();
    let body = serde_json::json!({"required_cbor_b64": B64_URL.encode(&cbor)});
    let _ = http_post_json(&client, &format!("{}/api/v1/wasm/handshake/required", base), &body).await;

    // start
    let _ = http_post_json(&client, &format!("{}/api/v1/wasm/handshake/start", base), &serde_json::json!({})).await;

    // craft peer SETTINGS for required 424242 (format: count:u16, then id:u32, min_major:u16, min_minor:u16, cap:u8, cfg_len:u16)
    let mut peer = Vec::new();
    peer.extend_from_slice(&(1u16.to_be_bytes()));
    peer.extend_from_slice(&(424242u32.to_be_bytes()));
    peer.extend_from_slice(&(1u16.to_be_bytes()));
    peer.extend_from_slice(&(0u16.to_be_bytes()));
    peer.push(2u8); // Required
    peer.extend_from_slice(&(0u16.to_be_bytes())); // no config
    let _ = http_post_bytes(&client, &format!("{}/api/v1/wasm/handshake/process-peer-settings", base), peer, "application/octet-stream").await;

    // complete -> incompatible
    let complete = http_post_json(&client, &format!("{}/api/v1/wasm/handshake/complete", base), &serde_json::json!({})).await;
    assert_eq!(complete["ok"], false);
    assert_eq!(complete["result"], "incompatible");

    let _ = child.kill();
}


