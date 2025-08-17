//! Capability negotiation audit logging tests
//!
//! These tests verify audit logging and monitoring for capability negotiation
//! rejection and degradation scenarios as specified in the traceability matrix.

use nyx_stream::capability::*;
use nyx_stream::management::*;
use serde_json;
use std::sync::{Arc, Mutex};

/// Mock audit logger for testing
#[derive(Debug, Clone)]
pub struct MockAuditLogger {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AuditEvent {
    pub event_type: String,
    pub capability_id: Option<u32>,
    pub peer_id: Option<String>,
    pub reason: String,
    pub timestamp: u64,
}

impl MockAuditLogger {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log_capability_rejection(&self, cap_id: u32, peer_id: &str, reason: &str) {
        let event = AuditEvent {
            event_type: "capability_rejection".to_string(),
            capability_id: Some(cap_id),
            peer_id: Some(peer_id.to_string()),
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.events.lock().unwrap().push(event);
    }

    pub fn log_capability_degradation(&self, cap_id: u32, peer_id: &str, reason: &str) {
        let event = AuditEvent {
            event_type: "capability_degradation".to_string(),
            capability_id: Some(cap_id),
            peer_id: Some(peer_id.to_string()),
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.events.lock().unwrap().push(event);
    }

    pub fn log_session_termination(&self, peer_id: &str, reason: &str) {
        let event = AuditEvent {
            event_type: "session_termination".to_string(),
            capability_id: None,
            peer_id: Some(peer_id.to_string()),
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        self.events.lock().unwrap().push(event);
    }

    pub fn get_events(&self) -> Vec<AuditEvent> {
        self.events.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }
}

/// Enhanced capability negotiator with audit logging
pub struct AuditingCapabilityNegotiator {
    local_supported: Vec<u32>,
    audit_logger: MockAuditLogger,
}

impl AuditingCapabilityNegotiator {
    pub fn new(local_supported: Vec<u32>, audit_logger: MockAuditLogger) -> Self {
        Self {
            local_supported,
            audit_logger,
        }
    }

    /// Negotiate capabilities with comprehensive audit logging
    pub fn negotiate_with_audit(
        &self,
        peer_caps: &[Capability],
        peer_id: &str,
    ) -> Result<Vec<Capability>, CapabilityError> {
        let mut accepted_caps = Vec::new();
        let mut degraded_caps = Vec::new();

        for cap in peer_caps {
            if cap.is_required() {
                if !self.local_supported.contains(&cap.id) {
                    // Log rejection and prepare for session termination
                    self.audit_logger.log_capability_rejection(
                        cap.id,
                        peer_id,
                        &format!("Unsupported required capability: 0x{:08x}", cap.id),
                    );

                    self.audit_logger.log_session_termination(
                        peer_id,
                        &format!("Required capability 0x{:08x} not supported", cap.id),
                    );

                    return Err(CapabilityError::UnsupportedRequired(cap.id));
                } else {
                    accepted_caps.push(cap.clone());
                }
            } else {
                // Optional capability
                if self.local_supported.contains(&cap.id) {
                    accepted_caps.push(cap.clone());
                } else {
                    // Log degradation (optional capability ignored)
                    self.audit_logger.log_capability_degradation(
                        cap.id,
                        peer_id,
                        &format!("Optional capability 0x{:08x} not supported - ignored", cap.id),
                    );
                    degraded_caps.push(cap.clone());
                }
            }
        }

        Ok(accepted_caps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_capability_rejection() {
        let audit_logger = MockAuditLogger::new();
        let negotiator = AuditingCapabilityNegotiator::new(
            vec![CAP_CORE], // Only support core
            audit_logger.clone(),
        );

        let peer_caps = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::required(CAP_PLUGIN_FRAMEWORK, vec![]), // This will be rejected
        ];

        let result = negotiator.negotiate_with_audit(&peer_caps, "peer-001");
        assert!(result.is_err());

        let events = audit_logger.get_events();
        assert_eq!(events.len(), 2); // Rejection + termination

        // Check rejection event
        let rejection_event = &events[0];
        assert_eq!(rejection_event.event_type, "capability_rejection");
        assert_eq!(rejection_event.capability_id, Some(CAP_PLUGIN_FRAMEWORK));
        assert_eq!(rejection_event.peer_id, Some("peer-001".to_string()));
        assert!(rejection_event.reason.contains("Unsupported required capability"));

        // Check termination event
        let termination_event = &events[1];
        assert_eq!(termination_event.event_type, "session_termination");
        assert_eq!(termination_event.peer_id, Some("peer-001".to_string()));
        assert!(termination_event.reason.contains("Required capability"));
    }

    #[test]
    fn test_audit_capability_degradation() {
        let audit_logger = MockAuditLogger::new();
        let negotiator = AuditingCapabilityNegotiator::new(
            vec![CAP_CORE], // Only support core
            audit_logger.clone(),
        );

        let peer_caps = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::optional(CAP_PLUGIN_FRAMEWORK, vec![]), // This will be degraded
            Capability::optional(0x9999, vec![]), // Unknown optional - degraded
        ];

        let result = negotiator.negotiate_with_audit(&peer_caps, "peer-002");
        assert!(result.is_ok());

        let accepted = result.unwrap();
        assert_eq!(accepted.len(), 1); // Only core accepted

        let events = audit_logger.get_events();
        assert_eq!(events.len(), 2); // Two degradation events

        // Check first degradation (plugin framework)
        let degradation1 = &events[0];
        assert_eq!(degradation1.event_type, "capability_degradation");
        assert_eq!(degradation1.capability_id, Some(CAP_PLUGIN_FRAMEWORK));
        assert!(degradation1.reason.contains("Optional capability"));

        // Check second degradation (unknown capability)
        let degradation2 = &events[1];
        assert_eq!(degradation2.event_type, "capability_degradation");
        assert_eq!(degradation2.capability_id, Some(0x9999));
    }

    #[test]
    fn test_audit_successful_negotiation() {
        let audit_logger = MockAuditLogger::new();
        let negotiator = AuditingCapabilityNegotiator::new(
            vec![CAP_CORE, CAP_PLUGIN_FRAMEWORK],
            audit_logger.clone(),
        );

        let peer_caps = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::optional(CAP_PLUGIN_FRAMEWORK, vec![]),
        ];

        let result = negotiator.negotiate_with_audit(&peer_caps, "peer-003");
        assert!(result.is_ok());

        let accepted = result.unwrap();
        assert_eq!(accepted.len(), 2); // Both capabilities accepted

        let events = audit_logger.get_events();
        assert_eq!(events.len(), 0); // No audit events for successful negotiation
    }

    #[test]
    fn test_close_frame_audit_integration() {
        let audit_logger = MockAuditLogger::new();
        let unsupported_cap_id = 0x12345678u32;

        // Simulate rejection scenario
        audit_logger.log_capability_rejection(
            unsupported_cap_id,
            "peer-004",
            "Required capability not supported",
        );

        // Build CLOSE frame
        let close_frame = build_close_unsupported_cap(unsupported_cap_id);
        assert_eq!(close_frame.len(), 6);

        // Verify we can parse back the capability ID
        let parsed_id = parse_close_unsupported_cap(&close_frame).unwrap();
        assert_eq!(parsed_id, unsupported_cap_id);

        // Verify audit event
        let events = audit_logger.get_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].capability_id, Some(unsupported_cap_id));
    }

    #[test]
    fn test_audit_event_serialization() {
        let audit_logger = MockAuditLogger::new();
        
        audit_logger.log_capability_rejection(
            CAP_PLUGIN_FRAMEWORK,
            "peer-005",
            "Test rejection reason",
        );

        let events = audit_logger.get_events();
        let event = &events[0];

        // Test that audit events can be serialized to JSON for external logging
        let json = serde_json::to_string(event).expect("Should serialize to JSON");
        assert!(json.contains("capability_rejection"));
        assert!(json.contains("2")); // CAP_PLUGIN_FRAMEWORK as decimal
        assert!(json.contains("peer-005"));
    }

    #[test]
    fn test_audit_timestamp_ordering() {
        let audit_logger = MockAuditLogger::new();

        // Log multiple events
        audit_logger.log_capability_degradation(0x1111, "peer-006", "First degradation");
        std::thread::sleep(std::time::Duration::from_millis(10));
        audit_logger.log_capability_rejection(0x2222, "peer-006", "Rejection");
        std::thread::sleep(std::time::Duration::from_millis(10));
        audit_logger.log_session_termination("peer-006", "Session ended");

        let events = audit_logger.get_events();
        assert_eq!(events.len(), 3);

        // Verify timestamp ordering
        assert!(events[0].timestamp <= events[1].timestamp);
        assert!(events[1].timestamp <= events[2].timestamp);

        // Verify event sequence
        assert_eq!(events[0].event_type, "capability_degradation");
        assert_eq!(events[1].event_type, "capability_rejection");
        assert_eq!(events[2].event_type, "session_termination");
    }

    #[test]
    fn test_capability_validation_audit() {
        let audit_logger = MockAuditLogger::new();

        // Test oversized capability data
        let oversized_cap = Capability::new(CAP_CORE, FLAG_REQUIRED, vec![0u8; 2048]);
        let validation_result = validate_capability(&oversized_cap);
        assert!(validation_result.is_err());

        // In a real implementation, validation failures would be audited
        if validation_result.is_err() {
            audit_logger.log_capability_rejection(
                oversized_cap.id,
                "peer-007",
                "Capability data too large",
            );
        }

        let events = audit_logger.get_events();
        assert_eq!(events.len(), 1);
        assert!(events[0].reason.contains("too large"));
    }
}
