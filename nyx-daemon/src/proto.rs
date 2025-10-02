//! Proto Definition Management
//! 
//! This module provides centralized management of Protocol Buffer definitions
//! for the Nyx daemon, including message type conversions and re-exports.

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use prost::Message;

/// Re-export commonly used protobuf types for convenience
pub use prost_types::{Timestamp, Duration as ProtoDuration, Any as ProtoAny};

/// Nyx protocol message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NyxMessage {
    /// Message type identifier
    pub message_type: String,
    /// Timestamp when message was created
    pub timestamp: SystemTime,
    /// Message payload as bytes
    pub payload: Vec<u8>,
    /// Optional message metadata
    pub metadata: HashMap<String, String>,
    /// Message sequence number
    pub sequence: u64,
    /// Message priority level
    pub priority: MessagePriority,
}

/// Message priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessagePriority {
    /// Low priority - background processing
    Low = 0,
    /// Normal priority - default
    Normal = 1,
    /// High priority - expedited processing
    High = 2,
    /// Critical priority - immediate processing
    Critical = 3,
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Session-related message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    /// Session ID
    pub session_id: String,
    /// Connection ID within session
    pub connection_id: String,
    /// Session message type
    pub msg_type: SessionMessageType,
    /// Message data
    pub data: Vec<u8>,
}

/// Session message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionMessageType {
    /// Handshake initiation
    HandshakeInit,
    /// Handshake response
    HandshakeResponse,
    /// Session establishment confirmation
    SessionEstablished,
    /// Data frame
    DataFrame,
    /// Control frame
    ControlFrame,
    /// Session termination
    SessionClose,
}

/// Stream-related message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    /// Stream ID
    pub stream_id: u32,
    /// Stream message type
    pub msg_type: StreamMessageType,
    /// Payload data
    pub payload: Vec<u8>,
    /// Flow control information
    pub flow_control: Option<FlowControlInfo>,
}

/// Stream message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamMessageType {
    /// Stream data frame
    Data,
    /// Stream control frame
    Control,
    /// Stream reset
    Reset,
    /// Stream close
    Close,
    /// Flow control update
    FlowControl,
}

/// Flow control information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowControlInfo {
    /// Window size
    pub window_size: u32,
    /// Available credits
    pub credits: u32,
    /// Backpressure indication
    pub backpressure: bool,
}

/// DHT-related message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtMessage {
    /// Source node ID
    pub source_node: String,
    /// Target node ID (if applicable)
    pub target_node: Option<String>,
    /// DHT operation type
    pub operation: DhtOperation,
    /// Message data
    pub data: Vec<u8>,
    /// TTL for message propagation
    pub ttl: u8,
}

/// DHT operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtOperation {
    /// Find node operation
    FindNode,
    /// Find value operation
    FindValue,
    /// Store value operation
    Store,
    /// Ping operation
    Ping,
    /// Pong response
    Pong,
    /// Node announcement
    Announce,
}

/// Push notification message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushNotificationMessage {
    /// Target device token
    pub device_token: String,
    /// Notification title
    pub title: String,
    /// Notification body
    pub body: String,
    /// Custom data payload
    pub data: HashMap<String, String>,
    /// Notification priority
    pub priority: NotificationPriority,
    /// Time-to-live
    pub ttl: Option<Duration>,
}

/// Notification priority levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationPriority {
    /// Normal priority
    Normal,
    /// High priority
    High,
}

/// Proto message manager for centralized message handling
pub struct ProtoManager {
    /// Message type registry
    type_registry: HashMap<String, MessageTypeInfo>,
    /// Sequence counter for message ordering
    sequence_counter: std::sync::atomic::AtomicU64,
}

/// Message type information
#[derive(Debug, Clone)]
pub struct MessageTypeInfo {
    /// Type name
    pub name: String,
    /// Type description
    pub description: String,
    /// Whether type supports serialization
    pub serializable: bool,
    /// Message schema version
    pub version: u32,
}

impl ProtoManager {
    /// Create a new proto manager
    pub fn new() -> Self {
        let mut manager = Self {
            type_registry: HashMap::new(),
            sequence_counter: std::sync::atomic::AtomicU64::new(0),
        };
        
        // Register built-in message types
        manager.register_builtin_types();
        manager
    }

    /// Register built-in message types
    fn register_builtin_types(&mut self) {
        let types = vec![
            ("session_message", "Session management messages", true, 1),
            ("stream_message", "Stream data and control messages", true, 1),
            ("dht_message", "DHT operation messages", true, 1),
            ("push_notification", "Push notification messages", true, 1),
            ("telemetry_message", "Telemetry and metrics messages", true, 1),
            ("control_message", "System control messages", true, 1),
        ];

        for (name, desc, serializable, version) in types {
            self.type_registry.insert(
                name.to_string(),
                MessageTypeInfo {
                    name: name.to_string(),
                    description: desc.to_string(),
                    serializable,
                    version,
                },
            );
        }
    }

    /// Register a new message type
    pub fn register_message_type(
        &mut self,
        name: String,
        description: String,
        serializable: bool,
        version: u32,
    ) -> Result<()> {
        if self.type_registry.contains_key(&name) {
            return Err(anyhow::anyhow!("Message type '{}' already registered", name));
        }

        self.type_registry.insert(
            name.clone(),
            MessageTypeInfo {
                name,
                description,
                serializable,
                version,
            },
        );

        Ok(())
    }

    /// Get message type information
    pub fn get_message_type(&self, name: &str) -> Option<&MessageTypeInfo> {
        self.type_registry.get(name)
    }

    /// List all registered message types
    pub fn list_message_types(&self) -> Vec<&MessageTypeInfo> {
        self.type_registry.values().collect()
    }

    /// Create a new Nyx message envelope
    pub fn create_message(
        &self,
        message_type: String,
        payload: Vec<u8>,
        priority: MessagePriority,
    ) -> NyxMessage {
        let sequence = self.sequence_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        
        NyxMessage {
            message_type,
            timestamp: SystemTime::now(),
            payload,
            metadata: HashMap::new(),
            sequence,
            priority,
        }
    }

    /// Serialize a message to bytes
    pub fn serialize_message<T: Serialize>(&self, message: &T) -> Result<Vec<u8>> {
        bincode::serialize(message).context("Failed to serialize message")
    }

    /// Deserialize a message from bytes
    pub fn deserialize_message<T: for<'de> Deserialize<'de>>(&self, data: &[u8]) -> Result<T> {
        bincode::deserialize(data).context("Failed to deserialize message")
    }

    /// Convert SystemTime to protobuf Timestamp
    pub fn system_time_to_proto(&self, time: SystemTime) -> Result<Timestamp> {
        let duration = time.duration_since(UNIX_EPOCH)
            .context("Time is before UNIX epoch")?;
        
        Ok(Timestamp {
            seconds: duration.as_secs() as i64,
            nanos: duration.subsec_nanos() as i32,
        })
    }

    /// Convert protobuf Timestamp to SystemTime
    pub fn proto_to_system_time(&self, timestamp: &Timestamp) -> Result<SystemTime> {
        let duration = Duration::new(
            timestamp.seconds as u64,
            timestamp.nanos as u32,
        );
        
        Ok(UNIX_EPOCH + duration)
    }

    /// Convert Duration to protobuf Duration
    pub fn duration_to_proto(&self, duration: Duration) -> ProtoDuration {
        ProtoDuration {
            seconds: duration.as_secs() as i64,
            nanos: duration.subsec_nanos() as i32,
        }
    }

    /// Convert protobuf Duration to Duration
    pub fn proto_to_duration(&self, proto_duration: &ProtoDuration) -> Result<Duration> {
        if proto_duration.seconds < 0 || proto_duration.nanos < 0 {
            return Err(anyhow::anyhow!("Invalid duration: negative values"));
        }
        
        Ok(Duration::new(
            proto_duration.seconds as u64,
            proto_duration.nanos as u32,
        ))
    }

    /// Validate message envelope
    pub fn validate_message(&self, message: &NyxMessage) -> Result<()> {
        // Check if message type is registered
        if !self.type_registry.contains_key(&message.message_type) {
            return Err(anyhow::anyhow!("Unknown message type: {}", message.message_type));
        }

        // Check payload size (max 64MB)
        if message.payload.len() > 64 * 1024 * 1024 {
            return Err(anyhow::anyhow!("Message payload too large: {} bytes", message.payload.len()));
        }

        // Check timestamp is reasonable (within last 24 hours and next 1 hour)
        let now = SystemTime::now();
        let day_ago = now - Duration::from_secs(86400);
        let hour_ahead = now + Duration::from_secs(3600);
        
        if message.timestamp < day_ago || message.timestamp > hour_ahead {
            return Err(anyhow::anyhow!("Message timestamp out of acceptable range"));
        }

        Ok(())
    }

    /// Get message statistics
    pub fn get_stats(&self) -> ProtoManagerStats {
        ProtoManagerStats {
            registered_types: self.type_registry.len(),
            current_sequence: self.sequence_counter.load(std::sync::atomic::Ordering::SeqCst),
        }
    }
}

impl Default for ProtoManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Proto manager statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtoManagerStats {
    /// Number of registered message types
    pub registered_types: usize,
    /// Current sequence number
    pub current_sequence: u64,
}

/// Utility functions for common protobuf operations
pub mod utils {
    use super::*;

    /// Create a session message
    pub fn create_session_message(
        session_id: String,
        connection_id: String,
        msg_type: SessionMessageType,
        data: Vec<u8>,
    ) -> SessionMessage {
        SessionMessage {
            session_id,
            connection_id,
            msg_type,
            data,
        }
    }

    /// Create a stream message
    pub fn create_stream_message(
        stream_id: u32,
        msg_type: StreamMessageType,
        payload: Vec<u8>,
        flow_control: Option<FlowControlInfo>,
    ) -> StreamMessage {
        StreamMessage {
            stream_id,
            msg_type,
            payload,
            flow_control,
        }
    }

    /// Create a DHT message
    pub fn create_dht_message(
        source_node: String,
        target_node: Option<String>,
        operation: DhtOperation,
        data: Vec<u8>,
        ttl: u8,
    ) -> DhtMessage {
        DhtMessage {
            source_node,
            target_node,
            operation,
            data,
            ttl,
        }
    }

    /// Create a push notification message
    pub fn create_push_notification(
        device_token: String,
        title: String,
        body: String,
        data: HashMap<String, String>,
        priority: NotificationPriority,
        ttl: Option<Duration>,
    ) -> PushNotificationMessage {
        PushNotificationMessage {
            device_token,
            title,
            body,
            data,
            priority,
            ttl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_manager_creation() {
        let manager = ProtoManager::new();
        assert!(!manager.type_registry.is_empty());
        
        // Check built-in types are registered
        assert!(manager.get_message_type("session_message").is_some());
        assert!(manager.get_message_type("stream_message").is_some());
        assert!(manager.get_message_type("dht_message").is_some());
    }

    #[test]
    fn test_message_type_registration() {
        let mut manager = ProtoManager::new();
        
        let result = manager.register_message_type(
            "test_message".to_string(),
            "Test message type".to_string(),
            true,
            1,
        );
        assert!(result.is_ok());
        
        let type_info = manager.get_message_type("test_message");
        assert!(type_info.is_some());
        assert_eq!(type_info.unwrap().name, "test_message");
    }

    #[test]
    fn test_duplicate_message_type_registration() {
        let mut manager = ProtoManager::new();
        
        let result1 = manager.register_message_type(
            "test_message".to_string(),
            "Test message type".to_string(),
            true,
            1,
        );
        assert!(result1.is_ok());
        
        let result2 = manager.register_message_type(
            "test_message".to_string(),
            "Duplicate test message type".to_string(),
            true,
            1,
        );
        assert!(result2.is_err());
    }

    #[test]
    fn test_message_creation() {
        let manager = ProtoManager::new();
        
        let message = manager.create_message(
            "test_message".to_string(),
            b"test payload".to_vec(),
            MessagePriority::High,
        );
        
        assert_eq!(message.message_type, "test_message");
        assert_eq!(message.payload, b"test payload");
        assert_eq!(message.priority, MessagePriority::High);
        assert_eq!(message.sequence, 0);
    }

    #[test]
    fn test_sequence_increment() {
        let manager = ProtoManager::new();
        
        let msg1 = manager.create_message("test".to_string(), vec![], MessagePriority::Normal);
        let msg2 = manager.create_message("test".to_string(), vec![], MessagePriority::Normal);
        
        assert_eq!(msg1.sequence, 0);
        assert_eq!(msg2.sequence, 1);
    }

    #[test]
    fn test_time_conversion() {
        let manager = ProtoManager::new();
        let now = SystemTime::now();
        
        let proto_time = manager.system_time_to_proto(now).unwrap();
        let converted_back = manager.proto_to_system_time(&proto_time).unwrap();
        
        // Should be close (within 1 second due to precision loss)
        let diff = now.duration_since(converted_back).unwrap_or_else(|_| {
            converted_back.duration_since(now).unwrap()
        });
        assert!(diff < Duration::from_secs(1));
    }

    #[test]
    fn test_duration_conversion() {
        let manager = ProtoManager::new();
        let duration = Duration::from_secs(3600);
        
        let proto_duration = manager.duration_to_proto(duration);
        let converted_back = manager.proto_to_duration(&proto_duration).unwrap();
        
        assert_eq!(duration, converted_back);
    }

    #[test]
    fn test_message_validation() {
        let manager = ProtoManager::new();
        
        // Valid message
        let valid_message = manager.create_message(
            "session_message".to_string(),
            b"test".to_vec(),
            MessagePriority::Normal,
        );
        assert!(manager.validate_message(&valid_message).is_ok());
        
        // Invalid message type
        let invalid_message = NyxMessage {
            message_type: "unknown_type".to_string(),
            timestamp: SystemTime::now(),
            payload: vec![],
            metadata: HashMap::new(),
            sequence: 0,
            priority: MessagePriority::Normal,
        };
        assert!(manager.validate_message(&invalid_message).is_err());
    }

    #[test]
    fn test_message_serialization() {
        let manager = ProtoManager::new();
        
        let message = utils::create_session_message(
            "session123".to_string(),
            "conn456".to_string(),
            SessionMessageType::HandshakeInit,
            b"handshake data".to_vec(),
        );
        
        let serialized = manager.serialize_message(&message).unwrap();
        let deserialized: SessionMessage = manager.deserialize_message(&serialized).unwrap();
        
        assert_eq!(message.session_id, deserialized.session_id);
        assert_eq!(message.connection_id, deserialized.connection_id);
        assert_eq!(message.data, deserialized.data);
    }

    #[test]
    fn test_utils_functions() {
        // Test session message creation
        let session_msg = utils::create_session_message(
            "session1".to_string(),
            "conn1".to_string(),
            SessionMessageType::DataFrame,
            b"data".to_vec(),
        );
        assert_eq!(session_msg.session_id, "session1");
        
        // Test stream message creation
        let stream_msg = utils::create_stream_message(
            42,
            StreamMessageType::Data,
            b"stream data".to_vec(),
            None,
        );
        assert_eq!(stream_msg.stream_id, 42);
        
        // Test DHT message creation
        let dht_msg = utils::create_dht_message(
            "node1".to_string(),
            Some("node2".to_string()),
            DhtOperation::FindNode,
            b"dht data".to_vec(),
            64,
        );
        assert_eq!(dht_msg.source_node, "node1");
        assert_eq!(dht_msg.ttl, 64);
        
        // Test push notification creation
        let push_msg = utils::create_push_notification(
            "device123".to_string(),
            "Title".to_string(),
            "Body".to_string(),
            HashMap::new(),
            NotificationPriority::High,
            Some(Duration::from_secs(3600)),
        );
        assert_eq!(push_msg.device_token, "device123");
        assert_eq!(push_msg.title, "Title");
    }

    #[test]
    fn test_message_priority_default() {
        let priority = MessagePriority::default();
        assert_eq!(priority, MessagePriority::Normal);
    }

    #[test]
    fn test_manager_stats() {
        let manager = ProtoManager::new();
        let stats = manager.get_stats();
        
        assert!(stats.registered_types > 0);
        assert_eq!(stats.current_sequence, 0);
        
        // Create a message to increment sequence
        let _msg = manager.create_message("test".to_string(), vec![], MessagePriority::Normal);
        let updated_stats = manager.get_stats();
        assert_eq!(updated_stats.current_sequence, 1);
    }
}
