#![forbid(unsafe_code)]

//! Zero-copy transmission adapter for the stream layer.
//!
//! This module bridges `nyx-stream` with the zero-copy pipeline provided by
//! `nyx-core::zero_copy`. It enables UDP transmission with minimal buffer copies
//! while keeping the stream layer decoupled from transport details.
//!
//! The adapter intentionally does not enforce AEAD/FEC here; those can be
//! configured via the zero-copy pipeline if desired by callers.

use std::net::SocketAddr;
use std::sync::Arc;
use anyhow::Result;

use nyx_core::zero_copy::{
    ZeroCopyManager, ZeroCopyManagerConfig,
};
use nyx_core::zero_copy::integration::ZeroCopyPipeline;

/// Runtime adapter that owns a zero-copy pipeline bound to a local UDP socket.
pub struct ZeroCopyTxAdapter {
    manager: Arc<ZeroCopyManager>,
    pipeline: ZeroCopyPipeline,
    target: tokio::sync::Mutex<Option<SocketAddr>>, // optional default destination
}

impl ZeroCopyTxAdapter {
    /// Create a new adapter bound to `bind_addr`. A unique `path_id` namespaces metrics.
    pub async fn new(bind_addr: SocketAddr, path_id: String) -> Result<Self> {
        let manager = Arc::new(ZeroCopyManager::new(ZeroCopyManagerConfig::default()));
        let pipeline = ZeroCopyPipeline::new(Arc::clone(&manager), path_id)
            .with_transmission(bind_addr)
            .await?;
        Ok(Self { manager, pipeline, target: tokio::sync::Mutex::new(None) })
    }

    /// Set default target address for subsequent sends.
    pub async fn set_target(&self, target: SocketAddr) {
        *self.target.lock().await = Some(target);
    }

    /// Send a packet to `target`. If `target` is None, uses the default target.
    pub async fn send(&self, data: &[u8], target: Option<SocketAddr>) -> Result<usize> {
        let dest = if let Some(t) = target { t } else {
            self.target.lock().await.ok_or_else(|| anyhow::anyhow!("no target configured"))?
        };
        // Use the end-to-end pipeline. AEAD/FEC are optional (not configured here).
        let bytes_sent = self.pipeline.process_complete_packet(data, dest).await?;
        Ok(bytes_sent)
    }

    /// Expose current aggregated metrics from the pipeline manager.
    pub async fn metrics(&self) -> nyx_core::zero_copy::AllocationMetrics {
        let path = self.manager.get_critical_path(&self.pipeline.path_id).await
            .expect("critical path must exist while adapter lives");
        path.get_metrics().await
    }
}


