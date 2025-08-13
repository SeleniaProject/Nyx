#![forbid(unsafe_code)]

//! Integration layer for zero-copy optimization with existing Nyx components.
//!
//! This module provides integration points between the zero-copy optimization
//! system and existing Nyx protocol components (AEAD, FEC, transmission).
//! It implements adaptation layers and enhanced versions of existing
//! interfaces to support zero-copy operations.

use super::*;
use crate::zero_copy::manager::{ZeroCopyManager, ZeroCopyError};

// AEAD は実装を必須とし、モックは廃止。暗号機能が無効なビルドでは本統合APIの一部を非公開化する。
#[cfg(feature = "nyx-crypto")]
pub use nyx_crypto::aead::{FrameCrypter, AeadError};
use std::sync::Arc;

/// Integration with AEAD encryption for zero-copy optimization
#[cfg(feature = "nyx-crypto")]
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
            // Ensure critical path exists, then fetch for tracking
            let path = match self.manager.get_critical_path(&self.path_id).await {
                Some(p) => p,
                None => {
                    // Attempt to create the path on-demand
                    let _ = self.manager.create_critical_path(self.path_id.clone()).await
                        .map_err(|_| AeadError::InvalidTag)?;
                    self.manager.get_critical_path(&self.path_id).await
                        .ok_or_else(|| AeadError::InvalidTag)?
                }
            };

            // Track allocation
            path.record_allocation(AllocationEvent {
                stage: Stage::Crypto,
                operation: OperationType::Allocate,
                size: plaintext.len() + 16, // AEAD tag overhead
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            // Perform encryption
            let result = self.crypter.encrypt(dir, plaintext, aad)?;

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

/// Integration with FEC for zero-copy optimization
pub mod fec_integration {
    use super::*;
    use std::sync::Arc;
    use std::error::Error;

    /// Abstraction over FEC codec to avoid circular dependencies with `nyx-fec`.
    /// Implement this trait in `nyx-fec` (or other FEC crates) to integrate with `nyx-core`.
    pub trait FecCodec: Send + Sync {
        /// Encode input data into FEC symbols represented as raw byte vectors.
        fn encode(&self, data: &[u8]) -> Vec<Vec<u8>>;

        /// Decode FEC symbols back to original bytes. Default returns unsupported.
        fn decode(&self, _symbols: &[Vec<u8>]) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
            Err("FEC decode not supported by this adapter".into())
        }

        /// Current redundancy ratio (0.0..=1.0) for telemetry purposes.
        fn current_redundancy(&self) -> f32 { 0.0 }
    }

    /// Zero-copy enhanced FEC codec wrapper that records allocation/copy metrics.
    pub struct ZeroCopyFecCodec {
        /// Underlying codec (trait object to break crate cycles)
        codec: Arc<dyn FecCodec>,
        /// Zero-copy manager reference
        manager: Arc<ZeroCopyManager>,
        /// Critical path ID
        path_id: String,
    }

    impl ZeroCopyFecCodec {
        /// Create new zero-copy FEC codec wrapper
        pub fn new(codec: Arc<dyn FecCodec>, manager: Arc<ZeroCopyManager>, path_id: String) -> Self {
            Self { codec, manager, path_id }
        }

        /// Encode with zero-copy optimization tracking
        pub async fn encode_zero_copy(
            &self,
            context_id: &str,
            data: &[u8],
        ) -> Result<Vec<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
            // Ensure critical path exists, then fetch for tracking
            let path = match self.manager.get_critical_path(&self.path_id).await {
                Some(p) => p,
                None => {
                    self.manager.create_critical_path(self.path_id.clone()).await
                        .map_err(|_| "Critical path creation failed")?;
                    self.manager.get_critical_path(&self.path_id).await
                        .ok_or("Critical path not found")?
                }
            };

            // Calculate nominal accounting parameters based on MTU-sized symbols (1280 bytes)
            let symbol_size = 1280usize;
            let num_source_symbols = (data.len() + symbol_size - 1) / symbol_size;

            // Perform encoding
            let encoded_symbols = self.codec.encode(data);

            // Track individual symbol allocations
            for (i, symbol) in encoded_symbols.iter().enumerate() {
                let operation = if i < num_source_symbols {
                    OperationType::ZeroCopy
                } else {
                    OperationType::Copy
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
            // Ensure critical path exists, then fetch for tracking
            let path = match self.manager.get_critical_path(&self.path_id).await {
                Some(p) => p,
                None => {
                    self.manager.create_critical_path(self.path_id.clone()).await
                        .map_err(|_| "Critical path creation failed")?;
                    self.manager.get_critical_path(&self.path_id).await
                        .ok_or("Critical path not found")?
                }
            };

            // Track FEC decoding allocation
            let total_symbol_bytes: usize = symbols.iter().map(|s| s.len()).sum();
            path.record_allocation(AllocationEvent {
                stage: Stage::Fec,
                operation: OperationType::Allocate,
                size: total_symbol_bytes,
                timestamp: Instant::now(),
                context: Some(context_id.to_string()),
            }).await;

            // Perform decoding through codec
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
    }
}

/// Integration with transmission layer for zero-copy optimization
pub mod transmission_integration {
    use super::*;
    use std::net::SocketAddr;
    
    use tokio::net::UdpSocket;

    /// Zero-copy enhanced transmission handler
    pub struct ZeroCopyTransmissionHandler {
        /// UDP socket for transmission (tokio::net::UdpSocket)
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
            // Bind actual UDP socket via Tokio. This enables kernel-level zero-copy paths
            // (e.g., gather I/O) where supported and integrates with our async runtime.
            let socket = Arc::new(UdpSocket::bind(bind_addr).await?);
            // Ensure the critical path exists before first use of this handler.
            if manager.get_critical_path(&path_id).await.is_none() {
                // Best-effort; if creation fails due to race, subsequent get will succeed.
                let _ = manager.create_critical_path(path_id.clone()).await;
            }
            Ok(Self { socket, manager, path_id })
        }

        /// Send data with zero-copy optimization tracking
        pub async fn send_zero_copy(
            &self,
            context_id: &str,
            data: &[u8],
            target: SocketAddr,
        ) -> Result<usize, std::io::Error> {
            // Ensure critical path exists for tracking
            let path = match self.manager.get_critical_path(&self.path_id).await {
                Some(p) => p,
                None => {
                    // Attempt creation then retry fetch
                    if self.manager.create_critical_path(self.path_id.clone()).await.is_err() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "Critical path not available"
                        ));
                    }
                    self.manager.get_critical_path(&self.path_id).await.ok_or_else(||
                        std::io::Error::new(std::io::ErrorKind::NotFound, "Critical path not found")
                    )?
                }
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
            // Ensure critical path exists for tracking
            let path = match self.manager.get_critical_path(&self.path_id).await {
                Some(p) => p,
                None => {
                    if self.manager.create_critical_path(self.path_id.clone()).await.is_err() {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "Critical path not available"
                        ));
                    }
                    self.manager.get_critical_path(&self.path_id).await.ok_or_else(||
                        std::io::Error::new(std::io::ErrorKind::NotFound, "Critical path not found")
                    )?
                }
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
    #[cfg(feature = "nyx-crypto")]
    aead_crypter: Option<aead_integration::ZeroCopyAeadCrypter>,
    fec_codec: Option<fec_integration::ZeroCopyFecCodec>,
    transmission_handler: Option<transmission_integration::ZeroCopyTransmissionHandler>,
}

impl ZeroCopyPipeline {
    /// Create new zero-copy pipeline
    pub fn new(manager: Arc<ZeroCopyManager>, path_id: String) -> Self {
        Self {
            manager,
            path_id,
            #[cfg(feature = "nyx-crypto")]
            aead_crypter: None,
            fec_codec: None,
            transmission_handler: None,
        }
    }

    /// Initialize AEAD component
    #[cfg(feature = "nyx-crypto")]
    pub fn with_aead(mut self, crypter: FrameCrypter) -> Self {
        self.aead_crypter = Some(aead_integration::ZeroCopyAeadCrypter::new(
            crypter,
            Arc::clone(&self.manager),
            self.path_id.clone(),
        ));
        self
    }

    /// Initialize FEC component
    pub fn with_fec(mut self, codec: Arc<dyn fec_integration::FecCodec>) -> Self {
        self.fec_codec = Some(fec_integration::ZeroCopyFecCodec::new(
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
        let encrypted_data = {
            #[cfg(feature = "nyx-crypto")]
            {
                if let Some(ref mut aead) = self.aead_crypter {
                    aead.encrypt_zero_copy(&context_id, 0x00000000, packet_data, &[]).await?
                } else {
                    packet_data.to_vec()
                }
            }
            #[cfg(not(feature = "nyx-crypto"))]
            {
                // AEAD is disabled at compile time; pass-through
                packet_data.to_vec()
            }
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
