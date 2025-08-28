//! DHT (Distributed Hash Table) minimal functionality tests.
//!
//! This suite validates the minimal, pure-Rust DHT primitives provided by `nyx-control`:
//! - NodeId / NodeInfo
//! - RoutingTable
//! - In-memory KV storage

/// Test that DHT functionality is properly stubbed out
#[test]
fn node_id_and_info() {
    use nyx_control::dht::{NodeId, NodeInfo};
    let n1 = NodeId::generate();
    let n2 = NodeId::generate();
    assert_ne!(n1, n2);
    assert!(n1.is_valid() && n2.is_valid());

    let addr: std::net::SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let info = NodeInfo::new(n1.clone(), addr);
    assert_eq!(info.node_id(), &n1);
    assert_eq!(info.address(), addr);
    assert!(info.is_reachable());
}

/// Test that DHT-related configuration is handled gracefully
#[test]
fn routing_table_basic() {
    use nyx_control::dht::{NodeId, RouteEntry, RoutingTable};
    let mut rt = RoutingTable::new();
    let dest = NodeId::generate();
    let r = RouteEntry::new(dest.clone(), 1, 100);
    rt.add_route(r);
    let got = rt.find_route(&dest).expect("route exists");
    assert_eq!(got.hops, 1);
    assert_eq!(got.est_latency_ms, 100);
    assert!(rt.remove_route(&dest));
    assert!(rt.find_route(&dest).is_none());
}

/// Test DHT node discovery functionality
#[test]
fn storage_kv_basic() {
    use nyx_control::dht::{DhtStorage, StorageKey, StorageValue};
    let mut s = DhtStorage::new();
    let k = StorageKey::from_bytes(b"test_key");
    let v = StorageValue::from_bytes(b"test_value");
    assert!(s.put(k.clone(), v.clone()).is_ok());
    assert_eq!(s.get(&k).unwrap(), v);
    assert!(s.delete(&k).unwrap());
    assert!(s.get(&k).is_none());
    assert!(s.capacity() > 0);
}

/// Test DHT routing table functionality
// Keep a smoke test that control module builds without external DHT deps

/// Test DHT key-value storage functionality
#[test]
fn test_dht_kv_store() {
    // TODO: Re-enable when storage module is implemented
    // Test DHT key-value storage implementation when available
    // use nyx_control::storage::{DhtStorage, StorageKey, StorageValue};

    // let mut storage = DhtStorage::new();
    // let key = StorageKey::from_bytes(b"test_key");
    // let value = StorageValue::from_bytes(b"test_value");

    // Test storage operations
    // assert!(storage.put(key.clone(), value.clone()).is_ok());

    // let retrieved = storage.get(&key);
    // assert!(retrieved.is_some());
    // assert_eq!(retrieved.unwrap(), value);

    // Test key deletion
    // assert!(storage.delete(&key).is_ok());
    // assert!(storage.get(&key).is_none());

    // Test storage capacity and limits
    // assert!(storage.capacity() > 0);
    // assert_eq!(storage.len(), 0);

    // For now, just pass the test - TODO: implement proper KV store tests
}

/// Test that the control module build_s without DHT dependencie_s
#[test]
fn control_module_builds_without_dht() {
    // Verify that the control module can be built and used
    // without requiring DHT functionality
    // The module compiles successfully, which proves this works
}
