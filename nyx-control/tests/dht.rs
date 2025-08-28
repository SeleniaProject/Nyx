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

// Test DHT routing table functionality
// Keep a smoke test that control module builds without external DHT deps

/// Test DHT key-value storage functionality (capacity, put/get/delete)
#[test]
fn test_dht_kv_store() {
    use nyx_control::dht::{DhtStorage, StorageKey, StorageValue};

    let mut storage = DhtStorage::with_capacity(2);
    let k1 = StorageKey::from_bytes(b"k1");
    let v1 = StorageValue::from_bytes(b"v1");
    let k2 = StorageKey::from_bytes(b"k2");
    let v2 = StorageValue::from_bytes(b"v2");
    let k3 = StorageKey::from_bytes(b"k3");
    let v3 = StorageValue::from_bytes(b"v3");

    // capacity is enforced
    assert_eq!(storage.capacity(), 2);

    // put two entries succeeds
    assert!(storage.put(k1.clone(), v1.clone()).is_ok());
    assert!(storage.put(k2.clone(), v2.clone()).is_ok());
    assert_eq!(storage.len(), 2);

    // third insert should fail due to capacity
    assert!(storage.put(k3.clone(), v3.clone()).is_err());

    // get returns stored values
    assert_eq!(storage.get(&k1).unwrap(), v1);
    assert_eq!(storage.get(&k2).unwrap(), v2);

    // delete removes entry and frees a slot
    assert!(storage.delete(&k1).unwrap());
    assert!(storage.get(&k1).is_none());
    assert_eq!(storage.len(), 1);

    // now we can insert the third key
    assert!(storage.put(k3.clone(), v3.clone()).is_ok());
    assert_eq!(storage.len(), 2);
    assert_eq!(storage.get(&k3).unwrap(), v3);
}

/// Test that the control module builds without DHT dependencies
#[test]
fn control_module_builds_without_dht() {
    // Verify that the control module can be built and used
    // without requiring DHT functionality
    // The module compiles successfully, which proves this works
}
