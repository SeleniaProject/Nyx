//! Lightweight STUN Binding responder (RFC 5389) suitable for test nets.
//!
//! * Listens on a UDP socket (IPv4) and responds to Binding Requests.
//! * Handles XOR-MAPPED-ADDRESS attribute construction.
//! * Not intended for production at scale; only to aid Nyx dev / local tests.
//!
//! # Example
//! ```ignore
//! // Launch STUN server on port 3478 (tokio runtime required)
//! # async {
//! let handle = nyx_transport::stun_server::start_stun_server(3478).await.unwrap();
//! handle.await.unwrap();
//! # }
//! ```

#![forbid(unsafe_code)]

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::{net::UdpSocket, task::JoinHandle};
use tracing::{info, error};

const STUN_BINDING_REQUEST: u16 = 0x0001;
const STUN_BINDING_RESPONSE: u16 = 0x0101;
const STUN_MAGIC_COOKIE: u32 = 0x2112A442;
const XOR_MAPPED_ADDR: u16 = 0x0020;

/// Start a background STUN server on `0.0.0.0:port`.
pub async fn start_stun_server(port: u16) -> std::io::Result<JoinHandle<()>> {
    let socket = UdpSocket::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))).await?;
    info!("STUN server listening on {}", socket.local_addr()?);
    Ok(tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, src)) => {
                    if len < 20 { continue; }
                    let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
                    if msg_type != STUN_BINDING_REQUEST { continue; }
                    let txid = &buf[8..20];
                    let resp = build_binding_response(src, txid);
                    if socket.send_to(&resp, src).await.is_err() { error!("failed to send STUN response"); }
                }
                Err(e) => {
                    error!("stun recv error: {}", e);
                }
            }
        }
    }))
} 

/// Construct a STUN Binding Success Response with single XOR-MAPPED-ADDRESS attribute.
fn build_binding_response(src: SocketAddr, txid: &[u8]) -> Vec<u8> {
    // STUN header (20 bytes) + XOR-MAPPED-ADDRESS attribute (4 + 8 = 12 bytes) = 32 bytes
    let mut resp = Vec::with_capacity(32);
    resp.extend_from_slice(&STUN_BINDING_RESPONSE.to_be_bytes()); // Type
    // Message Length will be filled later after attributes are appended
    resp.extend_from_slice(&[0,0]);
    resp.extend_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
    resp.extend_from_slice(txid); // 12 bytes
    // Attribute start
    resp.extend_from_slice(&XOR_MAPPED_ADDR.to_be_bytes());
    resp.extend_from_slice(&8u16.to_be_bytes());
    resp.push(0); // reserved
    resp.push(0x01); // Family IPv4
    // Compute XORed port: port XOR the most significant 16 bits of the magic cookie
    let x_port = if let SocketAddr::V4(v4) = src { v4.port() } else { 0 } ^ ((STUN_MAGIC_COOKIE >> 16) as u16);
    resp.extend_from_slice(&x_port.to_be_bytes());
    if let IpAddr::V4(ipv4) = src.ip() {
        let cookie = STUN_MAGIC_COOKIE.to_be_bytes();
        for (i, b) in ipv4.octets().iter().enumerate() {
            // XOR each IPv4 octet with corresponding cookie byte
            resp.push((*b) ^ cookie[i]);
        }
    } else {
        resp.extend_from_slice(&[0,0,0,0]);
    }
    // Now set message length (bytes after the 20-byte header). Only one attribute: 4(header)+8(value)=12
    let msg_len = 12u16;
    resp[2..4].copy_from_slice(&msg_len.to_be_bytes());
    resp
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn stun_round_trip() {
        let handle = start_stun_server(3480).await.unwrap();
        // simple client send binding request (reuse existing code path in ice.rs?)
        use tokio::net::UdpSocket;
        let sock = UdpSocket::bind("0.0.0.0:0").await.unwrap();
        let mut req = [0u8;20];
        // minimal binding request
        req[0..2].copy_from_slice(&STUN_BINDING_REQUEST.to_be_bytes());
        req[4..8].copy_from_slice(&STUN_MAGIC_COOKIE.to_be_bytes());
        sock.send_to(&req, ("127.0.0.1",3480)).await.unwrap();
        let mut buf=[0u8;1500];
        let (len,_) = timeout(Duration::from_millis(500), sock.recv_from(&mut buf)).await.unwrap().unwrap();
        assert!(len >= 32, "unexpected STUN response length: {}", len);
        // Basic header checks
        assert_eq!(u16::from_be_bytes([buf[0],buf[1]]), STUN_BINDING_RESPONSE);
        drop(handle);
    }
}