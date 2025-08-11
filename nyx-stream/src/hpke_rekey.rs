#![forbid(unsafe_code)]

//! HPKE-based rekey frame helpers.
//!
//! This module introduces a **Cryptographic Rekey Frame** that transports a
//! freshly generated symmetric session key encrypted to the peer using
//! RFC 9180 HPKE (mode 0 = base).  The frame is carried inside the Nyx
//! *CRYPTO* packet category (type = 2, flags = 0) and has the following layout:
//!
//! ```text
//!  0               1               2               3
//! +---------------+---------------+---------------+---------------+
//! |  EncLen (16)  |        EncappedKey (N bytes)  |  ...          |
//! +---------------+---------------+---------------+---------------+
//! |  CtLen (16)   |     Ciphertext (M bytes)      |  ...          |
//! +---------------+---------------+---------------+---------------+
//! ```
//! * `EncLen`  – length of the HPKE encapped key in octets.
//! * `CtLen`   – length of the HPKE ciphertext that wraps the 32-byte     
//!               Nyx session key.
//!
//! Both length fields are unsigned big-endian 16-bit integers.  The current
//! implementation relies on the [`hpke`] crate’s X25519-HKDF-SHA256 KEM and
//! ChaCha20-Poly1305 AEAD, matching the Nyx cryptographic baseline.
//!
//! The helper API is deliberately kept **stateless** – callers supply the
//! remote party’s HPKE public key when sealing and their own private key when
//! opening.  This avoids having to thread additional context through the
//! stream layer while still providing convenient one-shot functions.

use nom::{number::complete::be_u16, bytes::complete::take, IResult};
use nyx_crypto::noise::SessionKey;
use nyx_crypto::hpke::{PublicKey, PrivateKey, generate_and_seal_session, open_session, HpkeError};
#[cfg(feature="telemetry")]
use nyx_telemetry::{inc_hpke_rekey_applied, inc_hpke_rekey_failure, inc_hpke_rekey_failure_reason};
use crate::HpkeRekeyManager; // manager resides in same crate guarded by feature

/// Frame carrying an HPKE-encrypted rekey blob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RekeyFrame {
    /// Encapped KEM public key (variable-length).
    pub encapped_key: Vec<u8>,
    /// Ciphertext that seals a fresh 32-byte session key.
    pub ciphertext: Vec<u8>,
}

/// Serialise a [`RekeyFrame`] to bytes suitable for on-wire transmission.
#[must_use]
pub fn build_rekey_frame(frame: &RekeyFrame) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + frame.encapped_key.len() + frame.ciphertext.len());
    out.extend_from_slice(&(frame.encapped_key.len() as u16).to_be_bytes());
    out.extend_from_slice(&frame.encapped_key);
    out.extend_from_slice(&(frame.ciphertext.len() as u16).to_be_bytes());
    out.extend_from_slice(&frame.ciphertext);
    out
}

/// Parse a rekey frame from the input byte slice.
pub fn parse_rekey_frame(input: &[u8]) -> IResult<&[u8], RekeyFrame> {
    let (input, enc_len) = be_u16(input)?;
    let (input, enc) = take(enc_len)(input)?;
    let (input, ct_len) = be_u16(input)?;
    let (input, ct) = take(ct_len)(input)?;
    Ok((input, RekeyFrame { encapped_key: enc.to_vec(), ciphertext: ct.to_vec() }))
}

/// Seal a fresh session key to `remote_pk` and return the on-wire frame **and**
/// the locally generated [`SessionKey`].  The caller MUST switch to the new
/// key immediately after sending the frame.
pub fn seal_for_rekey(remote_pk: &PublicKey, info: &[u8]) -> Result<(RekeyFrame, SessionKey), HpkeError> {
    let (enc, ct, sk) = generate_and_seal_session(remote_pk, info, b"")?;
    Ok((RekeyFrame { encapped_key: enc, ciphertext: ct }, sk))
}

/// Open a received rekey frame using our private key `sk_r` and return the
/// decrypted [`SessionKey`].  The caller MUST adopt the new key *before*
/// acknowledging the frame to avoid key desynchronisation.
pub fn open_rekey(sk_r: &PrivateKey, frame: &RekeyFrame, info: &[u8]) -> Result<SessionKey, HpkeError> {
    open_session(sk_r, &frame.encapped_key, info, b"", &frame.ciphertext)
}

/// Process an inbound rekey frame bytes: parse -> decrypt -> install.
/// Returns Ok(()) on success; increments telemetry counters when enabled.
pub fn process_inbound_rekey(manager: &mut HpkeRekeyManager, sk_r: &PrivateKey, bytes: &[u8], info: &[u8]) -> Result<(), HpkeError> {
    // Parse
    let (_rest, frame) = match parse_rekey_frame(bytes) {
        Ok(ok) => ok,
        Err(_) => {
            #[cfg(feature="telemetry")]
            inc_hpke_rekey_failure_reason("parse");
            return Err(HpkeError::OpenError);
        }
    }; // reuse OpenError for parse failures
    // Decrypt
    match open_rekey(sk_r, &frame, info) {
        Ok(session_key) => {
            manager.accept_remote_rekey(session_key);
            #[cfg(feature="telemetry")]
            inc_hpke_rekey_applied();
            Ok(())
        }
        Err(e) => {
            #[cfg(feature="telemetry")]
            {
                inc_hpke_rekey_failure();
                inc_hpke_rekey_failure_reason("decrypt");
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyx_crypto::hpke::{generate_keypair};

    #[test]
    fn frame_roundtrip() {
        let (_, pk) = generate_keypair();
        let (frame, _key) = seal_for_rekey(&pk, b"nyx-rekey-test").unwrap();
        let bytes = build_rekey_frame(&frame);
        let (_, parsed) = parse_rekey_frame(&bytes).unwrap();
        assert_eq!(frame, parsed);
    }

    #[test]
    fn key_exchange_success() {
        let (sk_r, pk_r) = generate_keypair();
        let (frame, local_key) = seal_for_rekey(&pk_r, b"rekey").unwrap();
        let remote_key = open_rekey(&sk_r, &frame, b"rekey").unwrap();
        assert_eq!(local_key.0, remote_key.0);
    }

    #[test]
    fn inbound_process_success() {
        use crate::{HpkeRekeyManager, RekeyPolicy};
        let (sk_r, pk_r) = generate_keypair();
        let (frame, _local_key) = seal_for_rekey(&pk_r, b"rekey-test").unwrap();
        let initial = nyx_crypto::noise::SessionKey([9u8;32]);
    let policy = RekeyPolicy { time_interval: std::time::Duration::from_secs(999), packet_interval: 1_000_000, grace_period: std::time::Duration::from_secs(1), min_cooldown: std::time::Duration::from_millis(0) };
        let mut mgr = HpkeRekeyManager::new(policy, initial);
        let bytes = build_rekey_frame(&frame);
        process_inbound_rekey(&mut mgr, &sk_r, &bytes, b"rekey-test").unwrap();
        assert_ne!(mgr.current_key().0, [9u8;32]);
    }

    #[test]
    fn parse_failure_returns_error() {
        use crate::{HpkeRekeyManager, RekeyPolicy};
        let (sk_r, pk_r) = generate_keypair();
        let (_frame, _local_key) = seal_for_rekey(&pk_r, b"ctx").unwrap();
        // Malformed bytes: declare enc_len=5 but only provide 1 byte, should trigger parse error -> OpenError mapping
        let malformed: Vec<u8> = vec![0,5, 0xAA, 0,0];
        let initial = nyx_crypto::noise::SessionKey([7u8;32]);
    let policy = RekeyPolicy { time_interval: std::time::Duration::from_secs(1000), packet_interval: 1_000_000, grace_period: std::time::Duration::from_secs(1), min_cooldown: std::time::Duration::from_millis(0) };
        let mut mgr = HpkeRekeyManager::new(policy, initial);
        let err = process_inbound_rekey(&mut mgr, &sk_r, &malformed, b"ctx").err().expect("expected error");
        // We cannot easily pattern match HpkeError variants here without exposing; acceptance is that it errored.
        let _ = err; // silence warning
        assert_eq!(mgr.current_key().0, [7u8;32]); // key unchanged
    }

    #[test]
    fn decrypt_failure_returns_error() {
        use crate::{HpkeRekeyManager, RekeyPolicy};
        // Create frame for recipient A but attempt to open with recipient B's private key.
        let (sk_a, pk_a) = generate_keypair();
        let (sk_b, _pk_b) = generate_keypair();
        let (frame, _local_key) = seal_for_rekey(&pk_a, b"mismatch").unwrap();
        let bytes = build_rekey_frame(&frame);
        let initial = nyx_crypto::noise::SessionKey([3u8;32]);
    let policy = RekeyPolicy { time_interval: std::time::Duration::from_secs(1000), packet_interval: 1_000_000, grace_period: std::time::Duration::from_secs(1), min_cooldown: std::time::Duration::from_millis(0) };
        let mut mgr = HpkeRekeyManager::new(policy, initial);
        let err = process_inbound_rekey(&mut mgr, &sk_b, &bytes, b"mismatch").err().expect("expected decrypt failure");
        let _ = err;
        assert_eq!(mgr.current_key().0, [3u8;32]);
        // Ensure using correct private key still works
        process_inbound_rekey(&mut mgr, &sk_a, &bytes, b"mismatch").unwrap();
        assert_ne!(mgr.current_key().0, [3u8;32]);
    }
} 