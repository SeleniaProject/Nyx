#![forbid(unsafe_code)]

use crate::dht::types::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

/// Minimal routing table used by tests and higher layers
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
