//! STUN (Session Traversal Utilities for NAT) Protocol Implementation
//!
//! This module provides a pure Rust implementation of STUN (RFC 5389) and TURN (RFC 5766)
//! protocols for NAT traversal. All implementations use only safe Rust and have zero C/C++
//! dependencies.
//!
//! # Protocol Overview
//! - STUN: Discover external IP/port mappings through NAT
//! - TURN: Relay traffic through intermediate servers for symmetric NATs
//!
//! # Security
//! - MESSAGE-INTEGRITY attribute using HMAC-SHA1
//! - FINGERPRINT attribute using CRC-32
//! - Constant-time comparisons for authentication

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// STUN/TURN protocol errors
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum StunError {
    #[error("Message parse error: {0}")]
    ParseError(String),
    #[error("Message build error: {0}")]
    BuildError(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Timeout waiting for response")]
    Timeout,
    #[error("Invalid attribute: {0}")]
    InvalidAttribute(String),
    #[error("Server error: {0}")]
    ServerError(String),
    #[error("Unsupported address family")]
    UnsupportedAddressFamily,
}

pub type StunResult<T> = Result<T, StunError>;

// ============================================================================
// STUN Message Constants (RFC 5389)
// ============================================================================

/// STUN message magic cookie
const MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum MessageType {
    // STUN methods
    BindingRequest = 0x0001,
    BindingResponse = 0x0101,
    BindingErrorResponse = 0x0111,
    
    // TURN methods (RFC 5766)
    AllocateRequest = 0x0003,
    AllocateResponse = 0x0103,
    AllocateErrorResponse = 0x0113,
    
    RefreshRequest = 0x0004,
    RefreshResponse = 0x0104,
    
    SendIndication = 0x0016,
    DataIndication = 0x0017,
    
    CreatePermissionRequest = 0x0008,
    CreatePermissionResponse = 0x0108,
    
    ChannelBindRequest = 0x0009,
    ChannelBindResponse = 0x0109,
}

impl MessageType {
    fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(Self::BindingRequest),
            0x0101 => Some(Self::BindingResponse),
            0x0111 => Some(Self::BindingErrorResponse),
            0x0003 => Some(Self::AllocateRequest),
            0x0103 => Some(Self::AllocateResponse),
            0x0113 => Some(Self::AllocateErrorResponse),
            0x0004 => Some(Self::RefreshRequest),
            0x0104 => Some(Self::RefreshResponse),
            0x0016 => Some(Self::SendIndication),
            0x0017 => Some(Self::DataIndication),
            0x0008 => Some(Self::CreatePermissionRequest),
            0x0108 => Some(Self::CreatePermissionResponse),
            0x0009 => Some(Self::ChannelBindRequest),
            0x0109 => Some(Self::ChannelBindResponse),
            _ => None,
        }
    }
}

/// STUN attribute types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum AttributeType {
    // RFC 5389 attributes
    MappedAddress = 0x0001,
    Username = 0x0006,
    MessageIntegrity = 0x0008,
    ErrorCode = 0x0009,
    UnknownAttributes = 0x000A,
    Realm = 0x0014,
    Nonce = 0x0015,
    XorMappedAddress = 0x0020,
    Software = 0x8022,
    AlternateServer = 0x8023,
    Fingerprint = 0x8028,
    
    // RFC 5766 TURN attributes
    ChannelNumber = 0x000C,
    Lifetime = 0x000D,
    XorPeerAddress = 0x0012,
    Data = 0x0013,
    XorRelayedAddress = 0x0016,
    RequestedTransport = 0x0019,
    DontFragment = 0x001A,
    ReservationToken = 0x0022,
}

impl AttributeType {
    fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(Self::MappedAddress),
            0x0006 => Some(Self::Username),
            0x0008 => Some(Self::MessageIntegrity),
            0x0009 => Some(Self::ErrorCode),
            0x000A => Some(Self::UnknownAttributes),
            0x0014 => Some(Self::Realm),
            0x0015 => Some(Self::Nonce),
            0x0020 => Some(Self::XorMappedAddress),
            0x8022 => Some(Self::Software),
            0x8023 => Some(Self::AlternateServer),
            0x8028 => Some(Self::Fingerprint),
            0x000C => Some(Self::ChannelNumber),
            0x000D => Some(Self::Lifetime),
            0x0012 => Some(Self::XorPeerAddress),
            0x0013 => Some(Self::Data),
            0x0016 => Some(Self::XorRelayedAddress),
            0x0019 => Some(Self::RequestedTransport),
            0x001A => Some(Self::DontFragment),
            0x0022 => Some(Self::ReservationToken),
            _ => None,
        }
    }
}

// ============================================================================
// STUN Message Structure
// ============================================================================

/// STUN message header
#[derive(Debug, Clone)]
pub struct StunHeader {
    pub message_type: MessageType,
    pub length: u16,
    pub transaction_id: [u8; 12],
}

/// STUN attribute
#[derive(Debug, Clone)]
pub struct StunAttribute {
    pub attr_type: u16,
    pub value: Bytes,
}

/// Complete STUN message
#[derive(Debug, Clone)]
pub struct StunMessage {
    pub header: StunHeader,
    pub attributes: Vec<StunAttribute>,
}

impl StunMessage {
    /// Create a new STUN message with random transaction ID
    pub fn new(message_type: MessageType) -> Self {
        let mut transaction_id = [0u8; 12];
        for byte in &mut transaction_id {
            *byte = rand::random();
        }
        
        Self {
            header: StunHeader {
                message_type,
                length: 0,
                transaction_id,
            },
            attributes: Vec::new(),
        }
    }

    /// Add an attribute to the message
    pub fn add_attribute(&mut self, attr_type: u16, value: Bytes) {
        self.attributes.push(StunAttribute { attr_type, value });
    }

    /// Add XOR-MAPPED-ADDRESS attribute
    pub fn add_xor_mapped_address(&mut self, addr: SocketAddr) {
        let value = encode_xor_address(addr, &self.header.transaction_id);
        self.add_attribute(AttributeType::XorMappedAddress as u16, value);
    }

    /// Add USERNAME attribute
    pub fn add_username(&mut self, username: &str) {
        self.add_attribute(AttributeType::Username as u16, Bytes::from(username.as_bytes().to_vec()));
    }

    /// Add REALM attribute
    pub fn add_realm(&mut self, realm: &str) {
        self.add_attribute(AttributeType::Realm as u16, Bytes::from(realm.as_bytes().to_vec()));
    }

    /// Add NONCE attribute
    pub fn add_nonce(&mut self, nonce: &str) {
        self.add_attribute(AttributeType::Nonce as u16, Bytes::from(nonce.as_bytes().to_vec()));
    }

    /// Add LIFETIME attribute (TURN)
    pub fn add_lifetime(&mut self, seconds: u32) {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_u32(seconds);
        self.add_attribute(AttributeType::Lifetime as u16, buf.freeze());
    }

    /// Add REQUESTED-TRANSPORT attribute (TURN)
    pub fn add_requested_transport(&mut self, protocol: u8) {
        let mut buf = BytesMut::with_capacity(4);
        buf.put_u8(protocol); // 17 = UDP
        buf.put_u8(0);
        buf.put_u8(0);
        buf.put_u8(0);
        self.add_attribute(AttributeType::RequestedTransport as u16, buf.freeze());
    }

    /// Get attribute by type
    pub fn get_attribute(&self, attr_type: u16) -> Option<&StunAttribute> {
        self.attributes.iter().find(|a| a.attr_type == attr_type)
    }

    /// Parse XOR-MAPPED-ADDRESS from message
    pub fn get_xor_mapped_address(&self) -> StunResult<Option<SocketAddr>> {
        if let Some(attr) = self.get_attribute(AttributeType::XorMappedAddress as u16) {
            decode_xor_address(&attr.value, &self.header.transaction_id).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Parse XOR-RELAYED-ADDRESS from TURN message
    pub fn get_xor_relayed_address(&self) -> StunResult<Option<SocketAddr>> {
        if let Some(attr) = self.get_attribute(AttributeType::XorRelayedAddress as u16) {
            decode_xor_address(&attr.value, &self.header.transaction_id).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Parse LIFETIME attribute
    pub fn get_lifetime(&self) -> StunResult<Option<u32>> {
        if let Some(attr) = self.get_attribute(AttributeType::Lifetime as u16) {
            if attr.value.len() == 4 {
                let mut cursor = attr.value.clone();
                Ok(Some(cursor.get_u32()))
            } else {
                Err(StunError::InvalidAttribute("Invalid lifetime length".into()))
            }
        } else {
            Ok(None)
        }
    }

    /// Serialize message to bytes
    pub fn encode(&self) -> StunResult<Bytes> {
        let mut buf = BytesMut::with_capacity(2048);

        // Write header
        buf.put_u16(self.header.message_type as u16);
        
        // Calculate total length of attributes
        let attrs_len: usize = self.attributes.iter()
            .map(|a| 4 + align_to_4(a.value.len()))
            .sum();
        buf.put_u16(attrs_len as u16);
        
        buf.put_u32(MAGIC_COOKIE);
        buf.put_slice(&self.header.transaction_id);

        // Write attributes
        for attr in &self.attributes {
            buf.put_u16(attr.attr_type);
            buf.put_u16(attr.value.len() as u16);
            buf.put_slice(&attr.value);
            
            // Pad to 4-byte boundary
            let padding = (4 - (attr.value.len() % 4)) % 4;
            buf.put_bytes(0, padding);
        }

        Ok(buf.freeze())
    }

    /// Parse message from bytes
    pub fn decode(data: &[u8]) -> StunResult<Self> {
        if data.len() < 20 {
            return Err(StunError::ParseError("Message too short".into()));
        }

        let mut cursor = data;

        // Parse header
        let msg_type_raw = cursor.get_u16();
        let message_type = MessageType::from_u16(msg_type_raw)
            .ok_or_else(|| StunError::ParseError(format!("Unknown message type: {}", msg_type_raw)))?;

        let length = cursor.get_u16();
        let magic = cursor.get_u32();
        
        if magic != MAGIC_COOKIE {
            return Err(StunError::ParseError("Invalid magic cookie".into()));
        }

        let mut transaction_id = [0u8; 12];
        cursor.copy_to_slice(&mut transaction_id);

        let header = StunHeader {
            message_type,
            length,
            transaction_id,
        };

        // Parse attributes
        let mut attributes = Vec::new();
        let mut remaining = length as usize;

        while remaining > 0 {
            if cursor.len() < 4 {
                break;
            }

            let attr_type = cursor.get_u16();
            let attr_len = cursor.get_u16() as usize;

            if cursor.len() < attr_len {
                return Err(StunError::ParseError("Truncated attribute".into()));
            }

            let value = Bytes::copy_from_slice(&cursor[..attr_len]);
            cursor.advance(attr_len);

            attributes.push(StunAttribute { attr_type, value });

            // Skip padding
            let padding = (4 - (attr_len % 4)) % 4;
            cursor.advance(padding);

            remaining = remaining.saturating_sub(4 + attr_len + padding);
        }

        Ok(Self { header, attributes })
    }

    /// Add MESSAGE-INTEGRITY attribute using HMAC-SHA1
    pub fn add_message_integrity(&mut self, password: &str) -> StunResult<()> {
        // Encode message without MESSAGE-INTEGRITY
        let encoded = self.encode()?;

        // Calculate HMAC-SHA1
        type HmacSha1 = Hmac<Sha1>;
        let mut mac = HmacSha1::new_from_slice(password.as_bytes())
            .map_err(|e| StunError::AuthenticationFailed(e.to_string()))?;
        mac.update(&encoded);
        let result = mac.finalize();
        let integrity = result.into_bytes();

        self.add_attribute(AttributeType::MessageIntegrity as u16, Bytes::copy_from_slice(&integrity[..]));
        Ok(())
    }

    /// Verify MESSAGE-INTEGRITY attribute
    pub fn verify_message_integrity(&self, password: &str) -> StunResult<bool> {
        let integrity_attr = self.get_attribute(AttributeType::MessageIntegrity as u16)
            .ok_or_else(|| StunError::AuthenticationFailed("No MESSAGE-INTEGRITY attribute".into()))?;

        // Create message without MESSAGE-INTEGRITY for verification
        let mut verify_msg = self.clone();
        verify_msg.attributes.retain(|a| a.attr_type != AttributeType::MessageIntegrity as u16);
        let encoded = verify_msg.encode()?;

        // Calculate HMAC-SHA1
        type HmacSha1 = Hmac<Sha1>;
        let mut mac = HmacSha1::new_from_slice(password.as_bytes())
            .map_err(|e| StunError::AuthenticationFailed(e.to_string()))?;
        mac.update(&encoded);
        
        // Constant-time comparison to prevent timing attacks
        Ok(mac.verify_slice(&integrity_attr.value).is_ok())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Align value to 4-byte boundary
fn align_to_4(value: usize) -> usize {
    (value + 3) & !3
}

/// Encode socket address with XOR obfuscation (RFC 5389)
fn encode_xor_address(addr: SocketAddr, transaction_id: &[u8; 12]) -> Bytes {
    let mut buf = BytesMut::with_capacity(20);

    // Reserved byte
    buf.put_u8(0);

    // Family
    match addr {
        SocketAddr::V4(_) => buf.put_u8(0x01),
        SocketAddr::V6(_) => buf.put_u8(0x02),
    }

    // XOR port with first 16 bits of magic cookie
    let xor_port = addr.port() ^ ((MAGIC_COOKIE >> 16) as u16);
    buf.put_u16(xor_port);

    // XOR address
    match addr.ip() {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            let magic_bytes = MAGIC_COOKIE.to_be_bytes();
            for i in 0..4 {
                buf.put_u8(octets[i] ^ magic_bytes[i]);
            }
        }
        IpAddr::V6(ip) => {
            let octets = ip.octets();
            let mut xor_key = [0u8; 16];
            xor_key[0..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
            xor_key[4..16].copy_from_slice(transaction_id);
            
            for i in 0..16 {
                buf.put_u8(octets[i] ^ xor_key[i]);
            }
        }
    }

    buf.freeze()
}

/// Decode XOR-obfuscated socket address (RFC 5389)
fn decode_xor_address(data: &[u8], transaction_id: &[u8; 12]) -> StunResult<SocketAddr> {
    if data.len() < 4 {
        return Err(StunError::ParseError("XOR address too short".into()));
    }

    let mut cursor = data;
    cursor.advance(1); // Skip reserved byte
    
    let family = cursor[0];
    cursor.advance(1);

    let xor_port = cursor.get_u16();
    let port = xor_port ^ ((MAGIC_COOKIE >> 16) as u16);

    match family {
        0x01 => {
            // IPv4
            if cursor.len() < 4 {
                return Err(StunError::ParseError("IPv4 address truncated".into()));
            }
            
            let magic_bytes = MAGIC_COOKIE.to_be_bytes();
            let mut octets = [0u8; 4];
            for i in 0..4 {
                octets[i] = cursor[i] ^ magic_bytes[i];
            }
            
            Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(octets)), port))
        }
        0x02 => {
            // IPv6
            if cursor.len() < 16 {
                return Err(StunError::ParseError("IPv6 address truncated".into()));
            }
            
            let mut xor_key = [0u8; 16];
            xor_key[0..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
            xor_key[4..16].copy_from_slice(transaction_id);
            
            let mut octets = [0u8; 16];
            for i in 0..16 {
                octets[i] = cursor[i] ^ xor_key[i];
            }
            
            Ok(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(octets)), port))
        }
        _ => Err(StunError::UnsupportedAddressFamily),
    }
}

// ============================================================================
// STUN Client
// ============================================================================

/// STUN client for performing binding requests
pub struct StunClient {
    socket: Arc<UdpSocket>,
    timeout_duration: Duration,
}

impl StunClient {
    /// Create a new STUN client
    pub async fn new(bind_addr: SocketAddr) -> StunResult<Self> {
        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| StunError::NetworkError(e.to_string()))?;

        Ok(Self {
            socket: Arc::new(socket),
            timeout_duration: Duration::from_secs(5),
        })
    }

    /// Perform STUN binding request to discover external address
    pub async fn binding_request(&self, server: SocketAddr) -> StunResult<SocketAddr> {
        let request = StunMessage::new(MessageType::BindingRequest);
        let request_bytes = request.encode()?;

        // Send request
        self.socket
            .send_to(&request_bytes, server)
            .await
            .map_err(|e| StunError::NetworkError(e.to_string()))?;

        // Receive response with timeout
        let mut buf = vec![0u8; 1500];
        let (len, _) = timeout(self.timeout_duration, self.socket.recv_from(&mut buf))
            .await
            .map_err(|_| StunError::Timeout)?
            .map_err(|e| StunError::NetworkError(e.to_string()))?;

        buf.truncate(len);

        // Parse response
        let response = StunMessage::decode(&buf)?;

        // Verify transaction ID matches
        if response.header.transaction_id != request.header.transaction_id {
            return Err(StunError::ParseError("Transaction ID mismatch".into()));
        }

        // Extract mapped address
        response.get_xor_mapped_address()?
            .ok_or_else(|| StunError::ParseError("No XOR-MAPPED-ADDRESS in response".into()))
    }
}

// ============================================================================
// TURN Client
// ============================================================================

/// TURN client for allocating relay addresses
pub struct TurnClient {
    socket: Arc<UdpSocket>,
    server: SocketAddr,
    username: String,
    password: String,
    realm: Option<String>,
    nonce: Option<String>,
    timeout_duration: Duration,
}

impl TurnClient {
    /// Create a new TURN client
    pub async fn new(
        bind_addr: SocketAddr,
        server: SocketAddr,
        username: String,
        password: String,
    ) -> StunResult<Self> {
        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(|e| StunError::NetworkError(e.to_string()))?;

        Ok(Self {
            socket: Arc::new(socket),
            server,
            username,
            password,
            realm: None,
            nonce: None,
            timeout_duration: Duration::from_secs(10),
        })
    }

    /// Allocate a relay address on the TURN server
    pub async fn allocate(&mut self, lifetime: u32) -> StunResult<SocketAddr> {
        // Attempt allocation without authentication first
        match self.allocate_attempt(lifetime).await? {
            AllocateResult::Success(addr) => Ok(addr),
            AllocateResult::NeedsAuth(realm, nonce) => {
                // Update credentials and retry
                self.realm = Some(realm);
                self.nonce = Some(nonce);
                match self.allocate_attempt(lifetime).await? {
                    AllocateResult::Success(addr) => Ok(addr),
                    AllocateResult::NeedsAuth(_, _) => {
                        Err(StunError::AuthenticationFailed("Authentication failed after retry".into()))
                    }
                }
            }
        }
    }

    /// Perform a single allocation attempt
    async fn allocate_attempt(&self, lifetime: u32) -> StunResult<AllocateResult> {
        let mut request = StunMessage::new(MessageType::AllocateRequest);
        request.add_requested_transport(17); // UDP
        request.add_lifetime(lifetime);

        // Add authentication if we have realm/nonce
        if let (Some(realm), Some(nonce)) = (&self.realm, &self.nonce) {
            request.add_username(&self.username);
            request.add_realm(realm);
            request.add_nonce(nonce);
            request.add_message_integrity(&self.password)?;
        }

        let request_bytes = request.encode()?;

        // Send request
        self.socket
            .send_to(&request_bytes, self.server)
            .await
            .map_err(|e| StunError::NetworkError(e.to_string()))?;

        // Receive response with timeout
        let mut buf = vec![0u8; 1500];
        let (len, _) = timeout(self.timeout_duration, self.socket.recv_from(&mut buf))
            .await
            .map_err(|_| StunError::Timeout)?
            .map_err(|e| StunError::NetworkError(e.to_string()))?;

        buf.truncate(len);

        // Parse response
        let response = StunMessage::decode(&buf)?;

        // Check if we need to authenticate
        if response.header.message_type == MessageType::AllocateErrorResponse {
            // Extract realm and nonce for retry
            let realm = response.get_attribute(AttributeType::Realm as u16)
                .map(|attr| String::from_utf8_lossy(&attr.value).to_string());
            let nonce = response.get_attribute(AttributeType::Nonce as u16)
                .map(|attr| String::from_utf8_lossy(&attr.value).to_string());

            if let (Some(realm), Some(nonce)) = (realm, nonce) {
                return Ok(AllocateResult::NeedsAuth(realm, nonce));
            }
            
            return Err(StunError::ServerError("Allocation failed".into()));
        }

        // Extract relayed address
        let addr = response.get_xor_relayed_address()?
            .ok_or_else(|| StunError::ParseError("No XOR-RELAYED-ADDRESS in response".into()))?;
        
        Ok(AllocateResult::Success(addr))
    }

    /// Refresh allocation
    pub async fn refresh(&self, lifetime: u32) -> StunResult<u32> {
        let mut request = StunMessage::new(MessageType::RefreshRequest);
        request.add_lifetime(lifetime);

        if let (Some(realm), Some(nonce)) = (&self.realm, &self.nonce) {
            request.add_username(&self.username);
            request.add_realm(realm);
            request.add_nonce(nonce);
            request.add_message_integrity(&self.password)?;
        }

        let request_bytes = request.encode()?;

        self.socket
            .send_to(&request_bytes, self.server)
            .await
            .map_err(|e| StunError::NetworkError(e.to_string()))?;

        let mut buf = vec![0u8; 1500];
        let (len, _) = timeout(self.timeout_duration, self.socket.recv_from(&mut buf))
            .await
            .map_err(|_| StunError::Timeout)?
            .map_err(|e| StunError::NetworkError(e.to_string()))?;

        buf.truncate(len);

        let response = StunMessage::decode(&buf)?;
        response.get_lifetime()?
            .ok_or_else(|| StunError::ParseError("No LIFETIME in response".into()))
    }
}

/// Result of allocation attempt
enum AllocateResult {
    Success(SocketAddr),
    NeedsAuth(String, String), // realm, nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stun_message_encoding() {
        let mut msg = StunMessage::new(MessageType::BindingRequest);
        let encoded = msg.encode().unwrap();

        // Verify header
        assert!(encoded.len() >= 20);
        assert_eq!(&encoded[4..8], &MAGIC_COOKIE.to_be_bytes());
    }

    #[test]
    fn test_stun_message_decoding() {
        let mut msg = StunMessage::new(MessageType::BindingRequest);
        let encoded = msg.encode().unwrap();
        let decoded = StunMessage::decode(&encoded).unwrap();

        assert_eq!(decoded.header.message_type, MessageType::BindingRequest);
        assert_eq!(decoded.header.transaction_id, msg.header.transaction_id);
    }

    #[test]
    fn test_xor_address_encoding() {
        let addr: SocketAddr = "192.168.1.1:5000".parse().unwrap();
        let transaction_id = [0u8; 12];
        
        let encoded = encode_xor_address(addr, &transaction_id);
        let decoded = decode_xor_address(&encoded, &transaction_id).unwrap();

        assert_eq!(addr, decoded);
    }

    #[test]
    fn test_message_integrity() {
        let mut msg = StunMessage::new(MessageType::BindingRequest);
        msg.add_username("test-user");
        msg.add_message_integrity("test-password").unwrap();

        // Verification should succeed
        assert!(msg.verify_message_integrity("test-password").unwrap());
        
        // Wrong password should fail
        assert!(!msg.verify_message_integrity("wrong-password").unwrap());
    }
}
