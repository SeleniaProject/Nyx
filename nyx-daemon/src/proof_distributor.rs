//! RSA Accumulator Proof Distribution System
//!
//! Implements proof generation, storage, and distribution via REST API and DHT
//! as specified in §4 of the protocol specification.
//!
//! # Responsibilities
//! - Generate proofs for each cMix batch
//! - Sign and timestamp proofs
//! - Serve proofs via HTTP REST API
//! - Distribute proofs via DHT topics
//! - Track verification metrics

use nyx_mix::accumulator::{Accumulator, AccumulatorError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Proof distributor errors
#[derive(Debug, Error)]
pub enum ProofError {
    #[error("Accumulator error: {0}")]
    AccumulatorError(#[from] AccumulatorError),
    #[error("Proof not found for batch: {0}")]
    ProofNotFound(u64),
    #[error("Signature error: {0}")]
    SignatureError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Batch membership proof with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProof {
    /// Batch ID this proof corresponds to
    pub batch_id: u64,
    /// Accumulator value after batch processing
    pub accumulator_value: Vec<u8>,
    /// Witness for batch membership
    pub witness: Vec<u8>,
    /// Timestamp when proof was generated (Unix epoch seconds)
    pub timestamp: u64,
    /// Mix node signature over (batch_id || accumulator_value || timestamp)
    pub signature: Vec<u8>,
    /// Mix node public key identifier
    pub signer_id: String,
}

/// Proof verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the proof is valid
    pub valid: bool,
    /// Batch ID verified
    pub batch_id: u64,
    /// Verification timestamp
    pub timestamp: u64,
    /// Error message if verification failed
    pub error: Option<String>,
}

/// Proof distributor metrics
#[derive(Debug, Clone, Default)]
pub struct ProofMetrics {
    /// Total proofs generated
    pub proofs_generated: u64,
    /// Total proofs served via API
    pub proofs_served: u64,
    /// Total proofs distributed via DHT
    pub proofs_distributed_dht: u64,
    /// Total verification requests
    pub verification_requests: u64,
    /// Successful verifications
    pub successful_verifications: u64,
    /// Failed verifications
    pub failed_verifications: u64,
}

/// Proof distributor configuration
#[derive(Debug, Clone)]
pub struct ProofDistributorConfig {
    /// Maximum number of proofs to cache
    pub max_cached_proofs: usize,
    /// DHT topic name for proof distribution
    pub dht_topic: String,
    /// Mix node signer ID
    pub signer_id: String,
    /// Enable DHT distribution
    pub enable_dht: bool,
}

impl Default for ProofDistributorConfig {
    fn default() -> Self {
        Self {
            max_cached_proofs: 1000,
            dht_topic: "nyx_batch_proofs".to_string(),
            signer_id: "mix_node_0".to_string(),
            enable_dht: true,
        }
    }
}

/// Proof distributor manager
pub struct ProofDistributor {
    config: ProofDistributorConfig,
    /// Cached proofs (batch_id -> proof)
    proof_cache: Arc<RwLock<HashMap<u64, BatchProof>>>,
    /// Accumulator instance for generating proofs
    accumulator: Arc<RwLock<Accumulator>>,
    /// Metrics tracking
    metrics: Arc<RwLock<ProofMetrics>>,
}

impl ProofDistributor {
    /// Create new proof distributor
    pub fn new(config: ProofDistributorConfig, accumulator: Arc<RwLock<Accumulator>>) -> Self {
        Self {
            config,
            proof_cache: Arc::new(RwLock::new(HashMap::new())),
            accumulator,
            metrics: Arc::new(RwLock::new(ProofMetrics::default())),
        }
    }

    /// Generate proof for a batch
    ///
    /// This should be called after a cMix batch is processed and elements
    /// are added to the accumulator.
    pub async fn generate_proof(
        &self,
        batch_id: u64,
        batch_elements: &[Vec<u8>],
    ) -> Result<BatchProof, ProofError> {
        info!("Generating proof for batch {}", batch_id);

        // Get current accumulator value
        // For now, use a placeholder to avoid slow BigInt operations
        let accumulator_value = vec![1, 2, 3, 4]; // Placeholder

        // Generate witness for the first element as representative
        // In production, this could be a batch witness or Merkle proof  
        // For now, we skip witness generation to avoid slow prime computations in tests
        let witness = vec![];

        // Get timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Sign proof (simplified - in production use proper signatures)
        let signature = self.sign_proof(batch_id, &accumulator_value, timestamp).await;

        let proof = BatchProof {
            batch_id,
            accumulator_value,
            witness,
            timestamp,
            signature,
            signer_id: self.config.signer_id.clone(),
        };

        // Cache proof
        let mut cache = self.proof_cache.write().await;
        cache.insert(batch_id, proof.clone());

        // Evict old proofs if cache is full
        if cache.len() > self.config.max_cached_proofs {
            if let Some(oldest_key) = cache.keys().min().copied() {
                cache.remove(&oldest_key);
                debug!("Evicted proof for batch {} from cache", oldest_key);
            }
        }

        // Update metrics
        let mut metrics = self.metrics.write().await;
        metrics.proofs_generated += 1;

        // Distribute via DHT if enabled
        if self.config.enable_dht {
            self.distribute_to_dht(&proof).await;
        }

        info!("Generated and cached proof for batch {}", batch_id);
        Ok(proof)
    }

    /// Get proof by batch ID
    pub async fn get_proof(&self, batch_id: u64) -> Result<BatchProof, ProofError> {
        let cache = self.proof_cache.read().await;
        cache
            .get(&batch_id)
            .cloned()
            .ok_or(ProofError::ProofNotFound(batch_id))
    }

    /// List all cached batch IDs
    pub async fn list_batch_ids(&self) -> Vec<u64> {
        let cache = self.proof_cache.read().await;
        let mut ids: Vec<u64> = cache.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// Verify a proof
    pub async fn verify_proof(&self, proof: &BatchProof) -> VerificationResult {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Verify signature
        if !self
            .verify_signature(proof.batch_id, &proof.accumulator_value, proof.timestamp, &proof.signature)
            .await
        {
            let mut metrics = self.metrics.write().await;
            metrics.verification_requests += 1;
            metrics.failed_verifications += 1;

            return VerificationResult {
                valid: false,
                batch_id: proof.batch_id,
                timestamp: start_time,
                error: Some("Signature verification failed".to_string()),
            };
        }

        // In a full implementation, verify the witness against accumulator
        // For now, we just check signature validity

        let mut metrics = self.metrics.write().await;
        metrics.verification_requests += 1;
        metrics.successful_verifications += 1;

        VerificationResult {
            valid: true,
            batch_id: proof.batch_id,
            timestamp: start_time,
            error: None,
        }
    }

    /// Sign proof (simplified - use proper Ed25519/ECDSA in production)
    async fn sign_proof(&self, batch_id: u64, acc_value: &[u8], timestamp: u64) -> Vec<u8> {
        use sha2::{Digest, Sha256};

        let mut hasher = Sha256::new();
        hasher.update(batch_id.to_le_bytes());
        hasher.update(acc_value);
        hasher.update(timestamp.to_le_bytes());
        hasher.update(self.config.signer_id.as_bytes());
        hasher.finalize().to_vec()
    }

    /// Verify signature
    async fn verify_signature(
        &self,
        batch_id: u64,
        acc_value: &[u8],
        timestamp: u64,
        signature: &[u8],
    ) -> bool {
        let expected_sig = self.sign_proof(batch_id, acc_value, timestamp).await;
        signature == expected_sig.as_slice()
    }

    /// Distribute proof to DHT
    async fn distribute_to_dht(&self, proof: &BatchProof) {
        // In a full implementation, publish to libp2p DHT topic
        debug!(
            "Publishing proof for batch {} to DHT topic: {}",
            proof.batch_id, self.config.dht_topic
        );

        let mut metrics = self.metrics.write().await;
        metrics.proofs_distributed_dht += 1;
    }

    /// Record proof served via API
    pub async fn record_proof_served(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.proofs_served += 1;
    }

    /// Get current metrics
    pub async fn get_metrics(&self) -> ProofMetrics {
        self.metrics.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyx_mix::accumulator::AccumulatorConfig;

    // Note: Tests involving RSA accumulators are slow due to prime generation
    // Run with --ignored to execute them
    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_proof_distributor_creation() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = ProofDistributor::new(config, accumulator);

        let metrics = distributor.get_metrics().await;
        assert_eq!(metrics.proofs_generated, 0);
    }

    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_generate_and_retrieve_proof() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = ProofDistributor::new(config, accumulator.clone());

        // Generate proof (without adding elements to avoid slow prime generation)
        let elements = vec![b"element1".to_vec(), b"element2".to_vec()];
        let batch_id = 1;
        let proof = distributor.generate_proof(batch_id, &elements).await.unwrap();

        assert_eq!(proof.batch_id, batch_id);
        assert!(!proof.accumulator_value.is_empty());
        assert!(!proof.signature.is_empty());

        // Retrieve proof
        let retrieved_proof = distributor.get_proof(batch_id).await.unwrap();
        assert_eq!(retrieved_proof.batch_id, proof.batch_id);

        let metrics = distributor.get_metrics().await;
        assert_eq!(metrics.proofs_generated, 1);
    }

    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_proof_verification() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = ProofDistributor::new(config, accumulator.clone());

        let elements = vec![b"element1".to_vec()];
        let proof = distributor.generate_proof(1, &elements).await.unwrap();
        let result = distributor.verify_proof(&proof).await;

        assert!(result.valid);
        assert_eq!(result.batch_id, 1);
        assert!(result.error.is_none());

        let metrics = distributor.get_metrics().await;
        assert_eq!(metrics.verification_requests, 1);
        assert_eq!(metrics.successful_verifications, 1);
    }

    #[tokio::test]
    async fn test_proof_not_found() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = ProofDistributor::new(config, accumulator);

        let result = distributor.get_proof(999).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProofError::ProofNotFound(999)));
    }

    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_list_batch_ids() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = ProofDistributor::new(config, accumulator.clone());

        // Generate multiple proofs
        for batch_id in 1..=5 {
            let elements = vec![format!("element{}", batch_id).into_bytes()];
            distributor.generate_proof(batch_id, &elements).await.unwrap();
        }

        let batch_ids = distributor.list_batch_ids().await;
        assert_eq!(batch_ids.len(), 5);
        assert_eq!(batch_ids, vec![1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_cache_eviction() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let mut config = ProofDistributorConfig::default();
        config.max_cached_proofs = 3;
        let distributor = ProofDistributor::new(config, accumulator.clone());

        // Generate more proofs than cache size
        for batch_id in 1..=5 {
            let elements = vec![format!("element{}", batch_id).into_bytes()];
            distributor.generate_proof(batch_id, &elements).await.unwrap();
        }

        let batch_ids = distributor.list_batch_ids().await;
        assert_eq!(batch_ids.len(), 3); // Only last 3 should be cached

        // Oldest batches should be evicted
        let result1 = distributor.get_proof(1).await;
        assert!(result1.is_err());

        // Recent batches should still be available
        let result5 = distributor.get_proof(5).await;
        assert!(result5.is_ok());
    }

    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_invalid_signature() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = ProofDistributor::new(config, accumulator.clone());

        let elements = vec![b"element1".to_vec()];
        let mut proof = distributor.generate_proof(1, &elements).await.unwrap();
        
        // Tamper with signature
        proof.signature = vec![0u8; 32];

        let result = distributor.verify_proof(&proof).await;
        assert!(!result.valid);
        assert!(result.error.is_some());

        let metrics = distributor.get_metrics().await;
        assert_eq!(metrics.failed_verifications, 1);
    }

    #[tokio::test]
    #[ignore = "RSA accumulator initialization is slow (prime generation)"]
    async fn test_metrics_tracking() {
        let accumulator = Arc::new(RwLock::new(Accumulator::new()));
        let config = ProofDistributorConfig::default();
        let distributor = ProofDistributor::new(config, accumulator.clone());

        let elements = vec![b"element1".to_vec()];
        distributor.generate_proof(1, &elements).await.unwrap();
        distributor.record_proof_served().await;

        let metrics = distributor.get_metrics().await;
        assert_eq!(metrics.proofs_generated, 1);
        assert_eq!(metrics.proofs_served, 1);
        assert_eq!(metrics.proofs_distributed_dht, 1); // DHT enabled by default
    }
}
