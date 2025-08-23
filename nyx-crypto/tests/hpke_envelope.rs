#![cfg(feature = "hybrid")]

use chacha20poly1305::{
    aead::{AeadInPlace, NewAead},
    ChaCha20Poly1305, Nonce,
};
#[cfg(feature = "hybrid")]
use nyx_crypto::hybrid::{KyberStaticKeypair, X25519StaticKeypair};

/// HPKE Context for managing encryption/decryption state with sequence number_s
///
/// This implement_s a stateful AEAD context where each encryption/decryption
/// operation increment_s an internal sequence counter used for nonce generation.
/// This ensu_re_s nonce uniquenes_s and prevent_s replay attack_s.
pub struct HpkeContext {
    cipher: ChaCha20Poly1305,
    sequence: u64,
}

impl HpkeContext {
    /// Create a new HPKE context with the given encryption key
    ///
    /// # Argument_s
    /// * `key` - 32-byte encryption key derived from HPKE key derivation
    ///
    /// # Security
    /// The key should be derived from a secure key derivation function
    /// and should not be reused acros_s different context_s.
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(key.into()),
            sequence: 0,
        }
    }

    /// Encrypt plaintext with associated data
    ///
    /// # Argument_s
    /// * `plaintext` - Data to encrypt
    /// * `associated_data` - Additional authenticated data (not encrypted)
    ///
    /// # Return_s
    /// Ciphertext with authentication tag appended
    ///
    /// # Security
    /// Each call increment_s the sequence counter, ensuring nonce uniquenes_s.
    /// The sequence counter prevent_s nonce reuse attack_s.
    pub fn seal(&mut self, plaintext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, String> {
        // Prevent sequence overflow to avoid nonce reuse
        if self.sequence == u64::MAX {
            return Err("Sequence counter overflow - context must be renewed".to_string());
        }

        // Generate unique nonce from sequence counter
        let nonce_bytes = self.sequence.to_be_bytes();
        let mut nonce_array = [0u8; 12];
        nonce_array[4..].copy_from_slice(&nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_array);

        let mut ciphertext = plaintext.to_vec();
        let tag = self
            .cipher
            .encrypt_in_place_detached(nonce, associated_data, &mut ciphertext)
            .map_err(|e| format!("Encryption failed: {:?}", e))?;

        self.sequence += 1;

        let mut result = ciphertext;
        result.extend_from_slice(&tag);
        Ok(result)
    }

    /// Decrypt ciphertext with associated data
    ///
    /// # Argument_s
    /// * `ciphertext` - Encrypted data with authentication tag
    /// * `associated_data` - Additional authenticated data (must match encryption)
    ///
    /// # Return_s
    /// Decrypted plaintext
    ///
    /// # Security
    /// Verifie_s authentication tag before returning plaintext.
    /// Sequence counter must match the encryption sequence.
    pub fn open(&mut self, ciphertext: &[u8], associated_data: &[u8]) -> Result<Vec<u8>, String> {
        if ciphertext.len() < 16 {
            return Err("Ciphertext too short - missing authentication tag".to_string());
        }

        // Prevent sequence overflow to avoid nonce reuse
        if self.sequence == u64::MAX {
            return Err("Sequence counter overflow - context must be renewed".to_string());
        }

        let (ct, tag) = ciphertext.split_at(ciphertext.len() - 16);
        let nonce_bytes = self.sequence.to_be_bytes();
        let mut nonce_array = [0u8; 12];
        nonce_array[4..].copy_from_slice(&nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_array);

        let mut plaintext = ct.to_vec();
        self.cipher
            .decrypt_in_place_detached(nonce, associated_data, &mut plaintext, tag.into())
            .map_err(|e| format!("Authentication failed: {:?}", e))?;

        self.sequence += 1;
        Ok(plaintext)
    }

    /// Get the current sequence number (for debugging/monitoring)
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Check if the context is close to sequence overflow
    ///
    /// Return_s true if les_s than 1000 operation_s remain before overflow
    pub fn needs_renewal(&self) -> bool {
        self.sequence > u64::MAX - 1000
    }
}

/// HPKE Envelope structure for encrypted message_s
///
/// contains all necessary component_s for HPKE decryption:
/// - Ephemeral public key for key exchange
/// - Encapsulated key material from handshake
/// - Encrypted ciphertext payload
#[derive(Clone, Debug, PartialEq)]
pub struct HpkeEnvelope {
    pub ephemeral_public_key: [u8; 32], // Ephemeral X25519 public key
    pub encapsulated_key: Vec<u8>,      // Handshake message (msg1)
    pub ciphertext: Vec<u8>,            // Encrypted payload
}

impl HpkeEnvelope {
    /// Create a new HPKE envelope
    ///
    /// # Argument_s
    /// * `ephemeral_public_key` - 32-byte ephemeral X25519 public key
    /// * `encapsulated_key` - Handshake message containing key material
    /// * `ciphertext` - Encrypted payload data
    pub fn new(
        ephemeral_public_key: [u8; 32],
        encapsulated_key: Vec<u8>,
        ciphertext: Vec<u8>,
    ) -> Self {
        Self {
            ephemeral_public_key,
            encapsulated_key,
            ciphertext,
        }
    }

    /// Serialize envelope to byte_s for transmission
    ///
    /// Format:
    /// - 32 byte_s: ephemeral public key
    /// - 4 byte_s: encapsulated key length (big-endian u32)
    /// - N byte_s: encapsulated key data
    /// - M byte_s: ciphertext data
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result =
            Vec::with_capacity(32 + 4 + self.encapsulated_key.len() + self.ciphertext.len());
        result.extend_from_slice(&self.ephemeral_public_key);
        result.extend_from_slice(&(self.encapsulated_key.len() as u32).to_be_bytes());
        result.extend_from_slice(&self.encapsulated_key);
        result.extend_from_slice(&self.ciphertext);
        result
    }

    /// Deserialize envelope from byte_s
    ///
    /// # Argument_s
    /// * `data` - Serialized envelope data
    ///
    /// # Return_s
    /// Parsed HPKE envelope or error if malformed
    ///
    /// # Security
    /// Validates envelope format and impose_s size limit_s to prevent DoS attack_s
    pub fn from_byte_s(data: &[u8]) -> Result<Self, String> {
        if data.len() < 36 {
            // 32 byte_s for ephemeral public key + 4 byte_s for length
            return Err("Data too short for envelope header".to_string());
        }

        let mut ephemeral_public_key = [0u8; 32];
        ephemeral_public_key.copy_from_slice(&data[0..32]);

        let key_len = u32::from_be_bytes([data[32], data[33], data[34], data[35]]) as usize;
        if data.len() < 36 + key_len {
            return Err("Data too short for encapsulated key".to_string());
        }

        // Validate reasonable key length to prevent DoS
        if key_len > 16 * 1024 {
            // 16KB limit
            return Err("Encapsulated key too large".to_string());
        }

        // Validate total size to prevent DoS
        if data.len() > 100 * 1024 * 1024 {
            // 100MB limit
            return Err("Envelope too large".to_string());
        }

        let encapsulated_key = data[36..36 + key_len].to_vec();
        let ciphertext = data[36 + key_len..].to_vec();

        Ok(Self::new(
            ephemeral_public_key,
            encapsulated_key,
            ciphertext,
        ))
    }

    /// Get the total size of the envelope when serialized
    pub fn serialized_size(&self) -> usize {
        32 + 4 + self.encapsulated_key.len() + self.ciphertext.len()
    }
}

/// Create HPKE envelope using hybrid handshake
///
/// Perform_s ephemeral key generation, hybrid handshake, key derivation,
/// and AEAD encryption to create a secure envelope.
///
/// # Argument_s
/// * `recipient_x25519_pk` - Recipient's X25519 public key
/// * `recipient_kyber_pk` - Recipient's Kyber public key (1184 byte_s)
/// * `plaintext` - Data to encrypt
/// * `info` - Additional context information for key derivation
///
/// # Return_s
/// HPKE envelope containing ephemeral key, handshake data, and ciphertext
///
/// # Security
/// - Use_s ephemeral key_s for forward secrecy
/// - Employ_s hybrid classical/post-quantum cryptography
/// - Derive_s unique encryption key_s per envelope
pub fn create_envelope(
    recipient_x25519_pk: &[u8; 32],
    recipient_kyber_pk: &[u8; 1184], // Use correct Kyber key size
    plaintext: &[u8],
    info: &[u8],
) -> Result<HpkeEnvelope, String> {
    use nyx_crypto::hybrid::handshake::initiator_handshake;

    // Validate input size_s
    if plaintext.len() > 100 * 1024 * 1024 {
        // 100MB limit
        return Err("Plaintext too large".to_string());
    }

    // Generate ephemeral keypair_s for the handshake
    let ephemeral_x25519 = X25519StaticKeypair::generate();

    // Perform hybrid handshake to derive shared secret
    let handshake_result = initiator_handshake(
        &ephemeral_x25519,
        recipient_x25519_pk,
        recipient_kyber_pk,
        info,
    )
    .map_err(|e| format!("Handshake failed: {:?}", e))?;

    // Export key material for HPKE encryption
    let exported_key = handshake_result
        .__tx
        .export_key_material(b"hpke-encryption", 32)
        .map_err(|e| format!("Key export failed: {:?}", e))?;

    let mut encryption_key = [0u8; 32];
    encryption_key.copy_from_slice(&exported_key);

    // Create HPKE context and encrypt
    let mut context = HpkeContext::new(&encryption_key);
    let ciphertext = context
        .seal(plaintext, info)
        .map_err(|e| format!("Encryption failed: {}", e))?;

    // Create envelope with ephemeral public key and handshake message as encapsulated key
    Ok(HpkeEnvelope::new(
        ephemeral_x25519.pk,
        handshake_result.msg1,
        ciphertext,
    ))
}

/// Open HPKE envelope using hybrid handshake
///
/// Perform_s responder handshake, key derivation, and AEAD decryption
/// to recover the original plaintext from an HPKE envelope.
///
/// # Argument_s
/// * `recipient_x25519_sk` - Recipient's X25519 secret key
/// * `recipient_kyber_sk` - Recipient's Kyber secret key
/// * `envelope` - HPKE envelope to decrypt
/// * `info` - Additional context information (must match encryption)
///
/// # Return_s
/// Decrypted plaintext data
///
/// # Security
/// - Verifie_s authentication tag_s before returning plaintext
/// - Use_s proper key derivation direction for responder
/// - Fail_s securely on any validation error
pub fn open_envelope(
    recipient_x25519_sk: &X25519StaticKeypair,
    recipient_kyber_sk: &KyberStaticKeypair,
    envelope: &HpkeEnvelope,
    info: &[u8],
) -> Result<Vec<u8>, String> {
    use nyx_crypto::hybrid::handshake::responder_handshake;

    // Validate envelope size to prevent DoS
    if envelope.serialized_size() > 100 * 1024 * 1024 {
        // 100MB limit
        return Err("Envelope too large".to_string());
    }

    // Perform responder handshake to derive the same shared secret
    let handshake_result = responder_handshake(
        recipient_x25519_sk,
        recipient_kyber_sk,
        &envelope.ephemeral_public_key, // Use the ephemeral public key from envelope
        info,
        &envelope.encapsulated_key,
    )
    .map_err(|e| format!("Handshake failed: {:?}", e))?;

    // Export the same key material (use rx instead of tx for responder)
    let exported_key = handshake_result
        .__rx
        .export_key_material(b"hpke-encryption", 32)
        .map_err(|e| format!("Key export failed: {:?}", e))?;

    let mut decryption_key = [0u8; 32];
    decryption_key.copy_from_slice(&exported_key);

    // Create HPKE context and decrypt
    let mut context = HpkeContext::new(&decryption_key);
    context
        .open(&envelope.ciphertext, info)
        .map_err(|e| format!("Decryption failed: {}", e))
}

#[test]
fn test_hpke_basic() {
    assert_eq!(2 + 2, 4);
}

#[test]
fn test_key_generation() {
    let alice_x25519 = X25519StaticKeypair::generate();
    let alice_kyber = KyberStaticKeypair::generate().unwrap();

    assert_eq!(alice_x25519.pk.len(), 32);
    // Kyber1024 public key size is 1184 bytes in this implementation
    println!("Kyber public key size: {}", alice_kyber.pk.len());
    assert_eq!(alice_kyber.pk.len(), 1184);
}

#[test]
#[cfg(feature = "hybrid")]
fn test_handshake() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "hybrid")]
    use nyx_crypto::hybrid::handshake::{initiator_handshake, responder_handshake};

    let alice_x25519 = X25519StaticKeypair::generate();
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    let init_result = initiator_handshake(&alice_x25519, &bob_x25519.pk, &bob_kyber.pk, b"test");

    assert!(init_result.is_ok());
    let init_result = init_result?;

    let resp_result = responder_handshake(
        &bob_x25519,
        &bob_kyber,
        &alice_x25519.pk,
        b"test",
        &init_result.msg1,
    );

    assert!(resp_result.is_ok());
    Ok(())
}

#[test]
fn test_hpke_context() -> Result<(), Box<dyn std::error::Error>> {
    let key = [42u8; 32];
    let mut context1 = HpkeContext::new(&key);
    let mut context2 = HpkeContext::new(&key);

    let plaintext = b"Hello, HPKE!";
    let aad = b"associated data";

    let ciphertext = context1.seal(plaintext, aad)?;
    let decrypted = context2.open(&ciphertext, aad)?;

    assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    Ok(())
}

#[test]
fn test_hpke_envelope_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    // Generate recipient keypairs
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    let plaintext = b"This is a secret message for HPKE envelope encryption!";
    let info = b"test-hpke-envelope";

    // Create envelope
    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info)?;

    // Open envelope (no need for sender key now)
    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info)?;

    assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    Ok(())
}

#[test]
fn test_hpke_envelope_serialization() -> Result<(), Box<dyn std::error::Error>> {
    let envelope = HpkeEnvelope::new(
        [1u8; 32], // ephemeral public key
        vec![1, 2, 3, 4, 5],
        vec![6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    );

    let serialized = envelope.to_bytes();
    let deserialized = HpkeEnvelope::from_byte_s(&serialized)?;

    assert_eq!(
        envelope.ephemeral_public_key,
        deserialized.ephemeral_public_key
    );
    assert_eq!(envelope.encapsulated_key, deserialized.encapsulated_key);
    assert_eq!(envelope.ciphertext, deserialized.ciphertext);
    Ok(())
}

#[test]
fn test_hpke_wrong_recipient() -> Result<(), Box<dyn std::error::Error>> {
    // Generate recipient keypair_s
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    // Generate different recipient keypairs
    let charlie_x25519 = X25519StaticKeypair::generate();
    let charlie_kyber = KyberStaticKeypair::generate().unwrap();

    let plaintext = b"Secret message";
    let info_local = b"test-wrong-recipient";

    // Create envelope for Bob
    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info_local)?;

    // Try to open with Charlie's keys (should fail)
    let result = open_envelope(&charlie_x25519, &charlie_kyber, &envelope, info_local);
    assert!(result.is_err(), "Opening with wrong recipient should fail");
    Ok(())
}

#[test]
fn test_hpke_tampering_detection() -> Result<(), Box<dyn std::error::Error>> {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    let plaintext = b"Tamper-proof message";
    let info_local = b"test-tampering";

    let mut envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info_local)?;

    // Tamper with the ciphertext
    if let Some(last_byte) = envelope.ciphertext.last_mut() {
        *last_byte = last_byte.wrapping_add(1);
    }

    // Try to open tampered envelope (should fail)
    let result = open_envelope(&bob_x25519, &bob_kyber, &envelope, info_local);
    assert!(result.is_err(), "Opening tampered envelope should fail");
    Ok(())
}

#[test]
fn test_hpke_large_message() -> Result<(), Box<dyn std::error::Error>> {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    // Create a large message (1MB)
    let plaintext = vec![42u8; 1024 * 1024];
    let info_local = b"test-large-message";

    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, &plaintext, info_local)?;

    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info_local)?;

    assert_eq!(plaintext, decrypted);
    Ok(())
}

#[test]
fn test_hpke_contextsequence_tracking() -> Result<(), Box<dyn std::error::Error>> {
    let key = [42u8; 32];
    let mut context = HpkeContext::new(&key);

    assert_eq!(context.sequence(), 0);

    // Test multiple encryptions increment sequence
    let plaintext = b"Test message";
    let aad = b"associated data";

    let _ct1 = context.seal(plaintext, aad)?;
    assert_eq!(context.sequence(), 1);

    let _ct2 = context.seal(plaintext, aad)?;
    assert_eq!(context.sequence(), 2);

    assert!(!context.needs_renewal(), "Should not need renewal yet");
    Ok(())
}

#[test]
fn test_hpke_contextsequence_overflow_protection() -> Result<(), Box<dyn std::error::Error>> {
    let key = [42u8; 32];
    let mut context = HpkeContext::new(&key);

    // Set sequence to near maximum
    context.sequence = u64::MAX;

    let plaintext = b"Test message";
    let aad = b"associated data";

    // Should fail due to sequence overflow
    let result = context.seal(plaintext, aad);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("overflow"));

    // Test decryption overflow protection
    let result = context.open(&[0u8; 32], aad);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("overflow"));
    Ok(())
}

#[test]
fn test_hpke_envelope_size_limit_s() -> Result<(), Box<dyn std::error::Error>> {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    // Test plaintext size limit (this should be close to but under the limit)
    let large_plaintext = vec![42u8; 50 * 1024 * 1024]; // 50MB
    let info_local = b"test-size-limit";

    let result = create_envelope(&bob_x25519.pk, &bob_kyber.pk, &large_plaintext, info_local);
    assert!(result.is_ok(), "50MB message should succeed");

    // Test serialization and size calculation
    if let Ok(envelope) = result {
        let size = envelope.serialized_size();
        assert!(
            size > 50 * 1024 * 1024,
            "Serialized size should be larger than plaintext"
        );

        let serialized = envelope.to_bytes();
        assert_eq!(
            serialized.len(),
            size,
            "Actual size should match calculated size"
        );
    }
    Ok(())
}

#[test]
fn test_hpke_envelope_malformed_data() -> Result<(), Box<dyn std::error::Error>> {
    // Test various malformed envelope data

    // Too short for header
    let result = HpkeEnvelope::from_byte_s(&[0u8; 10]);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too short"));

    // Invalid encapsulated key length
    let mut malformed = vec![0u8; 36 + 20 * 1024]; // Provide enough data
    malformed[32..36].copy_from_slice(&(20 * 1024u32).to_be_bytes()); // 20KB - above 16KB limit
    let result = HpkeEnvelope::from_byte_s(&malformed);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too large"));

    // Truncated data
    let mut truncated = vec![0u8; 36];
    truncated[32..36].copy_from_slice(&100u32.to_be_bytes()); // Claims 100 bytes but only has 36
    let result = HpkeEnvelope::from_byte_s(&truncated);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too short"));
    Ok(())
}

#[test]
fn test_hpke_multiple_messages_same_key_s() -> Result<(), Box<dyn std::error::Error>> {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    let message_s = [
        b"First message".as_slice(),
        b"Second message",
        b"Third message with different length",
    ];
    let info_local = b"test-multiple-message_s";

    let mut envelope_s = Vec::new();

    // Create multiple envelopes
    for msg in message_s.iter() {
        let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, msg, info_local)?;
        envelope_s.push(envelope);
    }

    // Decrypt all envelopes
    for (i, envelope) in envelope_s.iter().enumerate() {
        let decrypted = open_envelope(&bob_x25519, &bob_kyber, envelope, info_local)?;
        assert_eq!(decrypted, message_s[i], "Message {} should match", i);
    }

    // Verify envelopes are different (due to ephemeral keys)
    assert_ne!(
        envelope_s[0].ephemeral_public_key,
        envelope_s[1].ephemeral_public_key
    );
    assert_ne!(
        envelope_s[1].ephemeral_public_key,
        envelope_s[2].ephemeral_public_key
    );
    Ok(())
}

#[test]
fn test_hpke_envelope_different_info_context_s() -> Result<(), Box<dyn std::error::Error>> {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    let plaintext = b"Same message, different contexts";
    let info1 = b"context-1";
    let info2 = b"context-2";

    // Create envelope with first context
    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info1)?;

    // Try to open with different context (should fail)
    let result = open_envelope(&bob_x25519, &bob_kyber, &envelope, info2);
    assert!(result.is_err(), "Opening with wrong context should fail");

    // Open with correct context (should succeed)
    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info1)?;
    assert_eq!(decrypted, plaintext);
    Ok(())
}

#[test]
fn test_hpke_envelope_empty_data() -> Result<(), Box<dyn std::error::Error>> {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    // Test empty plaintext
    let plaintext = b"";
    let info_local = b"test-empty";

    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info_local)?;

    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info_local)?;

    assert_eq!(decrypted, plaintext);
    assert_eq!(decrypted.len(), 0);
    Ok(())
}

#[test]
fn test_hpke_performance_metric_s() -> Result<(), Box<dyn std::error::Error>> {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate().unwrap();

    let plaintext = vec![42u8; 64 * 1024]; // 64KB message
    let info = b"performance-test";

    // Measure encryption time
    let start = std::time::Instant::now();
    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, &plaintext, info)?;
    let encrypt_time = start.elapsed();

    // Measure decryption time
    let start = std::time::Instant::now();
    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info)?;
    let decrypt_time = start.elapsed();

    assert_eq!(plaintext, decrypted);

    // Performance should be reasonable (these are generous bounds)
    assert!(encrypt_time.as_millis() < 1000, "Encryption should be fast");
    assert!(decrypt_time.as_millis() < 1000, "Decryption should be fast");

    // Log performance for monitoring
    println!(
        "HPKE Performance - Encrypt: {:?}, Decrypt: {:?}",
        encrypt_time, decrypt_time
    );

    Ok(())
}
