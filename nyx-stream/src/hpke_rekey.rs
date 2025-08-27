//! Minimal helpers to exercise rekey flows at the stream layer.
//! This does not perform HPKE itself; it relies on nyx-crypto sessions.
#![forbid(unsafe_code)]

use nyx_crypto::aead::{AeadKey, AeadSuite};
use nyx_crypto::session::AeadSession;

/// Small facade to create paired TX/RX sessions and tick counters to hit rekey.
pub struct RekeyHarness {
    pub tx: AeadSession,
    pub rx: AeadSession,
}

impl RekeyHarness {
    /// Build a pair with the same initial key/nonce and a record-based rekey interval.
    pub fn new_with_record_threshold(threshold: u64) -> Self {
        let key = AeadKey([42u8; 32]);
        let base = [9u8; 12];
        let tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base)
            .with_rekey_interval(threshold)
            .withdirection_id(1);
        let rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([42u8; 32]), base)
            .withdirection_id(1);
        Self { tx, rx }
    }

    /// Build a pair with a bytes-based rekey threshold on the sender.
    pub fn new_with_bytes_threshold(bytes: u64) -> Self {
        let key = AeadKey([42u8; 32]);
        let base = [9u8; 12];
        let tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base)
            .with_rekey_interval(u64::MAX)
            .with_rekey_bytes_interval(bytes)
            .withdirection_id(1);
        let rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([42u8; 32]), base)
            .withdirection_id(1);
        Self { tx, rx }
    }

    /// Send one message through the encryption/decryption roundtrip
    ///
    /// This method encrypts a plaintext message using the transmit session,
    /// then immediately decrypts it using the receive session. This is primarily
    /// used for testing rekey functionality and verifying session compatibility.
    ///
    /// # Arguments
    /// * `aad` - Additional authenticated data for the encryption
    /// * `pt` - Plaintext data to encrypt and decrypt
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - Successfully decrypted plaintext
    /// * `Err(String)` - Encryption or decryption failure with error details
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
            .tx
            .sealnext(aad, pt)
            .map_err(|seal_error| format!("Failed to encrypt message: {seal_error}"))?;

        // Attempt to open the ciphertext with the receive session
        let decrypted = self
            .rx
            .open_at(sequencenumber, aad, &ciphertext)
            .map_err(|open_error| {
                format!(
                    "Failed to decrypt message at sequence {sequencenumber}: {open_error}"
                )
            })?;

        Ok(decrypted)
    }
}
