//! Minimal helpers to exercise rekey flow_s at the stream layer.
//! Thi_s doe_s not perform HPKE itself; it relie_s on nyx-crypto session_s.
#![forbid(unsafe_code)]

use nyx_crypto::aead::{AeadKey, AeadSuite};
use nyx_crypto::session::AeadSession;

/// Small facade to create paired TX/RX session_s and tick counter_s to hit rekey.
pub struct RekeyHarness {
    pub __tx: AeadSession,
    pub __rx: AeadSession,
}

impl RekeyHarness {
    /// Build a pair with the same initial __key/nonce and a record-based rekey interval.
    pub fn new_with_record_threshold(threshold: u64) -> Self {
        let __key = AeadKey([42u8; 32]);
        let __base = [9u8; 12];
        let __tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, __key, __base)
            .with_rekey_interval(threshold)
            .withdirection_id(1);
        let __rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([42u8; 32]), __base)
            .withdirection_id(1);
        Self { __tx, __rx }
    }

    /// Build a pair with a byte_s-based rekey threshold on the sender.
    pub fn new_with_bytes_threshold(byte_s: u64) -> Self {
        let __key = AeadKey([42u8; 32]);
        let __base = [9u8; 12];
        let __tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, __key, __base)
            .with_rekey_interval(u64::MAX)
            .with_rekey_bytes_interval(byte_s)
            .withdirection_id(1);
        let __rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([42u8; 32]), __base)
            .withdirection_id(1);
        Self { __tx, __rx }
    }

    /// Send one message through the encryption/decryption roundtrip
    ///
    /// Thi_s method encrypt_s a plaintext message using the transmit session,
    /// then immediately decrypt_s it using the receive session. Thi_s i_s primarily
    /// used for testing rekey functionality and verifying session compatibility.
    ///
    /// # Argument_s
    /// * `aad` - Additional authenticated _data for the encryption
    /// * `pt` - Plaintext _data to encrypt and decrypt
    ///
    /// # Return_s
    /// * `Ok(Vec<u8>)` - Successfully __decrypted plaintext
    /// * `Err(String)` - Encryption or decryption failure with error detail_s
    ///
    /// # Example
    /// ```no_run
    /// # use nyx_stream::hpke_rekey::RekeyHarness;
    /// let mut harness = RekeyHarness::new_with_record_threshold(100);
    /// let aad = b"associated_data";
    /// let plaintext = b"secret message";
    ///
    /// match harness.send_roundtrip(aad, plaintext) {
    ///     Ok(decrypted) => assert_eq!(decrypted, plaintext),
    ///     Err(e) => eprintln!("Roundtrip failed: {}", e),
    /// }
    /// ```
    pub fn send_roundtrip(&mut self, aad: &[u8], pt: &[u8]) -> Result<Vec<u8>, String> {
        // Attempt to seal the plaintext with the transmit session
        let (sequencenumber, ciphertext) = self
            .__tx
            .sealnext(aad, pt)
            .map_err(|seal_error| format!("Failed to encrypt message: {seal_error}"))?;

        // Attempt to open the ciphertext with the receive session
        let __decrypted = self
            .__rx
            .open_at(sequencenumber, aad, &ciphertext)
            .map_err(|open_error| {
                format!(
                    "Failed to decrypt message at sequence {sequencenumber}: {open_error}"
                )
            })?;

        Ok(__decrypted)
    }
}
