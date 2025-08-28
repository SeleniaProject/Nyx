#![forbid(unsafe_code)]

use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::net::SocketAddr;
use std::time::Instant;
use blake3::Hasher;

#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct NodeId(pub [u8; 32]);

impl fmt::Debug for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeId({})", hex::encode(&self.0[..8]))
    }
}

impl NodeId {
    pub fn generate() -> Self {
        let mut id = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut id);
        Self(id)
    }
    pub fn is_valid(&self) -> bool {
        self.0.iter().any(|&b| b != 0)
    }
    pub fn from_pubkey(pubkey: &[u8]) -> Self {
        let mut hasher = Hasher::new();
        hasher.update(pubkey);
        let hash = hasher.finalize();
        let mut id = [0u8; 32];
        id.copy_from_slice(hash.as_bytes());
        Self(id)
    }
    pub fn distance(&self, other: &NodeId) -> Distance {
        let mut x = [0u8; 32];
        for (i, byte) in x.iter_mut().enumerate() { *byte = self.0[i] ^ other.0[i]; }
        Distance(x)
    }
}

#[derive(Clone)]
pub struct NodeInfo {
    pub id: NodeId,
    pub addr: SocketAddr,
    pub last_seen: Instant,
}

impl fmt::Debug for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NodeInfo{{id:{:?}, addr:{}, ..}}", self.id, self.addr)
    }
}

impl NodeInfo {
    pub fn new(id: NodeId, addr: SocketAddr) -> Self { Self { id, addr, last_seen: Instant::now() } }
    pub fn touch(&mut self) { self.last_seen = Instant::now() }
    pub fn node_id(&self) -> &NodeId { &self.id }
    pub fn address(&self) -> SocketAddr { self.addr }
    pub fn is_reachable(&self) -> bool { true }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct StorageKey(pub Vec<u8>);
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageValue(pub Vec<u8>);

impl fmt::Debug for StorageKey { fn fmt(&self, f:&mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Key({})", hex::encode(&self.0)) } }
impl fmt::Debug for StorageValue { fn fmt(&self, f:&mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Val({}B)", self.0.len()) } }

impl StorageKey { pub fn from_bytes(b: &[u8]) -> Self { Self(b.to_vec()) } }
impl StorageValue { pub fn from_bytes(b: &[u8]) -> Self { Self(b.to_vec()) } }

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Distance(pub [u8; 32]);

impl fmt::Debug for Distance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // print the most significant 4 bytes for readability
        write!(f, "Dist({})", hex::encode(&self.0[..4]))
    }
}
