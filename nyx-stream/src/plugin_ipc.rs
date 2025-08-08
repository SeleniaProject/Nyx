#![forbid(unsafe_code)]

//! Plugin IPC Transport Implementation
//! Mock implementation for initial development.

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
        self.tx.send(data.to_vec()).await
            .map_err(|_| crate::PluginFrameError::ValidationError("IPC channel closed".to_string()))
    }
}

impl PluginIpcRx {
    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.rx.recv().await
    }
}

/// Mock implementation for all platforms
pub async fn spawn_ipc_server(id: &str) -> std::io::Result<(PluginIpcTx, PluginIpcRx)> {
    let (tx, rx) = tokio::sync::mpsc::channel(1024);
    Ok((PluginIpcTx { tx }, PluginIpcRx { rx }))
}

/// Mock implementation for all platforms
pub async fn connect_client(_path: &str) -> std::io::Result<(PluginIpcTx, PluginIpcRx)> {
    let (tx, rx) = tokio::sync::mpsc::channel(1024);
    Ok((PluginIpcTx { tx }, PluginIpcRx { rx }))
}

/// Create mock IPC channel pair for testing
pub async fn create_plugin_ipc(plugin_id: crate::plugin_cbor::PluginId) -> Result<(PluginIpcTx, PluginIpcRx), crate::PluginFrameError> {
    let (tx, rx) = tokio::sync::mpsc::channel(1024);
    debug!("Created IPC channel for plugin {}", plugin_id);
    Ok((PluginIpcTx { tx }, PluginIpcRx { rx }))
} 