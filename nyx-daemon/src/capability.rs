//! Capability management and negotiation for Path Builder (Section F completion)
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct CapabilityCatalog {
    mandatory: HashSet<String>,
    optional: HashSet<String>,
}

impl CapabilityCatalog {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_mandatory(mut self, caps: &[&str]) -> Self {
        for c in caps {
            self.mandatory.insert((*c).to_string());
        }
        self
    }
    pub fn with_optional(mut self, caps: &[&str]) -> Self {
        for c in caps {
            self.optional.insert((*c).to_string());
        }
        self
    }
    pub fn mandatory(&self) -> &HashSet<String> {
        &self.mandatory
    }
    pub fn optional(&self) -> &HashSet<String> {
        &self.optional
    }
    pub fn negotiate(&self, peer_caps: &HashSet<String>) -> (HashSet<String>, HashSet<String>) {
        let missing: HashSet<String> = self.mandatory.difference(peer_caps).cloned().collect();
        if !missing.is_empty() {
            return (HashSet::new(), missing);
        }
        let mut accepted: HashSet<String> =
            self.mandatory.intersection(peer_caps).cloned().collect();
        for c in self.optional.intersection(peer_caps) {
            accepted.insert(c.clone());
        }
        (accepted, HashSet::new())
    }
}

#[derive(Debug, Clone)]
pub struct CapabilityNegotiationResult {
    pub accepted: HashSet<String>,
    pub missing_mandatory: HashSet<String>,
}
impl CapabilityNegotiationResult {
    pub fn is_success(&self) -> bool {
        self.missing_mandatory.is_empty()
    }
}

pub fn negotiate_with_peer(
    catalog: &CapabilityCatalog,
    peer_caps: &HashSet<String>,
) -> CapabilityNegotiationResult {
    let (accepted, missing) = catalog.negotiate(peer_caps);
    CapabilityNegotiationResult {
        accepted,
        missing_mandatory: missing,
    }
}
