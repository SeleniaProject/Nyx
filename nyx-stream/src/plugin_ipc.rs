#![forbid(unsafe_code)]

//! Plugin IPC Transport Implementation
//! In-memory duplex transport for development/testing. Provides a registry of
//! server instances keyed by a textual identifier so that a client can connect
//! to an existing server and exchange messages bidirectionally.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::timeout;

use bytes::{BufMut, Bytes};

use tracing::debug;

/// Outbound sender half.
pub struct PluginIpcTx {
    tx: mpsc::Sender<Vec<u8>>,
}

/// Inbound receiver half.
pub struct PluginIpcRx {
    rx: mpsc::Receiver<Vec<u8>>,
}

impl PluginIpcTx {
    pub async fn send(&self, data: &[u8]) -> Result<(), crate::PluginFrameError> {
        self.tx
            .send(data.to_vec())
            .await
            .map_err(|_| crate::PluginFrameError::ValidationError("IPC channel closed".to_string()))
    }
}

impl PluginIpcRx {
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.rx.recv().await
    }
}

// Global in-memory registry for duplex channels by id
struct IpcPair {
    // Server -> Client
    s2c_tx: mpsc::Sender<Vec<u8>>,
    s2c_rx: Option<mpsc::Receiver<Vec<u8>>>,
    // Client -> Server
    c2s_tx: mpsc::Sender<Vec<u8>>,
    c2s_rx: Option<mpsc::Receiver<Vec<u8>>>,
}

static IPC_REGISTRY: once_cell::sync::Lazy<RwLock<HashMap<String, IpcPair>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(HashMap::new()));

/// Spawn an in-memory IPC server with identifier `id`.
/// Returns (tx_to_client, rx_from_client) for the server side.
pub async fn spawn_ipc_server(id: &str) -> std::io::Result<(PluginIpcTx, PluginIpcRx)> {
    let (s2c_tx, s2c_rx) = mpsc::channel(1024);
    let (c2s_tx, c2s_rx) = mpsc::channel(1024);
    let mut reg = IPC_REGISTRY.write().await;
    reg.insert(
        id.to_string(),
        IpcPair {
            s2c_tx: s2c_tx.clone(),
            s2c_rx: Some(s2c_rx),
            c2s_tx: c2s_tx.clone(),
            c2s_rx: Some(c2s_rx),
        },
    );
    // Server consumes client->server receiver; client will later take server->client receiver.
    let rx_from_client = reg
        .get_mut(id)
        .and_then(|p| p.c2s_rx.take())
        .expect("c2s_rx just inserted");
    Ok((
        PluginIpcTx { tx: s2c_tx },
        PluginIpcRx { rx: rx_from_client },
    ))
}

/// Connect a client to an existing server identified by `path_or_id`.
/// Returns (tx_to_server, rx_from_server) for the client side.
pub async fn connect_client(path_or_id: &str) -> std::io::Result<(PluginIpcTx, PluginIpcRx)> {
    let mut reg = IPC_REGISTRY.write().await;
    match reg.get_mut(path_or_id) {
        Some(pair) => {
            let rx_from_server = pair.s2c_rx.take().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "server channel unavailable")
            })?;
            Ok((
                PluginIpcTx {
                    tx: pair.c2s_tx.clone(),
                },
                PluginIpcRx { rx: rx_from_server },
            ))
        }
        None => Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "ipc server not found",
        )),
    }
}

/// Create mock IPC channel pair for testing
pub async fn create_plugin_ipc(
    plugin_id: crate::plugin_cbor::PluginId,
) -> Result<(PluginIpcTx, PluginIpcRx), crate::PluginFrameError> {
    let id = format!("plugin:{}", plugin_id);
    let (srv_tx, srv_rx) = spawn_ipc_server(&id)
        .await
        .map_err(|e| crate::PluginFrameError::ValidationError(e.to_string()))?;
    debug!("Created IPC channel for plugin {}", plugin_id);
    Ok((srv_tx, srv_rx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_ipc_duplex_works() {
        let id = "test-ipc";
        // Spawn server
        let (mut srv_tx, mut srv_rx) = spawn_ipc_server(id).await.expect("spawn");
        // Connect client
        let (mut cli_tx, mut cli_rx) = connect_client(id).await.expect("connect");

        // Client -> Server
        cli_tx.send(b"ping").await.expect("c->s send");
        let s_msg = srv_rx.recv().await.expect("server recv");
        assert_eq!(s_msg, b"ping");

        // Server -> Client
        srv_tx.send(b"pong").await.expect("s->c send");
        let c_msg = cli_rx.recv().await.expect("client recv");
        assert_eq!(c_msg, b"pong");
    }
}
