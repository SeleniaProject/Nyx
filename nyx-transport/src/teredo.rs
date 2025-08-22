//! Teredo and IPv4-mapped IPv6 address handling utilities.
//!
//! This module provides comprehensive support for Teredo tunneling and IPv4-mapped
//! IPv6 addresses, enabling IPv6 connectivity over IPv4 networks. It includes
//! address validation, mapping, and NAT traversal helpers.

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
// use std::time::Duration;
use thiserror::Error;
// use tokio::time::timeout;

#[derive(Error, Debug)]
pub enum TeredoError {
    #[error("Invalid Teredo address: {0}")]
    InvalidAddress(String),
    #[error("Address validation failed: {0}")]
    ValidationFailed(String),
    #[error("NAT mapping error: {0}")]
    NatMapping(String),
    #[error("Teredo prefix mismatch")]
    PrefixMismatch,
}

pub type TeredoResult<T> = Result<T, TeredoError>;

/// Teredo address prefix (2001:0::/32)
pub const TEREDO_PREFIX: u32 = 0x2001_0000;

/// IPv4-mapped IPv6 prefix (::ffff:0:0/96)
pub const IPV4_MAPPED_PREFIX: [u8; 12] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff];

/// Teredo address components
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeredoAddress {
    /// Teredo server IPv4 address
    pub server: Ipv4Addr,
    /// Teredo client's external IPv4 address (obfuscated)
    pub external_addr: Ipv4Addr,
    /// Teredo client's external port (obfuscated)
    pub external_port: u16,
    /// Flags (cone NAT detection, etc.)
    pub flags: u16,
    /// Complete IPv6 Teredo address
    pub ipv6_addr: Ipv6Addr,
}

/// NAT type detection results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatType {
    /// No NAT or full cone NAT
    None,
    /// Full cone NAT
    FullCone,
    /// Restricted cone NAT
    RestrictedCone,
    /// Port restricted cone NAT
    PortRestrictedCone,
    /// Symmetric NAT
    Symmetric,
    /// Unknown or detection failed
    Unknown,
}

/// Address validation results
#[derive(Debug, Clone)]
pub struct AddressValidation {
    pub is_valid: bool,
    pub address_type: AddressType,
    pub nat_type: Option<NatType>,
    pub external_mapping: Option<SocketAddr>,
}

/// Address types we can detect
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressType {
    Ipv4,
    Ipv6,
    Ipv4MappedIpv6,
    TeredoIpv6,
    LinkLocal,
    Loopback,
    Multicast,
    Private,
    Unknown,
}

/// Create an IPv6-mapped IPv4 address (::ffff:a.b.c.d)
pub fn ipv6_mapped(ipv4: Ipv4Addr) -> Ipv6Addr {
    let octets = ipv4.octets();
    Ipv6Addr::from([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, octets[0], octets[1], octets[2], octets[3],
    ])
}

/// Extract IPv4 address from an IPv6-mapped address
pub fn extract_ipv4_from_mapped(ipv6: Ipv6Addr) -> Option<Ipv4Addr> {
    let octets = ipv6.octets();

    // Check for IPv4-mapped prefix
    if octets[0..12] == IPV4_MAPPED_PREFIX {
        Some(Ipv4Addr::new(
            octets[12], octets[13], octets[14], octets[15],
        ))
    } else {
        None
    }
}

/// Check if an IPv6 address is IPv4-mapped
pub fn is_ipv4_mapped(ipv6: Ipv6Addr) -> bool {
    extract_ipv4_from_mapped(ipv6).is_some()
}

/// Create a Teredo address from components (RFC 4380 compliant)
pub fn create_teredo_address(
    server: Ipv4Addr,
    external_addr: Ipv4Addr,
    external_port: u16,
    flags: u16,
) -> TeredoAddress {
    // Obfuscate external address and port according to RFC 4380
    let obfuscated_addr_bytes = external_addr.octets().map(|b| b ^ 0xFF);
    let obfuscated_port = external_port ^ 0xFFFF;

    // Build RFC 4380 compliant Teredo IPv6 address
    let server_bytes = server.octets();
    let ipv6_bytes = [
        0x20,
        0x01, // Teredo prefix (2001:0::/32)
        0x00,
        0x00, // Reserved fields
        server_bytes[0],
        server_bytes[1],
        server_bytes[2],
        server_bytes[3], // Teredo server
        (flags >> 8) as u8,
        (flags & 0xFF) as u8, // Flags field
        (obfuscated_port >> 8) as u8,
        (obfuscated_port & 0xFF) as u8, // Obfuscated port
        obfuscated_addr_bytes[0],
        obfuscated_addr_bytes[1], // Obfuscated IPv4 (bytes 12-13)
        obfuscated_addr_bytes[2],
        obfuscated_addr_bytes[3], // Obfuscated IPv4 (bytes 14-15)
    ];

    let ipv6_addr = Ipv6Addr::from(ipv6_bytes);

    TeredoAddress {
        server,
        external_addr,
        external_port,
        flags,
        ipv6_addr,
    }
}

/// Parse a Teredo address from an IPv6 address (RFC 4380 compliant)
pub fn parse_teredo_address(ipv6: Ipv6Addr) -> TeredoResult<TeredoAddress> {
    let octets = ipv6.octets();

    // Check Teredo prefix (2001:0::/32)
    if octets[0] != 0x20 || octets[1] != 0x01 || octets[2] != 0x00 || octets[3] != 0x00 {
        return Err(TeredoError::PrefixMismatch);
    }

    // Extract server address (bytes 4-7)
    let server = Ipv4Addr::new(octets[4], octets[5], octets[6], octets[7]);

    // Extract flags (bytes 8-9)
    let flags = ((octets[8] as u16) << 8) | (octets[9] as u16);

    // Extract obfuscated port (bytes 10-11) and deobfuscate
    let obfuscated_port = ((octets[10] as u16) << 8) | (octets[11] as u16);
    let external_port = obfuscated_port ^ 0xFFFF;

    // Extract obfuscated IPv4 address (bytes 12-15) and deobfuscate
    let obfuscated_addr_bytes = [
        octets[12] ^ 0xFF,
        octets[13] ^ 0xFF,
        octets[14] ^ 0xFF,
        octets[15] ^ 0xFF,
    ];
    let external_addr = Ipv4Addr::from(obfuscated_addr_bytes);

    Ok(TeredoAddress {
        server,
        external_addr,
        external_port,
        flags,
        ipv6_addr: ipv6,
    })
}

/// Check if an IPv6 address is a Teredo address
pub fn is_teredo_address(ipv6: Ipv6Addr) -> bool {
    parse_teredo_address(ipv6).is_ok()
}

/// Determine the type of an IP address
pub fn classify_address(addr: &SocketAddr) -> AddressType {
    match addr {
        SocketAddr::V4(v4) => {
            let ip = v4.ip();
            if ip.is_loopback() {
                AddressType::Loopback
            } else if ip.is_private() {
                AddressType::Private
            } else if ip.is_multicast() {
                AddressType::Multicast
            } else {
                AddressType::Ipv4
            }
        }
        SocketAddr::V6(v6) => {
            let ip = v6.ip();
            if is_ipv4_mapped(*ip) {
                AddressType::Ipv4MappedIpv6
            } else if is_teredo_address(*ip) {
                AddressType::TeredoIpv6
            } else if ip.is_unicast_link_local() {
                AddressType::LinkLocal
            } else if ip.is_loopback() {
                AddressType::Loopback
            } else if ip.is_multicast() {
                AddressType::Multicast
            } else {
                AddressType::Ipv6
            }
        }
    }
}

/// Validate an address and detect NAT characteristics
pub fn validate_address(addr: &SocketAddr) -> AddressValidation {
    let address_type = classify_address(addr);

    let is_valid = match address_type {
        AddressType::Loopback | AddressType::Multicast => false,
        AddressType::Private => true,
        AddressType::TeredoIpv6 => {
            if let SocketAddr::V6(v6) = addr {
                parse_teredo_address(*v6.ip()).is_ok()
            } else {
                false
            }
        }
        AddressType::Ipv4MappedIpv6 => {
            if let SocketAddr::V6(v6) = addr {
                is_ipv4_mapped(*v6.ip())
            } else {
                false
            }
        }
        _ => true,
    };

    let nat_type = match address_type {
        AddressType::Private => Some(NatType::Unknown),
        AddressType::TeredoIpv6 => Some(NatType::FullCone),
        AddressType::Ipv4 | AddressType::Ipv6 => None,
        _ => Some(NatType::Unknown),
    };

    AddressValidation {
        is_valid,
        address_type,
        nat_type,
        external_mapping: None,
    }
}

/// Convert between IPv4 and IPv6 socket addresses
pub fn convert_socket_addr(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V4(v4) => {
            let mapped = ipv6_mapped(*v4.ip());
            SocketAddr::V6(SocketAddrV6::new(mapped, v4.port(), 0, 0))
        }
        SocketAddr::V6(v6) => {
            if let Some(ipv4) = extract_ipv4_from_mapped(*v6.ip()) {
                SocketAddr::V4(SocketAddrV4::new(ipv4, v6.port()))
            } else {
                addr
            }
        }
    }
}

/// Perform NAT traversal for the given addresses
pub async fn perform_nat_traversal(
    _local_addr: SocketAddr,
    remote_addr: SocketAddr,
) -> TeredoResult<SocketAddr> {
    // Simple implementation - return remote address
    // In a real implementation, this would do hole punching, STUN queries, etc.
    Ok(remote_addr)
}

/// Helper to create a test Teredo address
pub fn create_test_teredo_address() -> TeredoAddress {
    let server = Ipv4Addr::new(192, 0, 2, 1);
    let external_addr = Ipv4Addr::new(203, 0, 113, 1);
    let external_port = 12345;
    let flags = 0x8000;

    create_teredo_address(server, external_addr, external_port, flags)
}

/// Display implementation for TeredoAddress
impl fmt::Display for TeredoAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TeredoAddress {{ server: {}, external: {}:{}, flags: 0x{:04x}, ipv6: {} }}",
            self.server, self.external_addr, self.external_port, self.flags, self.ipv6_addr
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv6_mapped_creation() {
        let ipv4 = Ipv4Addr::new(192, 0, 2, 1);
        let mapped = ipv6_mapped(ipv4);

        assert_eq!(mapped.octets()[0..12], IPV4_MAPPED_PREFIX);
        assert_eq!(mapped.octets()[12..16], [192, 0, 2, 1]);
    }

    #[test]
    fn test_teredo_address_creation() {
        let server = Ipv4Addr::new(192, 0, 2, 1);
        let external_addr = Ipv4Addr::new(203, 0, 113, 1);
        let external_port = 12345;
        let flags = 0x8000;

        let teredo = create_teredo_address(server, external_addr, external_port, flags);

        assert_eq!(teredo.server, server);
        assert_eq!(teredo.external_addr, external_addr);
        assert_eq!(teredo.external_port, external_port);
        assert_eq!(teredo.flags, flags);
    }

    #[test]
    fn test_address_classification() {
        let private_v4 = SocketAddr::new("192.168.1.1".parse().unwrap(), 80);
        assert_eq!(classify_address(&private_v4), AddressType::Private);

        let public_v4 = SocketAddr::new("8.8.8.8".parse().unwrap(), 53);
        assert_eq!(classify_address(&public_v4), AddressType::Ipv4);
    }
}
