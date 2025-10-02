#![no_main]

use libfuzzer_sys::{arbitrary::Arbitrary, fuzz_target};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use nyx_transport::ice::{Candidate, CandidateType, Transport};

/// Arbitrary implementation for CandidateType to enable fuzzing
#[derive(Debug, Arbitrary)]
enum FuzzCandidateType {
    Host,
    ServerReflexive,
    PeerReflexive,
    Relay,
    Invalid(u8), // Edge case: invalid discriminant
}

impl From<FuzzCandidateType> for CandidateType {
    fn from(fuzz: FuzzCandidateType) -> Self {
        match fuzz {
            FuzzCandidateType::Host => CandidateType::Host,
            FuzzCandidateType::ServerReflexive => CandidateType::ServerReflexive,
            FuzzCandidateType::PeerReflexive => CandidateType::PeerReflexive,
            FuzzCandidateType::Relay => CandidateType::Relay,
            FuzzCandidateType::Invalid(_) => CandidateType::Host, // Fallback
        }
    }
}

/// Arbitrary implementation for Transport
#[derive(Debug, Arbitrary)]
enum FuzzTransport {
    Udp,
    Tcp,
    Invalid(u8), // Edge case: invalid protocol
}

impl From<FuzzTransport> for Transport {
    fn from(fuzz: FuzzTransport) -> Self {
        match fuzz {
            FuzzTransport::Udp => Transport::Udp,
            FuzzTransport::Tcp => Transport::Tcp,
            FuzzTransport::Invalid(_) => Transport::Udp, // Fallback
        }
    }
}

/// Fuzzing input structure for ICE candidate
#[derive(Debug, Arbitrary)]
struct FuzzCandidate {
    foundation: Vec<u8>, // Arbitrary byte string for foundation
    component_id: u32,
    transport: FuzzTransport,
    priority: u32,
    ip_bytes: [u8; 16], // IPv6-sized buffer
    port: u16,
    candidate_type: FuzzCandidateType,
    has_related: bool,
    related_ip: [u8; 16],
    related_port: u16,
    extensions: Vec<(Vec<u8>, Vec<u8>)>, // Key-value pairs
}

impl FuzzCandidate {
    /// Convert fuzz input to valid Candidate structure
    fn to_candidate(&self) -> Result<Candidate, String> {
        // Edge case: Empty foundation
        let foundation = if self.foundation.is_empty() {
            "0".to_string()
        } else {
            String::from_utf8_lossy(&self.foundation)
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
                .take(32) // Limit foundation length
                .collect()
        };

        // Edge case: Invalid IP address construction
        let address = if self.ip_bytes[12..].iter().all(|&b| b == 0) {
            // Try IPv4
            let ipv4 = Ipv4Addr::from([
                self.ip_bytes[0],
                self.ip_bytes[1],
                self.ip_bytes[2],
                self.ip_bytes[3],
            ]);
            SocketAddr::new(IpAddr::V4(ipv4), self.port)
        } else {
            // Use full IPv6
            let ipv6 = Ipv6Addr::from(self.ip_bytes);
            SocketAddr::new(IpAddr::V6(ipv6), self.port)
        };

        // Edge case: Related address handling
        let related_address = if self.has_related {
            let related_ip = if self.related_ip[12..].iter().all(|&b| b == 0) {
                IpAddr::V4(Ipv4Addr::from([
                    self.related_ip[0],
                    self.related_ip[1],
                    self.related_ip[2],
                    self.related_ip[3],
                ]))
            } else {
                IpAddr::V6(Ipv6Addr::from(self.related_ip))
            };
            Some(SocketAddr::new(related_ip, self.related_port))
        } else {
            None
        };

        // Edge case: Oversized extensions map
        let mut extensions = HashMap::new();
        for (key_bytes, value_bytes) in self.extensions.iter().take(10) {
            // Limit to 10 extensions
            let key = String::from_utf8_lossy(key_bytes)
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .take(32)
                .collect();
            let value = String::from_utf8_lossy(value_bytes)
                .chars()
                .take(256)
                .collect();
            extensions.insert(key, value);
        }

        Ok(Candidate {
            foundation,
            component_id: self.component_id,
            transport: self.transport.into(),
            priority: self.priority,
            address,
            candidate_type: self.candidate_type.into(),
            related_address,
            extensions,
        })
    }
}

/// Simple SDP-like ICE candidate string parser for fuzzing
/// Format: "candidate:<foundation> <component-id> <transport> <priority> <address> <port> typ <type>"
fn parse_ice_candidate_string(input: &str) -> Result<Candidate, String> {
    // Edge case: Empty input
    if input.is_empty() {
        return Err("Empty input".to_string());
    }

    // Edge case: Input too large (DoS prevention)
    if input.len() > 4096 {
        return Err("Input too large".to_string());
    }

    let parts: Vec<&str> = input.split_whitespace().collect();

    // Edge case: Insufficient parts
    if parts.len() < 8 {
        return Err("Insufficient parts".to_string());
    }

    // Edge case: Invalid prefix
    if !parts[0].starts_with("candidate:") {
        return Err("Invalid prefix".to_string());
    }

    let foundation = parts[0]
        .strip_prefix("candidate:")
        .unwrap_or("0")
        .to_string();

    // Edge case: Invalid component ID
    let component_id = parts[1].parse::<u32>().unwrap_or(1);

    // Edge case: Invalid transport
    let transport = match parts[2].to_lowercase().as_str() {
        "udp" => Transport::Udp,
        "tcp" => Transport::Tcp,
        _ => return Err("Invalid transport".to_string()),
    };

    // Edge case: Invalid priority (u32 overflow)
    let priority = parts[3].parse::<u32>().unwrap_or(0);

    // Edge case: Invalid IP address
    let ip = parts[4].parse::<IpAddr>().map_err(|_| "Invalid IP")?;

    // Edge case: Invalid port
    let port = parts[5].parse::<u16>().unwrap_or(0);

    let address = SocketAddr::new(ip, port);

    // Edge case: Missing "typ" keyword
    if parts.get(6) != Some(&"typ") {
        return Err("Missing 'typ' keyword".to_string());
    }

    // Edge case: Invalid candidate type
    let candidate_type = match parts.get(7).map(|s| s.to_lowercase()).as_deref() {
        Some("host") => CandidateType::Host,
        Some("srflx") => CandidateType::ServerReflexive,
        Some("prflx") => CandidateType::PeerReflexive,
        Some("relay") => CandidateType::Relay,
        _ => return Err("Invalid candidate type".to_string()),
    };

    Ok(Candidate {
        foundation,
        component_id,
        transport,
        priority,
        address,
        candidate_type,
        related_address: None, // Simplified: no related address parsing
        extensions: HashMap::new(),
    })
}

fuzz_target!(|data: &[u8]| {
    // Strategy 1: Fuzz Candidate structure construction with Arbitrary
    if data.len() > 64 {
        if let Ok(fuzz_cand) = FuzzCandidate::arbitrary(&mut arbitrary::Unstructured::new(data)) {
            // This should never panic, only return Err
            let _ = fuzz_cand.to_candidate();
        }
    }

    // Strategy 2: Fuzz SDP-like ICE candidate string parsing
    if let Ok(input_str) = std::str::from_utf8(data) {
        // Edge cases covered:
        // - Empty string
        // - Invalid UTF-8 (handled by from_utf8 failure)
        // - Malformed fields (missing parts, invalid types)
        // - Oversized input (>4096 bytes)
        let _ = parse_ice_candidate_string(input_str);
    }

    // Strategy 3: Fuzz serde serialization/deserialization
    // This tests for panics in serde derives on Candidate
    if data.len() >= 32 {
        // Try to deserialize arbitrary bytes as JSON
        let _ = serde_json::from_slice::<Candidate>(data);

        // Try CBOR deserialization
        let _ = ciborium::from_reader::<Candidate, _>(data);
    }

    // All branches should handle errors gracefully without panicking
});
