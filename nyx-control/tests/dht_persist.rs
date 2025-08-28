#![forbid(unsafe_code)]

use nyx_control::dht::{DhtConfig, DhtNode, StorageKey, StorageValue};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

fn temp_file_path() -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!("nyx_dht_{ts}.cbor"));
    p
}

#[tokio::test]
async fn dht_snapshot_persist_and_reload() {
    let persist = temp_file_path();
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
    let cfg = DhtConfig {
        bind: addr,
        persist_path: Some(persist.clone()),
        ..Default::default()
    };

    // spawn and store value
    let mut n1 = DhtNode::spawn(cfg).await.unwrap();
    let key = StorageKey::from_bytes(b"persist-key");
    let val = StorageValue::from_bytes(b"persist-val");
    n1.put(key.clone(), val.clone()).await.unwrap();
    n1.persist_snapshot().await.unwrap();

    // spawn new node with same persist path; should load value locally
    let cfg2 = DhtConfig {
        bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
        persist_path: Some(persist.clone()),
        ..Default::default()
    };
    let n2 = DhtNode::spawn(cfg2).await.unwrap();
    let got = n2.get(key.clone()).await.unwrap();
    assert_eq!(got, Some(val));

    // cleanup
    let _ = std::fs::remove_file(persist);
}
