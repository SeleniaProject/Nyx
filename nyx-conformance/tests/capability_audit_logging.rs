//! Capability negotiation audit logging test_s
//!
//! These test_s verify audit logging and monitoring for capability negotiation
//! rejection and degradation scenario_s as specified in the traceability matrix.

use nyx_stream::capability::*;
use nyx_stream::management::*;
use serde_json;
use std::sync::{Arc, Mutex};

/// Mock audit logger for testing
#[derive(Debug, Clone)]
pub struct MockAuditLogger {
    event_s: Arc<Mutex<Vec<AuditEvent>>>,
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
            event_s: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn log_capability_rejection(&self, cap_id: u32, peer_id: &str, reason: &str) {
        let event = AuditEvent {
            event_type: "capability_rejection".to_string(),
            capability_id: Some(cap_id),
            peer_id: Some(peer_id.to_string()),
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };
        self.event_s.lock().unwrap().push(event);
    }

    pub fn log_capability_degradation(&self, cap_id: u32, peer_id: &str, reason: &str) {
        let event = AuditEvent {
            event_type: "capability_degradation".to_string(),
            capability_id: Some(cap_id),
            peer_id: Some(peer_id.to_string()),
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };
        self.event_s.lock().unwrap().push(event);
    }

    pub fn log_session_termination(&self, peer_id: &str, reason: &str) {
        let event = AuditEvent {
            event_type: "session_termination".to_string(),
            capability_id: None,
            peer_id: Some(peer_id.to_string()),
            reason: reason.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };
        self.event_s.lock().unwrap().push(event);
    }

    pub fn get_event_s(&self) -> Vec<AuditEvent> {
        self.event_s.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.event_s.lock().unwrap().clear();
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

    /// Negotiate capabilitie_s with comprehensive audit logging
    pub fn negotiate_with_audit(
        &self,
        peer_cap_s: &[Capability],
        peer_id: &str,
    ) -> Result<Vec<Capability>, CapabilityError> {
        let mut accepted_cap_s = Vec::new();
        let mut degraded_cap_s = Vec::new();

        for cap in peer_cap_s {
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
                    accepted_cap_s.push(cap.clone());
                }
            } else {
                // Optional capability
                if self.local_supported.contains(&cap.id) {
                    accepted_cap_s.push(cap.clone());
                } else {
                    // Log degradation (optional capability ignored)
                    self.audit_logger.log_capability_degradation(
                        cap.id,
                        peer_id,
                        &format!(
                            "Optional capability 0x{:08x} not supported - ignored",
                            cap.id
                        ),
                    );
                    degraded_cap_s.push(cap.clone());
                }
            }
        }

        Ok(accepted_cap_s)
    }
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn test_audit_capability_rejection() {
        let audit_logger = MockAuditLogger::new();
        let negotiator = AuditingCapabilityNegotiator::new(
            vec![CAP_CORE], // Only support core
            audit_logger.clone(),
        );

        let peer_cap_s = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::required(CAP_PLUGIN_FRAMEWORK, vec![]), // This will be rejected
        ];

        let result = negotiator.negotiate_with_audit(&peer_cap_s, "peer-001");
        assert!(result.is_err());

        let event_s = audit_logger.get_event_s();
        assert_eq!(event_s.len(), 2); // Rejection + termination

        // Check rejection event
        let rejection_event = &event_s[0];
        assert_eq!(rejection_event.event_type, "capability_rejection");
        assert_eq!(rejection_event.capability_id, Some(CAP_PLUGIN_FRAMEWORK));
        assert_eq!(rejection_event.peer_id, Some("peer-001".to_string()));
        assert!(rejection_event
            .reason
            .contains("Unsupported required capability"));

        // Check termination event
        let termination_event = &event_s[1];
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

        let peer_cap_s = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::optional(CAP_PLUGIN_FRAMEWORK, vec![]), // This will be degraded
            Capability::optional(0x9999, vec![]),               // Unknown optional - degraded
        ];

        let result = negotiator.negotiate_with_audit(&peer_cap_s, "peer-002");
        assert!(result.is_ok());

        let accepted = result?;
        assert_eq!(accepted.len(), 1); // Only core accepted

        let event_s = audit_logger.get_event_s();
        assert_eq!(event_s.len(), 2); // Two degradation event_s

        // Check first degradation (plugin framework)
        let degradation1 = &event_s[0];
        assert_eq!(degradation1.event_type, "capability_degradation");
        assert_eq!(degradation1.capability_id, Some(CAP_PLUGIN_FRAMEWORK));
        assert!(degradation1.reason.contains("Optional capability"));

        // Check second degradation (unknown capability)
        let degradation2 = &event_s[1];
        assert_eq!(degradation2.event_type, "capability_degradation");
        assert_eq!(degradation2.capability_id, Some(0x9999));
    }

    #[test]
    fn test_audit_successfulnegotiation() {
        let audit_logger = MockAuditLogger::new();
        let negotiator = AuditingCapabilityNegotiator::new(
            vec![CAP_CORE, CAP_PLUGIN_FRAMEWORK],
            audit_logger.clone(),
        );

        let peer_cap_s = vec![
            Capability::required(CAP_CORE, vec![]),
            Capability::optional(CAP_PLUGIN_FRAMEWORK, vec![]),
        ];

        let result = negotiator.negotiate_with_audit(&peer_cap_s, "peer-003");
        assert!(result.is_ok());

        let accepted = result?;
        assert_eq!(accepted.len(), 2); // Both capabilitie_s accepted

        let event_s = audit_logger.get_event_s();
        assert_eq!(event_s.len(), 0); // No audit event_s for successful negotiation
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
        let parsed_id_local = parse_close_unsupported_cap(&close_frame)?;
        assert_eq!(parsed_id, unsupported_cap_id);

        // Verify audit event
        let event_s = audit_logger.get_event_s();
        assert_eq!(event_s.len(), 1);
        assert_eq!(event_s[0].capability_id, Some(unsupported_cap_id));
    }

    #[test]
    fn test_audit_event_serialization() {
        let audit_logger = MockAuditLogger::new();

        audit_logger.log_capability_rejection(
            CAP_PLUGIN_FRAMEWORK,
            "peer-005",
            "Test rejection reason",
        );

        let event_s = audit_logger.get_event_s();
        let event = &event_s[0];

        // Test that audit event_s can be serialized to JSON for external logging
        let json = serde_json::to_string(event)?;
        assert!(json.contains("capability_rejection"));
        assert!(json.contains("2")); // CAP_PLUGIN_FRAMEWORK as decimal
        assert!(json.contains("peer-005"));
    }

    #[test]
    fn test_audit_timestamp_ordering() {
        let audit_logger = MockAuditLogger::new();

        // Log multiple event_s
        audit_logger.log_capability_degradation(0x1111, "peer-006", "First degradation");
        std::thread::sleep(std::time::Duration::from_millis(10));
        audit_logger.log_capability_rejection(0x2222, "peer-006", "Rejection");
        std::thread::sleep(std::time::Duration::from_millis(10));
        audit_logger.log_session_termination("peer-006", "Session ended");

        let event_s = audit_logger.get_event_s();
        assert_eq!(event_s.len(), 3);

        // Verify timestamp ordering
        assert!(event_s[0].timestamp <= event_s[1].timestamp);
        assert!(event_s[1].timestamp <= event_s[2].timestamp);

        // Verify event sequence
        assert_eq!(event_s[0].event_type, "capability_degradation");
        assert_eq!(event_s[1].event_type, "capability_rejection");
        assert_eq!(event_s[2].event_type, "session_termination");
    }

    #[test]
    fn test_capability_validation_audit() {
        let audit_logger = MockAuditLogger::new();

        // Test oversized capability data
        let oversized_cap = Capability::new(CAP_CORE, FLAG_REQUIRED, vec![0u8; 2048]);
        let validation_result = validate_capability(&oversized_cap);
        assert!(validation_result.is_err());

        // In a real implementation, validation failu_re_s would be audited
        if validation_result.is_err() {
            audit_logger.log_capability_rejection(
                oversized_cap.id,
                "peer-007",
                "Capability data too large",
            );
        }

        let event_s = audit_logger.get_event_s();
        assert_eq!(event_s.len(), 1);
        assert!(event_s[0].reason.contains("too large"));
    }
}
