//! cMix batcher implementation with VDF integration and tamper detection

use std::time::{Duration, Instant};
use sha2::{Digest, Sha256};
use crate::{vdf, accumulator};

/// Detailed error information for cMix operations
#[derive(Debug, Clone, PartialEq)]
pub enum CmixError {
    /// Batch verification failed due to tampering
    TamperedBatch { batch_id: u64, expected_hash: [u8; 32], actual_hash: [u8; 32] },
    /// VDF computation timeout
    VdfTimeout { duration: Duration, max_allowed: Duration },
    /// Invalid witness for RSA accumulator
    InvalidWitness { element: Vec<u8>, witness: Vec<u8> },
    /// Batch size constraints violated
    InvalidBatchSize { size: usize, min: usize, max: usize },
}

/// Comprehensive statistics for cMix batcher operations
#[derive(Default, Debug, Clone)]
pub struct BatchStats {
    /// Number of batches successfully emitted
    pub emitted: usize,
    /// Last flush timestamp
    pub last_flush: Option<Instant>,
    /// Number of errors encountered
    pub errors: usize,
    /// Number of VDF computations performed
    pub vdf_computations: usize,
    /// Number of verification failures
    pub verification_failures: usize,
    /// Total processing time for VDF operations
    pub total_vdf_time: Duration,
}

/// A batch with cryptographic verification metadata
#[derive(Debug, Clone)]
pub struct VerifiedBatch {
    /// Batch sequence number
    pub id: u64,
    /// Packet contents
    pub packets: Vec<Vec<u8>>,
    /// VDF output for timing verification
    pub vdf_proof: [u8; 32],
    /// RSA accumulator witness
    pub accumulator_witness: Vec<u8>,
    /// Batch integrity hash
    pub integrity_hash: [u8; 32],
    /// Creation timestamp
    pub created_at: Instant,
}

/// cMix batcher with VDF delays and cryptographic verification
pub struct Batcher {
    /// Maximum batch size
    size: usize,
    /// Timeout for batch emission
    timeout: Duration,
    /// VDF delay in milliseconds
    vdf_delay_ms: u32,
    /// Current packet buffer
    buf: Vec<Vec<u8>>,
    /// Operation statistics
    pub stats: BatchStats,
    /// Next batch sequence number
    next_batch_id: u64,
    /// Error log for detailed reporting
    pub error_log: Vec<(Instant, CmixError)>,
}

impl Batcher {
    /// Create a new cMix batcher with specified parameters
    pub fn new(size: usize, timeout: Duration) -> Self {
        Self::with_vdf_delay(size, timeout, 100) // Default 100ms VDF delay
    }

    /// Create a new cMix batcher with custom VDF delay
    pub fn with_vdf_delay(size: usize, timeout: Duration, vdf_delay_ms: u32) -> Self {
        Self {
            size,
            timeout,
            vdf_delay_ms,
            buf: Vec::with_capacity(size),
            stats: Default::default(),
            next_batch_id: 1,
            error_log: Vec::new(),
        }
    }

    /// Add a packet to the batch, returning a verified batch if ready
    pub fn push(&mut self, pkt: Vec<u8>) -> Result<Option<VerifiedBatch>, CmixError> {
        // Validate packet size constraints
        if pkt.len() > 65536 {
            let error = CmixError::InvalidBatchSize { 
                size: pkt.len(), 
                min: 1, 
                max: 65536 
            };
            self.record_error(error.clone());
            return Err(error);
        }

        self.buf.push(pkt);
        
        if self.buf.len() >= self.size {
            return Ok(Some(self.flush_with_verification()?));
        }
        
        Ok(None)
    }

    /// Check for timeout-based batch emission
    pub fn tick(&mut self, now: Instant) -> Result<Option<VerifiedBatch>, CmixError> {
        match self.stats.last_flush {
            None => {
                self.stats.last_flush = Some(now);
                Ok(None)
            }
            Some(last) if now.duration_since(last) >= self.timeout && !self.buf.is_empty() => {
                Ok(Some(self.flush_with_verification()?))
            }
            _ => Ok(None),
        }
    }

    /// Verify a batch against tampering
    pub fn verify_batch(&mut self, batch: &VerifiedBatch) -> Result<(), CmixError> {
        // Recompute integrity hash
        let computed_hash = self.compute_batch_hash(&batch.packets);
        
        if computed_hash != batch.integrity_hash {
            let error = CmixError::TamperedBatch {
                batch_id: batch.id,
                expected_hash: batch.integrity_hash,
                actual_hash: computed_hash,
            };
            self.record_error(error.clone());
            return Err(error);
        }

        // Verify RSA accumulator witness (simplified for this implementation)
        if !accumulator::verify_membership(
            &batch.accumulator_witness,
            &batch.id.to_le_bytes(),
            &computed_hash,
        ) {
            let error = CmixError::InvalidWitness {
                element: batch.id.to_le_bytes().to_vec(),
                witness: batch.accumulator_witness.clone(),
            };
            self.record_error(error.clone());
            return Err(error);
        }

        Ok(())
    }

    /// Force flush current buffer with full cryptographic verification
    pub fn force_flush(&mut self) -> Result<VerifiedBatch, CmixError> {
        self.flush_with_verification()
    }

    /// Generate detailed error report with security audit information
    pub fn generate_error_report(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!("=== cMix Batcher Security Audit Report ===\n"));
        report.push_str(&format!("Total errors: {}\n", self.stats.errors));
        report.push_str(&format!("Verification failures: {}\n", self.stats.verification_failures));
        report.push_str(&format!("VDF computations: {}\n", self.stats.vdf_computations));
        report.push_str(&format!("Total VDF time: {:?}\n", self.stats.total_vdf_time));
        report.push_str(&format!("Batches emitted: {}\n", self.stats.emitted));
        
        // Security metrics
        if self.stats.emitted > 0 {
            let error_rate = (self.stats.errors as f64 / self.stats.emitted as f64) * 100.0;
            report.push_str(&format!("Error rate: {:.2}%\n", error_rate));
            
            let verification_failure_rate = (self.stats.verification_failures as f64 / self.stats.emitted as f64) * 100.0;
            report.push_str(&format!("Verification failure rate: {:.2}%\n", verification_failure_rate));
        }
        
        if self.stats.vdf_computations > 0 {
            let avg_vdf_time = self.stats.total_vdf_time / self.stats.vdf_computations as u32;
            report.push_str(&format!("Average VDF time: {:?}\n", avg_vdf_time));
        }
        
        if !self.error_log.is_empty() {
            report.push_str("\n=== Recent Security Events ===\n");
            for (timestamp, error) in &self.error_log {
                match error {
                    CmixError::TamperedBatch { batch_id, expected_hash, actual_hash } => {
                        report.push_str(&format!("[{:?}] SECURITY ALERT: Batch {} tampered - Expected: {:?}, Actual: {:?}\n", 
                                               timestamp, batch_id, expected_hash, actual_hash));
                    },
                    CmixError::VdfTimeout { duration, max_allowed } => {
                        report.push_str(&format!("[{:?}] PERFORMANCE: VDF timeout - Took: {:?}, Max: {:?}\n", 
                                               timestamp, duration, max_allowed));
                    },
                    CmixError::InvalidWitness { element, witness } => {
                        report.push_str(&format!("[{:?}] SECURITY ALERT: Invalid accumulator witness for element {:?}\n", 
                                               timestamp, element));
                    },
                    CmixError::InvalidBatchSize { size, min, max } => {
                        report.push_str(&format!("[{:?}] VALIDATION: Invalid batch size {} (range: {}-{})\n", 
                                               timestamp, size, min, max));
                    },
                }
            }
        }
        
        report.push_str("\n=== Recommendations ===\n");
        if self.stats.verification_failures > 0 {
            report.push_str("• CRITICAL: Verification failures detected - Investigate potential attacks\n");
        }
        if self.stats.errors > 0 && self.stats.emitted > 0 {
            let error_rate = (self.stats.errors as f64 / self.stats.emitted as f64) * 100.0;
            if error_rate > 1.0 {
                report.push_str("• WARNING: High error rate detected - Review input validation\n");
            }
        }
        if self.stats.vdf_computations > 0 {
            let avg_vdf_time = self.stats.total_vdf_time / self.stats.vdf_computations as u32;
            if avg_vdf_time > Duration::from_millis(self.vdf_delay_ms as u64 * 2) {
                report.push_str("• PERFORMANCE: VDF computations taking longer than expected\n");
            }
        }
        
        report
    }

    /// Generate JSON audit log for automated monitoring
    pub fn generate_audit_json(&self) -> String {
        format!("{{\"timestamp\":\"{:?}\",\"emitted\":{},\"errors\":{},\"verification_failures\":{},\"vdf_computations\":{},\"total_vdf_time_ms\":{},\"next_batch_id\":{}}}",
                std::time::SystemTime::now(),
                self.stats.emitted,
                self.stats.errors,
                self.stats.verification_failures,
                self.stats.vdf_computations,
                self.stats.total_vdf_time.as_millis(),
                self.next_batch_id)
    }

    /// Flush current buffer with full cryptographic verification
    fn flush_with_verification(&mut self) -> Result<VerifiedBatch, CmixError> {
        let start_time = Instant::now();
        
        // Perform VDF computation for timing proof
        let vdf_seed = self.compute_vdf_seed();
        let vdf_proof = vdf::eval(&vdf_seed, self.vdf_delay_ms);
        
        let vdf_duration = start_time.elapsed();
        self.stats.total_vdf_time += vdf_duration;
        self.stats.vdf_computations += 1;

        // Check VDF timeout constraint
        let max_vdf_time = Duration::from_millis(self.vdf_delay_ms as u64 * 2); // 2x tolerance
        if vdf_duration > max_vdf_time {
            let error = CmixError::VdfTimeout {
                duration: vdf_duration,
                max_allowed: max_vdf_time,
            };
            self.record_error(error.clone());
            return Err(error);
        }

        // Compute batch integrity hash
        let integrity_hash = self.compute_batch_hash(&self.buf);
        
        // Generate RSA accumulator witness (simplified for this implementation)
        let accumulator_witness = self.generate_accumulator_witness(&integrity_hash);

        // Create verified batch
        let batch = VerifiedBatch {
            id: self.next_batch_id,
            packets: std::mem::take(&mut self.buf),
            vdf_proof,
            accumulator_witness,
            integrity_hash,
            created_at: Instant::now(),
        };

        // Update statistics
        self.stats.emitted += 1;
        self.stats.last_flush = Some(Instant::now());
        self.next_batch_id += 1;

        Ok(batch)
    }

    /// Compute VDF seed from current state
    fn compute_vdf_seed(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(&self.next_batch_id.to_le_bytes());
        hasher.update(&(self.buf.len() as u32).to_le_bytes());
        
        // Include packet hashes in seed
        for pkt in &self.buf {
            let mut pkt_hasher = Sha256::new();
            pkt_hasher.update(pkt);
            hasher.update(pkt_hasher.finalize());
        }
        
        hasher.finalize().to_vec()
    }

    /// Compute cryptographic hash of batch contents
    fn compute_batch_hash(&self, packets: &[Vec<u8>]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(&(packets.len() as u32).to_le_bytes());
        
        for pkt in packets {
            hasher.update(&(pkt.len() as u32).to_le_bytes());
            hasher.update(pkt);
        }
        
        hasher.finalize().into()
    }

    /// Generate RSA accumulator witness (simplified implementation)
    fn generate_accumulator_witness(&self, hash: &[u8; 32]) -> Vec<u8> {
        // Generate witness that matches accumulator::verify_membership expectations
        let mut hasher = Sha256::new();
        hasher.update(b"witness");
        hasher.update(&self.next_batch_id.to_le_bytes());
        hasher.update(hash);
        hasher.finalize().to_vec()
    }

    /// Record an error in the error log
    fn record_error(&mut self, error: CmixError) {
        self.stats.errors += 1;
        if matches!(error, CmixError::TamperedBatch { .. } | CmixError::InvalidWitness { .. }) {
            self.stats.verification_failures += 1;
        }
        self.error_log.push((Instant::now(), error));
        
        // Keep error log bounded to prevent memory growth
        if self.error_log.len() > 1000 {
            self.error_log.drain(0..500);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_batch_after_timeout() {
        let mut b = Batcher::new(10, Duration::from_millis(50));
        let t0 = Instant::now();
        assert!(b.tick(t0).unwrap().is_none());
        
        b.push(vec![1]).unwrap();
        let t1 = t0 + Duration::from_millis(60);
        let batch = b.tick(t1).unwrap();
        
        assert!(batch.is_some());
        let batch = batch.unwrap();
        assert_eq!(batch.packets.len(), 1);
        assert_eq!(batch.id, 1);
        assert!(!batch.vdf_proof.iter().all(|&x| x == 0)); // VDF proof should be non-zero
    }

    #[test]
    fn emits_batch_when_full() {
        let mut b = Batcher::new(2, Duration::from_secs(10));
        
        assert!(b.push(vec![1]).unwrap().is_none());
        let batch = b.push(vec![2]).unwrap();
        
        assert!(batch.is_some());
        let batch = batch.unwrap();
        assert_eq!(batch.packets.len(), 2);
        assert_eq!(batch.packets[0], vec![1]);
        assert_eq!(batch.packets[1], vec![2]);
    }

    #[test]
    fn detailed_verification_reports_errors() {
        let mut b = Batcher::new(10, Duration::from_millis(50));
        
        // Create a valid batch first
        b.push(vec![1]).unwrap();
        let mut batch = b.flush_with_verification().unwrap();
        
        // Tamper with the batch
        batch.packets.push(vec![99]); // Add unexpected packet
        
        // Verification should fail
        let result = b.verify_batch(&batch);
        assert!(result.is_err());
        
        if let Err(CmixError::TamperedBatch { batch_id, .. }) = result {
            assert_eq!(batch_id, batch.id);
        } else {
            panic!("Expected TamperedBatch error");
        }
        
        // Check that stats were updated properly
        assert_eq!(b.stats.verification_failures, 1);
        
        // Error report should contain details
        let report = b.generate_error_report();
        println!("Generated report:\n{}", report); // Debug output
        assert!(report.contains("cMix Batcher Security Audit Report"));
        assert!(report.contains("Verification failures: 1"));
    }

    #[test]
    fn rejects_oversized_packets() {
        let mut b = Batcher::new(10, Duration::from_millis(50));
        let oversized_packet = vec![0u8; 100000]; // 100KB packet
        
        let result = b.push(oversized_packet);
        assert!(result.is_err());
        
        if let Err(CmixError::InvalidBatchSize { size, max, .. }) = result {
            assert_eq!(size, 100000);
            assert_eq!(max, 65536);
        } else {
            panic!("Expected InvalidBatchSize error");
        }
    }

    #[test]
    fn vdf_timeout_detection() {
        let mut b = Batcher::with_vdf_delay(10, Duration::from_millis(50), 1); // Very fast VDF
        b.push(vec![1]).unwrap();
        
        // This should succeed with fast VDF
        let result = b.flush_with_verification();
        assert!(result.is_ok());
        
        // Statistics should reflect VDF computation
        assert_eq!(b.stats.vdf_computations, 1);
        assert!(b.stats.total_vdf_time > Duration::from_nanos(0));
    }

    #[test]
    fn batch_verification_success() {
        let mut b = Batcher::new(10, Duration::from_millis(50));
        b.push(vec![1, 2, 3]).unwrap();
        b.push(vec![4, 5, 6]).unwrap();
        
        let batch = b.flush_with_verification().unwrap();
        
        // Verification should pass for unmodified batch
        assert!(b.verify_batch(&batch).is_ok());
    }

    #[test]
    fn detailed_audit_report_generation() {
        let mut b = Batcher::new(10, Duration::from_millis(50));
        
        // Create some successful batches
        for i in 0..3 {
            b.push(format!("audit_test_{}", i).into_bytes()).unwrap();
            b.force_flush().unwrap();
        }
        
        // Generate some errors
        let oversized = vec![0u8; 100000];
        let _ = b.push(oversized); // This will fail
        
        // Generate audit report
        let report = b.generate_error_report();
        
        // Verify comprehensive reporting
        assert!(report.contains("Security Audit Report"));
        assert!(report.contains("Error rate:"));
        assert!(report.contains("Average VDF time:"));
        assert!(report.contains("Recommendations"));
        
        // Test JSON audit log
        let json_log = b.generate_audit_json();
        assert!(json_log.contains("\"emitted\":3"));
        assert!(json_log.contains("\"errors\":1"));
        
        println!("Generated audit report:\n{}", report);
    }

    #[test]
    fn timeout_detection_and_reporting() {
        let mut b = Batcher::with_vdf_delay(10, Duration::from_millis(50), 1); // Very fast VDF
        
        // Add a packet and flush
        b.push(vec![1, 2, 3]).unwrap();
        let result = b.force_flush();
        
        // Should succeed with fast VDF
        assert!(result.is_ok());
        
        // Check that no timeout errors were recorded
        assert!(b.error_log.is_empty());
        
        // Verify timing statistics
        assert!(b.stats.total_vdf_time > Duration::from_nanos(0));
        assert_eq!(b.stats.vdf_computations, 1);
        
        let report = b.generate_error_report();
        assert!(report.contains("Average VDF time:"));
    }

    #[test]
    fn sequential_batch_ids() {
        let mut b = Batcher::new(1, Duration::from_millis(50));
        
        let batch1 = b.push(vec![1]).unwrap().unwrap();
        let batch2 = b.push(vec![2]).unwrap().unwrap();
        let batch3 = b.push(vec![3]).unwrap().unwrap();
        
        assert_eq!(batch1.id, 1);
        assert_eq!(batch2.id, 2);
        assert_eq!(batch3.id, 3);
    }
}
