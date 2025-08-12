#![forbid(unsafe_code)]

//! Zero-copy egress helper for the stream layer.
//!
//! This helper spawns a background task that reads framed bytes from an
//! `mpsc::Receiver<Vec<u8>>` and transmits them using
//! `ZeroCopyTxAdapter` with minimal copying.

use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, error, instrument};

use crate::zero_copy_tx::ZeroCopyTxAdapter;

/// Spawn a zero-copy egress task.
///
/// - `bind_addr`: Local UDP bind address (e.g., 0.0.0.0:0)
/// - `target`: Remote destination address
/// - `path_id`: Identifier used for allocation/telemetry scoping
/// - `rx`: Receiver of already-framed bytes for transmission
pub async fn spawn_zero_copy_egress(
    bind_addr: SocketAddr,
    target: SocketAddr,
    path_id: String,
    mut rx: mpsc::Receiver<Vec<u8>>,
) -> Result<tokio::task::JoinHandle<()>> {
    let adapter = Arc::new(ZeroCopyTxAdapter::new(bind_addr, path_id).await?);
    adapter.set_target(target).await;

    let handle = tokio::spawn(async move {
        while let Some(frame) = rx.recv().await {
            if frame.is_empty() {
                continue;
            }
            match adapter.send(&frame, None).await {
                Ok(n) => {
                    debug!(bytes = n, "zero-copy egress sent");
                }
                Err(e) => {
                    error!(error = %e, "zero-copy egress failed");
                    // For now, continue; upper layers can observe via metrics.
                }
            }
        }
    });

    Ok(handle)
}


