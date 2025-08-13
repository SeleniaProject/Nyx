#![forbid(unsafe_code)]

#[cfg(feature = "dht")]
// use futures::StreamExt; // Uncomment when streaming DHT events
// #[cfg(feature = "dht")]
// use libp2p::{identity, kad::{store::MemoryStore, Kademlia, Quorum, record::{Key, Record}}, swarm::SwarmEvent, PeerId, Multiaddr};
use tokio::sync::{mpsc, oneshot};
use std::collections::HashMap;
use nyx_core::{NyxConfig};
use tracing::warn;

// Pure Rust replacement for libp2p::Multiaddr
pub type Multiaddr = String;

/// Aggregates control-plane handles (DHT + optional Push service).
#[derive(Clone)]
pub struct ControlManager {
    pub dht: DhtHandle,
    pub push: Option<PushHandle>,
}

/// Initialize control plane based on runtime configuration.
///
/// * Spawns Kademlia DHT node (feature gated)
/// * Starts background push service when `cfg.push` is provided
///
/// This async helper is intended to be invoked by the Nyx daemon at startup.
pub async fn init_control(cfg: &NyxConfig) -> ControlManager {
    let dht = spawn_dht().await;
    let push = cfg.push.clone().map(spawn_push_service);

    // If rendezvous endpoint configured in env NYX_RENDEZVOUS_URL
    if let Ok(_url) = std::env::var("NYX_RENDEZVOUS_URL") {
        // Use node_id from config or fallback random.
        let bytes = cfg.node_id.as_ref()
            .and_then(|s| hex::decode(s).ok())
            .unwrap_or(vec![0u8;32]);
        let mut id = [0u8;32];
        id.copy_from_slice(&bytes[..32]);
        // let client = rendezvous::RendezvousClient::new(url, id, dht.listen_addr().clone(), dht.clone());
        // client.spawn();
        warn!("DHT disabled, rendezvous client not started");
    }
    ControlManager { dht, push }
}

pub mod settings;
pub mod probe;
pub mod push;
pub use push::{PushHandle, spawn_push_service};
pub mod rendezvous;
pub use rendezvous::RendezvousClient as RendezvousService;
mod settings_sync;

/// Control messages for the DHT event loop.
#[cfg(feature = "dht")]
pub enum DhtCmd {
    Put { key: String, value: Vec<u8> },
    Get { key: String, resp: oneshot::Sender<Option<Vec<u8>>> },
    Bootstrap(Multiaddr),
}

#[cfg(not(feature = "dht"))]
pub enum DhtCmd { 
    /// Store key-value pair in DHT (stub implementation)
    Put { key: String, value: Vec<u8> },
    /// Retrieve value by key from DHT (stub implementation)
    Get { key: String, resp: oneshot::Sender<Option<Vec<u8>>> },
    /// Bootstrap connection to DHT network (stub implementation)
    Bootstrap(String), // Using String instead of Multiaddr for simplicity
    /// Announce presence in DHT network
    Announce { node_id: String },
    /// Find peers near a given key
    FindPeers { key: String, resp: oneshot::Sender<Vec<String>> },
    /// Ping a specific peer
    Ping { peer_id: String, resp: oneshot::Sender<bool> },
}

/// Handle to interact with running DHT node.
#[derive(Clone)]
pub struct DhtHandle {
    #[cfg(feature = "dht")]
    tx: mpsc::Sender<DhtCmd>,
    #[cfg(feature = "dht")]
    listen_addr: Multiaddr,
}

impl DhtHandle {
    #[cfg(feature = "dht")]
    pub async fn put(&self, key: &str, val: Vec<u8>) {
        let _ = self.tx.send(DhtCmd::Put { key: key.to_string(), value: val }).await;
    }

    #[cfg(feature = "dht")]
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let (resp_tx, resp_rx) = oneshot::channel();
        let _ = self.tx.send(DhtCmd::Get { key: key.to_string(), resp: resp_tx }).await;
        resp_rx.await.ok().flatten()
    }

    #[cfg(feature = "dht")]
    pub async fn add_bootstrap(&self, addr: Multiaddr) {
        let _ = self.tx.send(DhtCmd::Bootstrap(addr)).await;
    }

    // Return the primary listen address when DHT is enabled.
    #[cfg(feature = "dht")]
    #[must_use]
    pub fn listen_addr(&self) -> &Multiaddr {
        &self.listen_addr
    }

    #[cfg(not(feature = "dht"))]
    #[must_use]
    pub fn listen_addr(&self) {}

    #[cfg(not(feature = "dht"))]
    pub async fn put(&self, _key: &str, _val: Vec<u8>) {}

    #[cfg(not(feature = "dht"))]
    pub async fn get(&self, _key: &str) -> Option<Vec<u8>> { None }

    #[cfg(not(feature = "dht"))]
    pub async fn add_bootstrap(&self, _addr: ()) {}
}

/// Spawn DHT node; returns handle to interact.
#[cfg(feature = "dht")]
pub async fn spawn_dht() -> DhtHandle {
    // Pure Rust DHT implementation - shared in-memory store to simulate network propagation
    use std::sync::Mutex;
    use once_cell::sync::Lazy;
    static GLOBAL_DHT: Lazy<Mutex<HashMap<String, Vec<u8>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

    let (tx, mut rx) = mpsc::channel::<DhtCmd>(32);
    let listen_addr: Multiaddr = "127.0.0.1:0".to_string();

    // Background task to handle DHT commands
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                DhtCmd::Put { key, value } => {
                    let mut g = GLOBAL_DHT.lock().unwrap();
                    g.insert(key, value);
                }
                DhtCmd::Get { key, resp } => {
                    let g = GLOBAL_DHT.lock().unwrap();
                    let value = g.get(&key).cloned();
                    let _ = resp.send(value);
                }
                DhtCmd::Bootstrap(_addr) => {
                    // No-op in shared in-memory model
                }
            }
        }
    });

    DhtHandle { tx, listen_addr }
}

// Fallback stub when the `dht` feature is disabled.
#[cfg(not(feature = "dht"))]
pub async fn spawn_dht() -> DhtHandle {
    let (_tx, _rx) = mpsc::channel::<DhtCmd>(1);
    DhtHandle {}
}
