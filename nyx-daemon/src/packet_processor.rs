//! Extended Packet Format End-to-End Processing
//!
//! This module implements the send and receive paths for Extended Packet Format
//! as specified in `spec/Nyx_Protocol_v1.0_Spec_EN.md` Section 7.
//!
//! # Responsibilities
//! - Encoding outbound packets with proper headers
//! - Decoding inbound packets with validation
//! - Packet boundary padding for traffic analysis resistance
//! - Integration with Connection Manager and Stream Manager

use nyx_stream::extended_packet::{
    ConnectionId, ExtendedPacket, ExtendedPacketHeader, PacketFlags, PacketType, PathId,
    EXTENDED_HEADER_SIZE, MAX_PAYLOAD_SIZE,
};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, trace, warn};

/// Packet processor errors
#[derive(Debug, Error)]
pub enum PacketProcessorError {
    #[error("Connection not found: {0}")]
    ConnectionNotFound(u64),
    #[error("Invalid packet: {0}")]
    InvalidPacket(String),
    #[error("Packet too large: {0} bytes (max {1})")]
    PacketTooLarge(usize, usize),
    #[error("Encoding error: {0}")]
    EncodingError(String),
    #[error("Decoding error: {0}")]
    DecodingError(String),
}

/// Packet processor configuration
#[derive(Debug, Clone)]
pub struct PacketProcessorConfig {
    /// Enable packet padding for traffic analysis resistance
    pub enable_padding: bool,
    /// Minimum padded packet size (bytes)
    pub min_padded_size: usize,
    /// Maximum packet queue size per connection
    pub max_queue_size: usize,
}

impl Default for PacketProcessorConfig {
    fn default() -> Self {
        Self {
            enable_padding: true,
            min_padded_size: 256, // Minimum 256 bytes to hide packet sizes
            max_queue_size: 1000,
        }
    }
}

/// Connection-level packet state
struct ConnectionPacketState {
    cid: ConnectionId,
    default_path_id: PathId,
    send_count: u64,
    recv_count: u64,
}

/// Packet processor for Extended Packet Format
pub struct PacketProcessor {
    config: PacketProcessorConfig,
    connections: Arc<RwLock<HashMap<u64, ConnectionPacketState>>>,
}

impl PacketProcessor {
    /// Create new packet processor
    pub fn new(config: PacketProcessorConfig) -> Self {
        Self {
            config,
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register connection for packet processing
    pub async fn register_connection(&self, conn_id: u64, cid: ConnectionId, path_id: PathId) {
        let mut conns = self.connections.write().await;
        conns.insert(
            conn_id,
            ConnectionPacketState {
                cid,
                default_path_id: path_id,
                send_count: 0,
                recv_count: 0,
            },
        );
        debug!("Registered connection {} with CID {}", conn_id, cid);
    }

    /// Unregister connection
    pub async fn unregister_connection(&self, conn_id: u64) {
        let mut conns = self.connections.write().await;
        if conns.remove(&conn_id).is_some() {
            debug!("Unregistered connection {}", conn_id);
        }
    }

    /// Encode outbound packet (Send Path)
    ///
    /// This is called before transmitting data over the network.
    /// It constructs the Extended Packet Header and applies padding if enabled.
    pub async fn encode_packet(
        &self,
        conn_id: u64,
        packet_type: PacketType,
        flags: PacketFlags,
        path_id: Option<PathId>,
        payload: Vec<u8>,
    ) -> Result<bytes::Bytes, PacketProcessorError> {
        // Get connection state
        let mut conns = self.connections.write().await;
        let state = conns
            .get_mut(&conn_id)
            .ok_or(PacketProcessorError::ConnectionNotFound(conn_id))?;

        // Validate payload size
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(PacketProcessorError::PacketTooLarge(
                payload.len(),
                MAX_PAYLOAD_SIZE,
            ));
        }

        // Apply padding if enabled
        let padded_payload = if self.config.enable_padding {
            self.apply_padding(payload)
        } else {
            payload
        };

        // Use provided path_id or default
        let effective_path_id = path_id.unwrap_or(state.default_path_id);

        // Construct header
        let header = ExtendedPacketHeader::new(
            state.cid,
            packet_type,
            flags,
            effective_path_id,
            padded_payload.len() as u16,
        )
        .map_err(|e| PacketProcessorError::EncodingError(e.to_string()))?;

        // Create packet
        let packet = ExtendedPacket::new(header, padded_payload)
            .map_err(|e| PacketProcessorError::EncodingError(e.to_string()))?;

        // Encode to bytes
        let encoded = packet
            .encode()
            .map_err(|e| PacketProcessorError::EncodingError(e.to_string()))?;

        // Update stats
        state.send_count += 1;

        trace!(
            "Encoded packet for conn {} (CID {}, path {}, type {:?}, {} bytes)",
            conn_id,
            state.cid,
            effective_path_id,
            packet_type,
            encoded.len()
        );

        Ok(encoded)
    }

    /// Decode inbound packet (Receive Path)
    ///
    /// This is called after receiving data from the network.
    /// It validates the Extended Packet Header and extracts the payload.
    pub async fn decode_packet(
        &self,
        raw_bytes: bytes::Bytes,
    ) -> Result<DecodedPacket, PacketProcessorError> {
        // Decode packet
        let packet = ExtendedPacket::decode(raw_bytes)
            .map_err(|e| PacketProcessorError::DecodingError(e.to_string()))?;

        // Extract header fields
        let cid = packet.header.cid;
        let packet_type = packet.header.packet_type;
        let flags = packet.header.flags;
        let path_id = packet.header.path_id;

        // Remove padding if present
        let unpadded_payload = if self.config.enable_padding {
            self.remove_padding(packet.payload)
        } else {
            packet.payload
        };

        // Find connection by CID
        let mut conns = self.connections.write().await;
        if let Some((_conn_id, state)) = conns.iter_mut().find(|(_, s)| s.cid == cid) {
            state.recv_count += 1;
            trace!(
                "Decoded packet for CID {} (path {}, type {:?}, {} bytes)",
                cid,
                path_id,
                packet_type,
                unpadded_payload.len()
            );
        } else {
            warn!("Received packet for unknown CID {}", cid);
        }

        Ok(DecodedPacket {
            cid,
            packet_type,
            flags,
            path_id,
            payload: unpadded_payload,
        })
    }

    /// Apply packet boundary padding
    ///
    /// Pads payload to min_padded_size using PKCS#7-style padding.
    /// Format: [data...][padding...][padding_length (1 byte)]
    fn apply_padding(&self, mut payload: Vec<u8>) -> Vec<u8> {
        let current_size = payload.len();
        if current_size >= self.config.min_padded_size {
            // No padding needed
            payload.push(0); // 0 bytes of padding
            return payload;
        }

        let padding_needed = self.config.min_padded_size - current_size - 1; // -1 for padding_length byte
        payload.resize(current_size + padding_needed, 0);
        payload.push(padding_needed as u8);

        trace!("Applied {} bytes padding", padding_needed);
        payload
    }

    /// Remove packet boundary padding
    ///
    /// Extracts original payload by reading padding_length from last byte.
    fn remove_padding(&self, payload: Vec<u8>) -> Vec<u8> {
        if payload.is_empty() {
            return payload;
        }

        let padding_length = *payload.last().unwrap() as usize;
        let original_length = payload.len().saturating_sub(padding_length + 1);

        if padding_length > 0 {
            trace!("Removed {} bytes padding", padding_length);
        }

        payload[..original_length].to_vec()
    }

    /// Get connection statistics
    pub async fn get_stats(&self, conn_id: u64) -> Option<PacketStats> {
        let conns = self.connections.read().await;
        conns.get(&conn_id).map(|state| PacketStats {
            send_count: state.send_count,
            recv_count: state.recv_count,
        })
    }

    /// List all registered connections
    pub async fn list_connections(&self) -> Vec<u64> {
        let conns = self.connections.read().await;
        conns.keys().copied().collect()
    }
}

/// Decoded packet structure
#[derive(Debug, Clone)]
pub struct DecodedPacket {
    pub cid: ConnectionId,
    pub packet_type: PacketType,
    pub flags: PacketFlags,
    pub path_id: PathId,
    pub payload: Vec<u8>,
}

/// Packet statistics
#[derive(Debug, Clone)]
pub struct PacketStats {
    pub send_count: u64,
    pub recv_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_cid() -> ConnectionId {
        ConnectionId::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12])
    }

    #[tokio::test]
    async fn test_connection_lifecycle() {
        let processor = PacketProcessor::new(PacketProcessorConfig::default());
        let cid = create_test_cid();

        // Register
        processor.register_connection(1, cid, PathId(0)).await;
        let conns = processor.list_connections().await;
        assert_eq!(conns, vec![1]);

        // Unregister
        processor.unregister_connection(1).await;
        let conns = processor.list_connections().await;
        assert!(conns.is_empty());
    }

    #[tokio::test]
    async fn test_encode_decode_roundtrip() {
        let processor = PacketProcessor::new(PacketProcessorConfig {
            enable_padding: false, // Disable padding for exact comparison
            ..Default::default()
        });
        let cid = create_test_cid();
        processor.register_connection(1, cid, PathId(0)).await;

        // Encode
        let payload = b"Hello, Nyx!".to_vec();
        let encoded = processor
            .encode_packet(
                1,
                PacketType::Application,
                PacketFlags::default(),
                None,
                payload.clone(),
            )
            .await
            .unwrap();

        // Decode
        let decoded = processor.decode_packet(encoded).await.unwrap();

        assert_eq!(decoded.cid, cid);
        assert_eq!(decoded.packet_type, PacketType::Application);
        assert_eq!(decoded.path_id, PathId(0));
        assert_eq!(decoded.payload, payload);
    }

    #[tokio::test]
    async fn test_padding_applied() {
        let processor = PacketProcessor::new(PacketProcessorConfig {
            enable_padding: true,
            min_padded_size: 256,
            ..Default::default()
        });
        let cid = create_test_cid();
        processor.register_connection(1, cid, PathId(0)).await;

        // Encode small payload
        let payload = b"Small".to_vec();
        let encoded = processor
            .encode_packet(1, PacketType::Application, PacketFlags::default(), None, payload)
            .await
            .unwrap();

        // Check total size includes padding
        assert!(encoded.len() >= 256 + EXTENDED_HEADER_SIZE);
    }

    #[tokio::test]
    async fn test_padding_roundtrip() {
        let processor = PacketProcessor::new(PacketProcessorConfig {
            enable_padding: true,
            min_padded_size: 256,
            ..Default::default()
        });
        let cid = create_test_cid();
        processor.register_connection(1, cid, PathId(0)).await;

        // Encode
        let original_payload = b"Test data with padding".to_vec();
        let encoded = processor
            .encode_packet(
                1,
                PacketType::Application,
                PacketFlags::default(),
                None,
                original_payload.clone(),
            )
            .await
            .unwrap();

        // Decode
        let decoded = processor.decode_packet(encoded).await.unwrap();

        // Original payload should be recovered exactly
        assert_eq!(decoded.payload, original_payload);
    }

    #[tokio::test]
    async fn test_multipath_path_id() {
        let processor = PacketProcessor::new(PacketProcessorConfig::default());
        let cid = create_test_cid();
        processor.register_connection(1, cid, PathId(0)).await;

        // Encode with explicit path_id
        let payload = b"Multipath data".to_vec();
        let encoded = processor
            .encode_packet(
                1,
                PacketType::Application,
                PacketFlags::default(),
                Some(PathId(3)),
                payload,
            )
            .await
            .unwrap();

        // Decode and verify path_id
        let decoded = processor.decode_packet(encoded).await.unwrap();
        assert_eq!(decoded.path_id, PathId(3));
    }

    #[tokio::test]
    async fn test_packet_too_large() {
        let processor = PacketProcessor::new(PacketProcessorConfig::default());
        let cid = create_test_cid();
        processor.register_connection(1, cid, PathId(0)).await;

        // Try to encode oversized payload
        let oversized_payload = vec![0u8; MAX_PAYLOAD_SIZE + 1];
        let result = processor
            .encode_packet(
                1,
                PacketType::Application,
                PacketFlags::default(),
                None,
                oversized_payload,
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PacketProcessorError::PacketTooLarge(_, _)
        ));
    }

    #[tokio::test]
    async fn test_stats_tracking() {
        let processor = PacketProcessor::new(PacketProcessorConfig::default());
        let cid = create_test_cid();
        processor.register_connection(1, cid, PathId(0)).await;

        // Send multiple packets
        for _ in 0..5 {
            let _ = processor
                .encode_packet(
                    1,
                    PacketType::Application,
                    PacketFlags::default(),
                    None,
                    b"test".to_vec(),
                )
                .await;
        }

        // Check stats
        let stats = processor.get_stats(1).await.unwrap();
        assert_eq!(stats.send_count, 5);
    }

    #[tokio::test]
    async fn test_connection_not_found() {
        let processor = PacketProcessor::new(PacketProcessorConfig::default());

        // Try to encode for non-existent connection
        let result = processor
            .encode_packet(
                999,
                PacketType::Application,
                PacketFlags::default(),
                None,
                b"test".to_vec(),
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PacketProcessorError::ConnectionNotFound(999)
        ));
    }
}
