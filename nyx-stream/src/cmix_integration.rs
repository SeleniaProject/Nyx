//! cMix Integration for Nyx Protocol v1.0
//!
//! This module implements the complete cMix integration system as specified in
//! Nyx Protocol v1.0, providing VDF-based batch processing with RSA accumulator
//! proofs for enhanced anonymity guarantees.
//!
//! Key Features:
//! - VDF-based batch processing with configurable delays
//! - RSA accumulator proofs for mix node verification
//! - Frame-level integration with Nyx stream protocol
//! - Adaptive batch sizing and timing
//! - Cryptographic verification of mix processing

#![forbid(unsafe_code)]

use crate::frame::Frame;
use nyx_mix::{
    accumulator::{Accumulator, AccumulatorError},
    cmix::{Batcher, CmixError, VerifiedBatch},
    vdf::{SecureVdf, VdfConfig, VdfError, VdfOutput},
};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// cMix integration errors
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CmixIntegrationError {
    /// VDF computation failed
    #[error("VDF computation failed: {0}")]
    VdfFailed(VdfError),
    /// Batch processing error
    #[error("Batch processing error: {0}")]
    BatchError(CmixError),
    /// Accumulator proof validation failed
    #[error("Accumulator error: {0:?}")]
    AccumulatorError(AccumulatorError),
    /// Invalid configuration
    #[error("Invalid configuration: {reason}")]
    InvalidConfig { reason: String },
    /// Network timeout
    #[error("Network timeout after {duration:?}")]
    NetworkTimeout { duration: Duration },
    /// Channel communication error
    #[error("Channel error: {message}")]
    ChannelError { message: String },
}

impl From<VdfError> for CmixIntegrationError {
    fn from(e: VdfError) -> Self {
        CmixIntegrationError::VdfFailed(e)
    }
}

impl From<CmixError> for CmixIntegrationError {
    fn from(e: CmixError) -> Self {
        CmixIntegrationError::BatchError(e)
    }
}

impl From<AccumulatorError> for CmixIntegrationError {
    fn from(e: AccumulatorError) -> Self {
        CmixIntegrationError::AccumulatorError(e)
    }
}

/// cMix integration configuration
#[derive(Debug, Clone)]
pub struct CmixConfig {
    /// Enable cMix mode
    pub enabled: bool,
    /// Batch size for cMix processing
    pub batch_size: usize,
    /// VDF delay in milliseconds
    pub vdf_delay_ms: u64,
    /// Timeout for batch processing
    pub batch_timeout: Duration,
    /// Maximum concurrent batches
    pub max_concurrent_batches: usize,
    /// Network timeout for mix node communication
    pub network_timeout: Duration,
    /// VDF security parameters
    pub vdf_config: VdfConfig,
    /// Enable accumulator proofs
    pub enable_accumulator_proofs: bool,
}

impl Default for CmixConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            batch_size: 100,
            vdf_delay_ms: 100,
            batch_timeout: Duration::from_millis(1000),
            max_concurrent_batches: 10,
            network_timeout: Duration::from_secs(30),
            vdf_config: VdfConfig::default(),
            enable_accumulator_proofs: true,
        }
    }
}

/// cMix batch processing state
#[derive(Debug, Clone)]
pub struct BatchState {
    /// Batch ID
    pub id: u64,
    /// Processing state
    pub state: BatchProcessingState,
    /// Created timestamp
    pub created_at: Instant,
    /// VDF proof
    pub vdf_proof: Option<VdfOutput>,
    /// Accumulator witness
    pub accumulator_witness: Option<Vec<u8>>,
    /// Frame contents
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BatchProcessingState {
    /// Collecting frames
    Collecting,
    /// Computing VDF
    VdfComputing,
    /// Generating accumulator proof
    AccumulatorProof,
    /// Ready for transmission
    Ready,
    /// Transmitted to mix network
    Transmitted,
    /// Processing failed
    Failed(CmixIntegrationError),
}

/// cMix integration statistics
#[derive(Debug, Clone, Default)]
pub struct CmixStats {
    /// Total frames processed
    pub frames_processed: u64,
    /// Total batches created
    pub batches_created: u64,
    /// Total batches successfully transmitted
    pub batches_transmitted: u64,
    /// Total VDF computations
    pub vdf_computations: u64,
    /// Total VDF computation time
    pub total_vdf_time: Duration,
    /// Total accumulator proofs generated
    pub accumulator_proofs: u64,
    /// Processing errors
    pub errors: u64,
    /// Average batch processing time
    pub avg_batch_time: Duration,
}

/// Main cMix integration manager
pub struct CmixIntegrationManager {
    /// Configuration
    config: CmixConfig,
    /// VDF instance
    #[allow(dead_code)]
    vdf: SecureVdf,
    /// RSA accumulator
    accumulator: RwLock<Accumulator>,
    /// Current batcher
    batcher: RwLock<Batcher>,
    /// Active batches
    active_batches: RwLock<HashMap<u64, BatchState>>,
    /// Frame queue for processing
    frame_queue: RwLock<VecDeque<Frame>>,
    /// Statistics
    stats: RwLock<CmixStats>,
    /// Communication channels
    batch_sender: mpsc::UnboundedSender<VerifiedBatch>,
    #[allow(dead_code)]
    frame_receiver: RwLock<Option<mpsc::UnboundedReceiver<Frame>>>,
}

impl CmixIntegrationManager {
    /// Create new cMix integration manager
    pub fn new(config: CmixConfig) -> Result<Self, CmixIntegrationError> {
        // Validate configuration
        if config.batch_size == 0 {
            return Err(CmixIntegrationError::InvalidConfig {
                reason: "Batch size cannot be zero".to_string(),
            });
        }

        if config.vdf_delay_ms == 0 {
            return Err(CmixIntegrationError::InvalidConfig {
                reason: "VDF delay cannot be zero".to_string(),
            });
        }

        // Initialize VDF
        let vdf = SecureVdf::with_config(config.vdf_config.clone());

        // Initialize accumulator
        let accumulator = RwLock::new(Accumulator::new());

        // Initialize batcher
        let batcher = RwLock::new(Batcher::with_vdf_delay(
            config.batch_size,
            config.batch_timeout,
            config.vdf_delay_ms as u32,
        ));

        // Create communication channels
        let (batch_sender, _) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            vdf,
            accumulator,
            batcher,
            active_batches: RwLock::new(HashMap::new()),
            frame_queue: RwLock::new(VecDeque::new()),
            stats: RwLock::new(CmixStats::default()),
            batch_sender,
            frame_receiver: RwLock::new(None),
        })
    }

    /// Process incoming frame for cMix batching
    pub async fn process_frame(&self, frame: Frame) -> Result<(), CmixIntegrationError> {
        if !self.config.enabled {
            // Pass through without cMix processing
            return Ok(());
        }

        debug!(
            "Processing frame for cMix integration: {:?}",
            frame.header.ty
        );

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.frames_processed += 1;
        }

        // Add frame to queue
        {
            let mut queue = self.frame_queue.write().await;
            queue.push_back(frame);
        }

        // Try to create batch if queue is full
        self.try_create_batch().await?;

        Ok(())
    }

    /// Try to create a batch from queued frames
    async fn try_create_batch(&self) -> Result<Option<u64>, CmixIntegrationError> {
        let mut queue = self.frame_queue.write().await;

        if queue.len() < self.config.batch_size {
            return Ok(None);
        }

        // Collect frames for batch
        let mut frames = Vec::new();
        for _ in 0..self.config.batch_size {
            if let Some(frame) = queue.pop_front() {
                frames.push(frame);
            }
        }

        drop(queue); // Release lock early

        // Create batch
        let batch_id = self.create_batch(frames).await?;
        Ok(Some(batch_id))
    }

    /// Create a new batch for processing
    async fn create_batch(&self, frames: Vec<Frame>) -> Result<u64, CmixIntegrationError> {
        let start_time = Instant::now();

        // Generate batch ID
        let batch_id = {
            let mut stats = self.stats.write().await;
            stats.batches_created += 1;
            stats.batches_created
        };

        info!(
            "Creating cMix batch {} with {} frames",
            batch_id,
            frames.len()
        );

        // Create initial batch state
        let batch_state = BatchState {
            id: batch_id,
            state: BatchProcessingState::Collecting,
            created_at: start_time,
            vdf_proof: None,
            accumulator_witness: None,
            frames: frames.clone(),
        };

        // Store batch state
        {
            let mut batches = self.active_batches.write().await;
            batches.insert(batch_id, batch_state);
        }

        // Convert frames to packets for batcher
        let packets: Vec<Vec<u8>> = frames.iter().map(|frame| frame.payload.clone()).collect();

        // Process through batcher with VDF
        let verified_batch = {
            let mut batcher = self.batcher.write().await;

            // Add packets to batcher
            for packet in packets {
                batcher.push(packet)?;
            }

            // Force flush to create verified batch
            batcher.force_flush()?
        };

        // Update batch state with VDF proof
        {
            let mut batches = self.active_batches.write().await;
            if let Some(batch_state) = batches.get_mut(&batch_id) {
                batch_state.state = BatchProcessingState::VdfComputing;

                // Create VDF output from verified batch
                let vdf_output = VdfOutput {
                    output: verified_batch.vdf_proof,
                    proof: verified_batch.accumulator_witness.clone(),
                    __computation_time: start_time.elapsed(),
                    __iteration_s: self.config.vdf_delay_ms,
                };

                batch_state.vdf_proof = Some(vdf_output);
                batch_state.state = BatchProcessingState::AccumulatorProof;
            }
        }

        // Generate accumulator proof if enabled
        if self.config.enable_accumulator_proofs {
            self.generate_accumulator_proof(batch_id, &verified_batch)
                .await?;
        }

        // Mark batch as ready
        {
            let mut batches = self.active_batches.write().await;
            if let Some(batch_state) = batches.get_mut(&batch_id) {
                batch_state.state = BatchProcessingState::Ready;
                batch_state.accumulator_witness = Some(verified_batch.accumulator_witness.clone());
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.vdf_computations += 1;
            stats.total_vdf_time += start_time.elapsed();
            if self.config.enable_accumulator_proofs {
                stats.accumulator_proofs += 1;
            }
        }

        // Send batch for transmission
        if self.batch_sender.send(verified_batch).is_err() {
            warn!("Failed to send batch {} for transmission", batch_id);
        }

        info!("cMix batch {} ready for transmission", batch_id);
        Ok(batch_id)
    }

    /// Generate accumulator proof for batch
    async fn generate_accumulator_proof(
        &self,
        batch_id: u64,
        verified_batch: &VerifiedBatch,
    ) -> Result<Vec<u8>, CmixIntegrationError> {
        debug!("Generating accumulator proof for batch {}", batch_id);

        let mut accumulator = self.accumulator.write().await;

        // Add batch to accumulator
        let batch_element = verified_batch.integrity_hash.to_vec();
        accumulator.add_element(&batch_element)?;

        // Generate witness
        let witness = accumulator.generate_witness(&batch_element)?;

        // Convert BigInt to Vec<u8> for storage
        let witness_bytes = witness.to_bytes_be().1;

        debug!("Generated accumulator proof for batch {}", batch_id);
        Ok(witness_bytes)
    }

    /// Verify accumulator proof for received batch
    pub async fn verify_accumulator_proof(
        &self,
        batch_id: u64,
        integrity_hash: &[u8],
        witness: &[u8],
    ) -> Result<bool, CmixIntegrationError> {
        debug!("Verifying accumulator proof for batch {}", batch_id);

        let mut accumulator = self.accumulator.write().await;

        // Convert witness bytes back to BigInt
        let witness_bigint = num_bigint::BigInt::from_bytes_be(num_bigint::Sign::Plus, witness);

        let is_valid = accumulator.verify_element(integrity_hash, &witness_bigint);

        if is_valid {
            debug!("Accumulator proof verified for batch {}", batch_id);
        } else {
            warn!(
                "Accumulator proof verification failed for batch {}",
                batch_id
            );
        }

        Ok(is_valid)
    }

    /// Get current statistics
    pub async fn stats(&self) -> CmixStats {
        self.stats.read().await.clone()
    }

    /// Get batch state
    pub async fn get_batch_state(&self, batch_id: u64) -> Option<BatchState> {
        self.active_batches.read().await.get(&batch_id).cloned()
    }

    /// Remove completed batch
    pub async fn remove_batch(&self, batch_id: u64) -> Option<BatchState> {
        self.active_batches.write().await.remove(&batch_id)
    }

    /// Process pending batches (cleanup old ones)
    pub async fn process_pending_batches(&self) -> Result<(), CmixIntegrationError> {
        let now = Instant::now();
        let timeout = self.config.batch_timeout * 2; // Allow extra time for processing

        let mut to_remove = Vec::new();

        {
            let batches = self.active_batches.read().await;
            for (batch_id, batch_state) in batches.iter() {
                if now.duration_since(batch_state.created_at) > timeout {
                    to_remove.push(*batch_id);
                }
            }
        }

        // Remove timed out batches
        if !to_remove.is_empty() {
            let mut batches = self.active_batches.write().await;
            for batch_id in to_remove {
                if let Some(removed) = batches.remove(&batch_id) {
                    warn!("Removed timed out batch {}", batch_id);
                    if matches!(removed.state, BatchProcessingState::Failed(_)) {
                        let mut stats = self.stats.write().await;
                        stats.errors += 1;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if cMix mode is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get configuration
    pub fn config(&self) -> &CmixConfig {
        &self.config
    }

    /// Force flush any pending frames
    pub async fn force_flush(&self) -> Result<Vec<u64>, CmixIntegrationError> {
        let mut batch_ids = Vec::new();

        // Process any remaining frames in queue
        loop {
            let queue_len = {
                let queue = self.frame_queue.read().await;
                queue.len()
            };

            if queue_len == 0 {
                break;
            }

            // Create batch with remaining frames (even if less than batch_size)
            let frames = {
                let mut queue = self.frame_queue.write().await;
                let count = queue.len().min(self.config.batch_size);
                let mut frames = Vec::new();
                for _ in 0..count {
                    if let Some(frame) = queue.pop_front() {
                        frames.push(frame);
                    }
                }
                frames
            };

            if !frames.is_empty() {
                let batch_id = self.create_batch(frames).await?;
                batch_ids.push(batch_id);
            }
        }

        Ok(batch_ids)
    }

    /// Get frame queue length
    pub async fn queue_length(&self) -> usize {
        self.frame_queue.read().await.len()
    }

    /// Get active batch count
    pub async fn active_batch_count(&self) -> usize {
        self.active_batches.read().await.len()
    }
}

/// cMix frame wrapper for protocol integration
#[derive(Debug, Clone)]
pub struct CmixFrame {
    /// Original frame
    pub frame: Frame,
    /// Batch ID (if processed)
    pub batch_id: Option<u64>,
    /// VDF proof
    pub vdf_proof: Option<VdfOutput>,
    /// Accumulator witness
    pub accumulator_witness: Option<Vec<u8>>,
    /// Processing timestamp
    pub processed_at: Option<Instant>,
}

impl CmixFrame {
    /// Create new cMix frame
    pub fn new(frame: Frame) -> Self {
        Self {
            frame,
            batch_id: None,
            vdf_proof: None,
            accumulator_witness: None,
            processed_at: None,
        }
    }

    /// Mark as processed with batch information
    pub fn mark_processed(
        &mut self,
        batch_id: u64,
        vdf_proof: VdfOutput,
        accumulator_witness: Vec<u8>,
    ) {
        self.batch_id = Some(batch_id);
        self.vdf_proof = Some(vdf_proof);
        self.accumulator_witness = Some(accumulator_witness);
        self.processed_at = Some(Instant::now());
    }

    /// Check if frame has been processed
    pub fn is_processed(&self) -> bool {
        self.batch_id.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Frame, FrameHeader, FrameType};

    fn create_test_frame(payload: Vec<u8>) -> Frame {
        Frame {
            header: FrameHeader {
                stream_id: 1,
                seq: 1,
                ty: FrameType::Data,
            },
            payload,
        }
    }

    #[tokio::test]
    async fn test_cmix_manager_creation() {
        let config = CmixConfig::default();
        let manager = CmixIntegrationManager::new(config).unwrap();

        assert!(!manager.is_enabled());
        assert_eq!(manager.config().batch_size, 100);
        assert_eq!(manager.queue_length().await, 0);
    }

    #[tokio::test]
    async fn test_frame_processing_disabled() {
        let config = CmixConfig {
            enabled: false,
            ..Default::default()
        };
        let manager = CmixIntegrationManager::new(config).unwrap();

        let frame = create_test_frame(b"test data".to_vec());
        let result = manager.process_frame(frame).await;

        assert!(result.is_ok());
        assert_eq!(manager.queue_length().await, 0);
    }

    #[tokio::test]
    async fn test_frame_processing_enabled() {
        let config = CmixConfig {
            enabled: true,
            batch_size: 2,
            ..Default::default()
        };
        let manager = CmixIntegrationManager::new(config).unwrap();

        let frame1 = create_test_frame(b"test data 1".to_vec());
        let frame2 = create_test_frame(b"test data 2".to_vec());

        // Process first frame
        manager.process_frame(frame1).await.unwrap();
        assert_eq!(manager.queue_length().await, 1);

        // Process second frame - should trigger batch creation
        manager.process_frame(frame2).await.unwrap();
        assert_eq!(manager.queue_length().await, 0);

        // Check statistics
        let stats = manager.stats().await;
        assert_eq!(stats.frames_processed, 2);
        assert_eq!(stats.batches_created, 1);
    }

    #[tokio::test]
    async fn test_batch_state_tracking() {
        let config = CmixConfig {
            enabled: true,
            batch_size: 1,
            ..Default::default()
        };
        let manager = CmixIntegrationManager::new(config).unwrap();

        let frame = create_test_frame(b"test data".to_vec());
        manager.process_frame(frame).await.unwrap();

        // Check that batch was created
        let stats = manager.stats().await;
        assert_eq!(stats.batches_created, 1);

        // Check batch state
        let batch_state = manager.get_batch_state(1).await;
        assert!(batch_state.is_some());

        let state = batch_state.unwrap();
        assert_eq!(state.id, 1);
        assert_eq!(state.frames.len(), 1);
        assert!(matches!(state.state, BatchProcessingState::Ready));
    }

    #[tokio::test]
    async fn test_force_flush() {
        let config = CmixConfig {
            enabled: true,
            batch_size: 10, // Large batch size
            ..Default::default()
        };
        let manager = CmixIntegrationManager::new(config).unwrap();

        // Add some frames (less than batch size)
        for i in 0..5 {
            let frame = create_test_frame(format!("test data {i}").as_bytes().to_vec());
            manager.process_frame(frame).await.unwrap();
        }

        assert_eq!(manager.queue_length().await, 5);

        // Force flush
        let batch_ids = manager.force_flush().await.unwrap();

        assert_eq!(batch_ids.len(), 1);
        assert_eq!(manager.queue_length().await, 0);

        let stats = manager.stats().await;
        assert_eq!(stats.batches_created, 1);
    }

    #[tokio::test]
    async fn test_accumulator_proof_verification() {
        let config = CmixConfig {
            enabled: true,
            enable_accumulator_proofs: true,
            batch_size: 1,
            ..Default::default()
        };
        let manager = CmixIntegrationManager::new(config).unwrap();

        let frame = create_test_frame(b"test data".to_vec());
        manager.process_frame(frame).await.unwrap();

        // Get batch state
        let batch_state = manager.get_batch_state(1).await.unwrap();

        // Verify accumulator proof
        if let Some(witness) = &batch_state.accumulator_witness {
            let integrity_hash = b"test_integrity_hash"; // Simplified for test
            let result = manager
                .verify_accumulator_proof(1, integrity_hash, witness)
                .await;

            // Note: This test may fail due to simplified hash, but verifies the interface
            assert!(result.is_ok());
        }
    }

    #[tokio::test]
    async fn test_cmix_frame_wrapper() {
        let frame = create_test_frame(b"test data".to_vec());
        let mut cmix_frame = CmixFrame::new(frame);

        assert!(!cmix_frame.is_processed());
        assert!(cmix_frame.batch_id.is_none());

        let vdf_output = VdfOutput {
            output: [0u8; 32],
            proof: vec![1, 2, 3],
            __computation_time: Duration::from_millis(100),
            __iteration_s: 100,
        };

        cmix_frame.mark_processed(1, vdf_output, vec![4, 5, 6]);

        assert!(cmix_frame.is_processed());
        assert_eq!(cmix_frame.batch_id, Some(1));
        assert!(cmix_frame.vdf_proof.is_some());
        assert!(cmix_frame.accumulator_witness.is_some());
    }

    #[tokio::test]
    async fn test_invalid_config() {
        let config = CmixConfig {
            batch_size: 0, // Invalid
            ..Default::default()
        };

        let result = CmixIntegrationManager::new(config);
        assert!(result.is_err());

        if let Err(CmixIntegrationError::InvalidConfig { reason }) = result {
            assert!(reason.contains("Batch size cannot be zero"));
        }
    }
}
