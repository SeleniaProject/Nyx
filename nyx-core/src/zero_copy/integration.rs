#![forbid(unsafe_code)]

//! Integration layer for zero-copy optimization with existing Nyx components.
//!
//! This module provides integration points between the zero-copy optimization
//! system and existing Nyx protocol components (AEAD, FEC, transmission).
//! It implements adaptation layers and enhanced versions of existing
//! interfaces to support zero-copy operations.

use super::*;
use crate::zero_copy::manager::{ZeroCopyManager, ZeroCopyError};

// Conditional imports - only available with respective features
#[cfg(feature = "nyx-crypto")]
pub use nyx_crypto::aead::{FrameCrypter, AeadError};

// Mock crypto for testing without crypto feature
#[cfg(not(feature = "nyx-crypto"))]
mod mock_crypto {
    #[derive(Clone)]
    pub struct FrameCrypter;
    
    #[derive(Debug, thiserror::Error)]
    pub enum AeadError {
        #[error("Invalid tag")]
        InvalidTag,
        #[error("Decryption failed")]
        DecryptionFailed,
        #[error("Other error")]
        Other,
    }
    
    impl FrameCrypter {
        pub fn encrypt(&mut self, _dir: u32, plaintext: &[u8], _aad: &[u8]) -> Vec<u8> {
            let mut result = plaintext.to_vec();
            result.extend_from_slice(&[0u8; 16]); // Mock tag
            result
        }
        
        pub fn decrypt(&mut self, _dir: u32, _seq: u64, ciphertext: &[u8], _aad: &[u8]) -> Result<Vec<u8>, AeadError> {
            if ciphertext.len() < 16 {
                return Err(AeadError::DecryptionFailed);
            }
            Ok(ciphertext[..ciphertext.len()-16].to_vec())
        }
    }
}

#[cfg(not(feature = "nyx-crypto"))]
pub use mock_crypto::{FrameCrypter, AeadError};
use std::sync::Arc;

/// Integration with AEAD encryption for zero-copy optimization
pub mod aead_integration {
    use super::*;
    use std::sync::Arc;

    /// Zero-copy enhanced AEAD crypter
    pub struct ZeroCopyAeadCrypter {
        /// Underlying crypter
        crypter: FrameCrypter,
        /// Zero-copy manager reference
        manager: Arc<ZeroCopyManager>,
        /// Critical path ID
        path_id: String,
    }

    impl ZeroCopyAeadCrypter {
        /// Create new zero-copy AEAD crypter
        pub fn new(crypter: FrameCrypter, manager: Arc<ZeroCopyManager>, path_id: String) -> Self {
            Self { crypter, manager, path_id }
        }

        /// Encrypt with zero-copy optimization tracking
        pub async fn encrypt_zero_copy(
            &mut self,
            context_id: &str,
            dir: u32,
            plaintext: &[u8],
            aad: &[u8],
        ) -> Result<Vec<u8>, AeadError> {
            // Get critical path for tracking
            let path = self.manager.get_critical_path(&self.path_id).await
                .ok_or_else(|| AeadError::InvalidTag)?; // Map to available error

            // Track allocation
            path.record_allocation(AllocationEvent {
                stage: Stage::Crypto,
                operation: OperationType::Allocate,
                size: plaintext.len() + 16, // AEAD tag overhead
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            // Perform encryption
            let result = self.crypter.encrypt(dir, plaintext, aad);

            // Track completion
            path.record_allocation(AllocationEvent {
                stage: Stage::Crypto,
                operation: OperationType::ZeroCopy,
                size: result.len(),
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            Ok(result)
        }

        /// Decrypt with zero-copy optimization tracking
        pub async fn decrypt_zero_copy(
            &mut self,
            context_id: &str,
            dir: u32,
            seq: u64,
            ciphertext: &[u8],
            aad: &[u8],
        ) -> Result<Vec<u8>, AeadError> {
            // Get critical path for tracking
            let path = self.manager.get_critical_path(&self.path_id).await
                .ok_or_else(|| AeadError::InvalidTag)?;

            // Track allocation
            path.record_allocation(AllocationEvent {
                stage: Stage::Crypto,
                operation: OperationType::Allocate,
                size: ciphertext.len(),
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            // Perform decryption
            let result = self.crypter.decrypt(dir, seq, ciphertext, aad)?;

            // Track completion
            path.record_allocation(AllocationEvent {
                stage: Stage::Crypto,
                operation: OperationType::ZeroCopy,
                size: result.len(),
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            Ok(result)
        }
    }
}

/// Integration with RaptorQ FEC for zero-copy optimization  
pub mod fec_integration {
    use super::*;
    
    // Mock RaptorQ for testing
    #[derive(Clone)]
    pub struct RaptorQCodec {
        redundancy: f32,
    }
    
    #[derive(Debug, Clone, Default)]
    pub struct FECStats {
        pub encoding: EncodingStats,
        pub decoding: DecodingStats, 
        pub current_redundancy: f32,
    }
    
    #[derive(Debug, Clone, Default)]
    pub struct EncodingStats {
        pub total_blocks_encoded: u64,
        pub total_repair_symbols: u64,
    }
    
    #[derive(Debug, Clone, Default)]  
    pub struct DecodingStats {
        pub total_blocks_decoded: u64,
        pub successful_decodings: u64,
    }
    
    impl RaptorQCodec {
        pub fn new(redundancy: f32) -> Self {
            Self { redundancy }
        }
        
        pub fn encode(&self, data: &[u8]) -> Vec<Vec<u8>> {
            let symbol_size = 1280;
            let num_symbols = (data.len() + symbol_size - 1) / symbol_size;
            let mut symbols = Vec::new();
            
            for i in 0..num_symbols {
                let start = i * symbol_size;
                let end = std::cmp::min(start + symbol_size, data.len());
                symbols.push(data[start..end].to_vec());
            }
            
            // Add redundancy symbols (mock)
            let redundancy_symbols = (num_symbols as f32 * self.redundancy) as usize;
            for _ in 0..redundancy_symbols {
                symbols.push(vec![0u8; symbol_size]);
            }
            
            symbols
        }
        
        pub fn decode(&self, symbols: &[Vec<u8>]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
            let mut result = Vec::new();
            for symbol in symbols {
                result.extend_from_slice(symbol);
            }
            Ok(result)
        }
        
        pub fn get_stats(&self) -> FECStats {
            FECStats {
                current_redundancy: self.redundancy,
                ..Default::default()
            }
        }
    }
    use std::sync::Arc;

    /// Zero-copy enhanced RaptorQ codec
    pub struct ZeroCopyRaptorQCodec {
        /// Underlying codec
        codec: RaptorQCodec,
        /// Zero-copy manager reference
        manager: Arc<ZeroCopyManager>,
        /// Critical path ID
        path_id: String,
    }

    impl ZeroCopyRaptorQCodec {
        /// Create new zero-copy RaptorQ codec
        pub fn new(codec: RaptorQCodec, manager: Arc<ZeroCopyManager>, path_id: String) -> Self {
            Self { codec, manager, path_id }
        }

        /// Encode with zero-copy optimization tracking
        pub async fn encode_zero_copy(
            &self,
            context_id: &str,
            data: &[u8],
        ) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
            // Get critical path for tracking
            let path = self.manager.get_critical_path(&self.path_id).await
                .ok_or("Critical path not found")?;

            // Calculate encoding parameters
            let symbol_size = 1280; // RaptorQ symbol size
            let num_source_symbols = (data.len() + symbol_size - 1) / symbol_size;
            let redundancy_symbols = (num_source_symbols as f32 * 0.3) as usize; // 30% redundancy
            let total_symbols = num_source_symbols + redundancy_symbols;

            // Track FEC encoding allocation
            path.record_allocation(AllocationEvent {
                stage: Stage::Fec,
                operation: OperationType::Allocate,
                size: total_symbols * symbol_size,
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            // Perform encoding
            let encoded_symbols = self.codec.encode(data);

            // Track individual symbol allocations
            for (i, symbol) in encoded_symbols.iter().enumerate() {
                let operation = if i < num_source_symbols {
                    OperationType::ZeroCopy // Source symbols can be zero-copy
                } else {
                    OperationType::Copy // Repair symbols require computation
                };

                path.record_allocation(AllocationEvent {
                    stage: Stage::Fec,
                    operation,
                    size: symbol.len(),
                    timestamp: Instant::now(),
                    context: Some(context_id.to_string()),
                }).await;
            }

            Ok(encoded_symbols)
        }

        /// Decode with zero-copy optimization tracking
        pub async fn decode_zero_copy(
            &self,
            context_id: &str,
            symbols: &[Vec<u8>],
        ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
            // Get critical path for tracking
            let path = self.manager.get_critical_path(&self.path_id).await
                .ok_or("Critical path not found")?;

            // Track FEC decoding allocation
            let total_symbol_bytes: usize = symbols.iter().map(|s| s.len()).sum();
            path.record_allocation(AllocationEvent {
                stage: Stage::Fec,
                operation: OperationType::Allocate,
                size: total_symbol_bytes,
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            // Perform decoding
            let decoded_data = self.codec.decode(symbols)?;

            // Track zero-copy potential for successful decode
            path.record_allocation(AllocationEvent {
                stage: Stage::Fec,
                operation: OperationType::ZeroCopy,
                size: decoded_data.len(),
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            Ok(decoded_data)
        }

        /// Get codec statistics
        pub fn get_stats(&self) -> FECStats {
            self.codec.get_stats()
        }
    }
}

/// Integration with transmission layer for zero-copy optimization
pub mod transmission_integration {
    use super::*;
    use std::net::SocketAddr;
    
    // Mock UDP socket for testing
    #[derive(Clone)]
    pub struct UdpSocket;
    
    impl UdpSocket {
        pub async fn bind(_addr: SocketAddr) -> Result<Self, std::io::Error> {
            Ok(Self)
        }
        
        pub async fn send_to(&self, data: &[u8], _target: SocketAddr) -> Result<usize, std::io::Error> {
            Ok(data.len())
        }
        
        pub async fn recv_from(&self, buffer: &mut [u8]) -> Result<(usize, SocketAddr), std::io::Error> {
            let bytes_to_copy = std::cmp::min(buffer.len(), 100); // Mock receive 100 bytes
            buffer[..bytes_to_copy].fill(0);
            Ok((bytes_to_copy, "127.0.0.1:8080".parse().unwrap()))
        }
    }

    /// Zero-copy enhanced transmission handler
    pub struct ZeroCopyTransmissionHandler {
        /// UDP socket for transmission
        socket: Arc<UdpSocket>,
        /// Zero-copy manager reference
        manager: Arc<ZeroCopyManager>,
        /// Critical path ID
        path_id: String,
    }

    impl ZeroCopyTransmissionHandler {
        /// Create new zero-copy transmission handler
        pub async fn new(
            bind_addr: SocketAddr,
            manager: Arc<ZeroCopyManager>,
            path_id: String,
        ) -> Result<Self, std::io::Error> {
            let socket = Arc::new(UdpSocket::bind(bind_addr).await?);
            Ok(Self { socket, manager, path_id })
        }

        /// Send data with zero-copy optimization tracking
        pub async fn send_zero_copy(
            &self,
            context_id: &str,
            data: &[u8],
            target: SocketAddr,
        ) -> Result<usize, std::io::Error> {
            // Get critical path for tracking
            let path = match self.manager.get_critical_path(&self.path_id).await {
                Some(path) => path,
                None => return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Critical path not found"
                )),
            };

            // Track transmission allocation
            path.record_allocation(AllocationEvent {
                stage: Stage::Transmission,
                operation: OperationType::ZeroCopy, // UDP send can be zero-copy at kernel level
                size: data.len(),
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            // Perform transmission
            let bytes_sent = self.socket.send_to(data, target).await?;

            // Track completion
            path.record_allocation(AllocationEvent {
                stage: Stage::Transmission,
                operation: OperationType::ZeroCopy,
                size: bytes_sent,
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            Ok(bytes_sent)
        }

        /// Receive data with zero-copy optimization tracking
        pub async fn receive_zero_copy(
            &self,
            context_id: &str,
            buffer: &mut [u8],
        ) -> Result<(usize, SocketAddr), std::io::Error> {
            // Get critical path for tracking
            let path = match self.manager.get_critical_path(&self.path_id).await {
                Some(path) => path,
                None => return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "Critical path not found"
                )),
            };

            // Track reception buffer allocation
            path.record_allocation(AllocationEvent {
                stage: Stage::Transmission,
                operation: OperationType::ZeroCopy, // Direct to provided buffer
                size: buffer.len(),
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            // Perform reception
            let (bytes_received, sender_addr) = self.socket.recv_from(buffer).await?;

            // Track completion
            path.record_allocation(AllocationEvent {
                stage: Stage::Transmission,
                operation: OperationType::ZeroCopy,
                size: bytes_received,
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            Ok((bytes_received, sender_addr))
        }
    }
}

/// High-level integration API for zero-copy pipeline
pub struct ZeroCopyPipeline {
    /// Zero-copy manager
    manager: Arc<ZeroCopyManager>,
    /// Critical path ID
    pub path_id: String,
    /// Integration components
    aead_crypter: Option<aead_integration::ZeroCopyAeadCrypter>,
    fec_codec: Option<fec_integration::ZeroCopyRaptorQCodec>,
    transmission_handler: Option<transmission_integration::ZeroCopyTransmissionHandler>,
}

impl ZeroCopyPipeline {
    /// Create new zero-copy pipeline
    pub fn new(manager: Arc<ZeroCopyManager>, path_id: String) -> Self {
        Self {
            manager,
            path_id,
            aead_crypter: None,
            fec_codec: None,
            transmission_handler: None,
        }
    }

    /// Initialize AEAD component
    pub fn with_aead(mut self, crypter: FrameCrypter) -> Self {
        self.aead_crypter = Some(aead_integration::ZeroCopyAeadCrypter::new(
            crypter,
            Arc::clone(&self.manager),
            self.path_id.clone(),
        ));
        self
    }

    /// Initialize FEC component
    pub fn with_fec(mut self, codec: fec_integration::RaptorQCodec) -> Self {
        self.fec_codec = Some(fec_integration::ZeroCopyRaptorQCodec::new(
            codec,
            Arc::clone(&self.manager),
            self.path_id.clone(),
        ));
        self
    }

    /// Initialize transmission component
    pub async fn with_transmission(
        mut self,
        bind_addr: std::net::SocketAddr,
    ) -> Result<Self, std::io::Error> {
        self.transmission_handler = Some(
            transmission_integration::ZeroCopyTransmissionHandler::new(
                bind_addr,
                Arc::clone(&self.manager),
                self.path_id.clone(),
            ).await?
        );
        Ok(self)
    }

    /// Process packet through complete zero-copy pipeline
    pub async fn process_complete_packet(
        &mut self,
        packet_data: &[u8],
        target_addr: std::net::SocketAddr,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let context_id = format!("pipeline_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());

        // Stage 1: AEAD Encryption
        let encrypted_data = if let Some(ref mut aead) = self.aead_crypter {
            aead.encrypt_zero_copy(&context_id, 0x00000000, packet_data, &[]).await?
        } else {
            packet_data.to_vec()
        };

        // Stage 2: FEC Encoding
        let fec_symbols = if let Some(ref fec) = self.fec_codec {
            fec.encode_zero_copy(&context_id, &encrypted_data).await?
        } else {
            vec![encrypted_data]
        };

        // Stage 3: Transmission
        let mut total_bytes_sent = 0;
        if let Some(ref transmission) = self.transmission_handler {
            for symbol in &fec_symbols {
                let bytes_sent = transmission.send_zero_copy(&context_id, symbol, target_addr).await?;
                total_bytes_sent += bytes_sent;
            }
        }

        Ok(total_bytes_sent)
    }

    /// Get pipeline metrics
    pub async fn get_metrics(&self) -> Result<crate::zero_copy::AllocationMetrics, ZeroCopyError> {
        let path = self.manager.get_critical_path(&self.path_id).await
            .ok_or_else(|| ZeroCopyError::PathNotFound(self.path_id.clone()))?;
        Ok(path.get_metrics().await)
    }
}
