#![forbid(unsafe_code)]

use crate::dht::types::{NodeId, NodeInfo};

pub const K_PARAM: usize = 20; // typical k-bucket size

#[derive(Debug, Default)]
pub struct KBuckets {
    // single list sorted by distance to our node id (simplified)
    local: NodeId,
    peers: Vec<NodeInfo>,
}

impl KBuckets {
    pub fn new(local: NodeId) -> Self { Self { local, peers: Vec::new() } }

    pub fn upsert(&mut self, info: NodeInfo) {
        // Remove existing
        self.peers.retain(|p| p.id != info.id);
        // Insert and keep sorted by distance
        self.peers.push(info);
        let local = self.local.clone();
        self.peers.sort_by(|a, b| {
            let da = local.distance(&a.id);
            let db = local.distance(&b.id);
            da.cmp(&db)
        });
        if self.peers.len() > K_PARAM {
            self.peers.truncate(K_PARAM);
        }
    }

    pub fn nearest(&self, target: &NodeId, limit: usize) -> Vec<NodeInfo> {
        let mut v = self.peers.clone();
        v.sort_by(|a, b| {
            let da = target.distance(&a.id);
            let db = target.distance(&b.id);
            da.cmp(&db)
        });
        v.truncate(limit);
        v
    }

    pub fn len(&self) -> usize { self.peers.len() }
    pub fn is_empty(&self) -> bool { self.peers.is_empty() }
}
