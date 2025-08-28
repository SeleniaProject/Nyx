#![forbid(unsafe_code)]

use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Minimal Node identifier (32 bytes)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; 32]);

impl NodeId {
    pub fn generate() -> Self {
        let mut id = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut id);
        Self(id)
    }
    pub fn is_valid(&self) -> bool {
        // Consider all-nonzero as a simple validity check for now
        self.0.iter().any(|&b| b != 0)
    }
}

/// Basic node info with reachable socket address
#[derive(Debug, Clone)]
pub struct NodeInfo {
    id: NodeId,
    addr: SocketAddr,
    last_seen: Instant,
}

impl NodeInfo {
    pub fn new(id: NodeId, addr: SocketAddr) -> Self {
        Self { id, addr, last_seen: Instant::now() }
    }
    pub fn node_id(&self) -> &NodeId { &self.id }
    pub fn address(&self) -> SocketAddr { self.addr }
    pub fn touch(&mut self) { self.last_seen = Instant::now(); }
    pub fn is_reachable(&self) -> bool { true }
}

/// Simple route entry for a destination node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    pub dest: NodeId,
    pub hops: u16,
    pub est_latency_ms: u32,
}

impl RouteEntry {
    pub fn new(dest: NodeId, hops: u16, est_latency_ms: u32) -> Self {
        Self { dest, hops, est_latency_ms }
    }
}

/// Minimal routing table
#[derive(Debug, Default)]
pub struct RoutingTable {
    routes: HashMap<NodeId, RouteEntry>,
}

impl RoutingTable {
    pub fn new() -> Self { Self { routes: HashMap::new() } }
    pub fn add_route(&mut self, r: RouteEntry) { self.routes.insert(r.dest.clone(), r); }
    pub fn find_route(&self, dest: &NodeId) -> Option<&RouteEntry> { self.routes.get(dest) }
    pub fn remove_route(&mut self, dest: &NodeId) -> bool { self.routes.remove(dest).is_some() }
}

/// Storage key/value newtypes
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorageKey(Vec<u8>);
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageValue(Vec<u8>);

impl StorageKey {
    pub fn from_bytes(b: &[u8]) -> Self { Self(b.to_vec()) }
}
impl StorageValue {
    pub fn from_bytes(b: &[u8]) -> Self { Self(b.to_vec()) }
}

/// In-memory DHT-like storage (single-node)
#[derive(Debug, Default)]
pub struct DhtStorage {
    map: HashMap<StorageKey, (StorageValue, Instant)>,
    capacity: usize,
    ttl: Duration,
}

impl DhtStorage {
    pub fn new() -> Self { Self { map: HashMap::new(), capacity: 1024, ttl: Duration::from_secs(3600) } }
    pub fn with_capacity(cap: usize) -> Self { Self { capacity: cap, ..Self::new() } }
    pub fn put(&mut self, k: StorageKey, v: StorageValue) -> Result<(), &'static str> {
        if self.map.len() >= self.capacity { return Err("capacity exceeded"); }
        self.map.insert(k, (v, Instant::now()));
        Ok(())
    }
    pub fn get(&mut self, k: &StorageKey) -> Option<StorageValue> {
        self.gc();
        self.map.get(k).map(|(v, _)| v.clone())
    }
    pub fn delete(&mut self, k: &StorageKey) -> Result<bool, &'static str> { Ok(self.map.remove(k).is_some()) }
    pub fn capacity(&self) -> usize { self.capacity }
    pub fn len(&self) -> usize { self.map.len() }
    fn gc(&mut self) {
        let ttl = self.ttl;
        self.map.retain(|_, (_, t)| t.elapsed() < ttl);
    }
}
