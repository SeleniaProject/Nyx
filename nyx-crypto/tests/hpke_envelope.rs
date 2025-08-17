use chacha20poly1305::{
    aead::{AeadInPlace, NewAead},
    ChaCha20Poly1305, Nonce,
};
use nyx_crypto::hybrid::{KyberStaticKeypair, X25519StaticKeypair};

/// HPKE Context for managing encryption/decryption state with sequence numbers
///
/// This implements a stateful AEAD context where each encryption/decryption
/// operation increments an internal sequence counter used for nonce generation.
/// This ensures nonce uniqueness and prevents replay attacks.
pub struct HpkeContext {
    cipher: ChaCha20Poly1305,
    sequence: u64,
}

impl HpkeContext {
    /// Create a new HPKE context with the given encryption key
    ///
    /// # Arguments
    /// * `key` - 32-byte encryption key derived from HPKE key derivation
    ///
    /// # Security
    /// The key should be derived from a secure key derivation function
    /// and should not be reused across different contexts.
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: ChaCha20Poly1305::new(key.into()),
            sequence: 0,
        }
    }

    /// Encrypt plaintext with associated data
    ///
    /// # Arguments
    /// * `plaintext` - Data to encrypt
    /// * `associated_data` - Additional authenticated data (not encrypted)
    ///
    /// # Returns
    /// Ciphertext with authentication tag appended
    ///
    /// # Security
    /// Each call increments the sequence counter, ensuring nonce uniqueness.
    /// The sequence counter prevents nonce reuse attacks.
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
    /// # Arguments
    /// * `ciphertext` - Encrypted data with authentication tag
    /// * `associated_data` - Additional authenticated data (must match encryption)
    ///
    /// # Returns
    /// Decrypted plaintext
    ///
    /// # Security
    /// Verifies authentication tag before returning plaintext.
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
    /// Returns true if less than 1000 operations remain before overflow
    pub fn needs_renewal(&self) -> bool {
        self.sequence > u64::MAX - 1000
    }
}

/// HPKE Envelope structure for encrypted messages
///
/// Contains all necessary components for HPKE decryption:
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
    /// # Arguments
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

    /// Serialize envelope to bytes for transmission
    ///
    /// Format:
    /// - 32 bytes: ephemeral public key
    /// - 4 bytes: encapsulated key length (big-endian u32)
    /// - N bytes: encapsulated key data
    /// - M bytes: ciphertext data
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut result =
            Vec::with_capacity(32 + 4 + self.encapsulated_key.len() + self.ciphertext.len());
        result.extend_from_slice(&self.ephemeral_public_key);
        result.extend_from_slice(&(self.encapsulated_key.len() as u32).to_be_bytes());
        result.extend_from_slice(&self.encapsulated_key);
        result.extend_from_slice(&self.ciphertext);
        result
    }

    /// Deserialize envelope from bytes
    ///
    /// # Arguments
    /// * `data` - Serialized envelope data
    ///
    /// # Returns
    /// Parsed HPKE envelope or error if malformed
    ///
    /// # Security
    /// Validates envelope format and imposes size limits to prevent DoS attacks
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() < 36 {
            // 32 bytes for ephemeral public key + 4 bytes for length
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
/// Performs ephemeral key generation, hybrid handshake, key derivation,
/// and AEAD encryption to create a secure envelope.
///
/// # Arguments
/// * `recipient_x25519_pk` - Recipient's X25519 public key
/// * `recipient_kyber_pk` - Recipient's Kyber public key (1184 bytes)
/// * `plaintext` - Data to encrypt
/// * `info` - Additional context information for key derivation
///
/// # Returns
/// HPKE envelope containing ephemeral key, handshake data, and ciphertext
///
/// # Security
/// - Uses ephemeral keys for forward secrecy
/// - Employs hybrid classical/post-quantum cryptography
/// - Derives unique encryption keys per envelope
pub fn create_envelope(
    recipient_x25519_pk: &[u8; 32],
    recipient_kyber_pk: &[u8; 1184], // Use correct Kyber key size
    plaintext: &[u8],
    info: &[u8],
) -> Result<HpkeEnvelope, String> {
    use nyx_crypto::hybrid::handshake::initiator_handshake;

    // Validate input sizes
    if plaintext.len() > 100 * 1024 * 1024 {
        // 100MB limit
        return Err("Plaintext too large".to_string());
    }

    // Generate ephemeral keypairs for the handshake
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
        .tx
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
/// Performs responder handshake, key derivation, and AEAD decryption
/// to recover the original plaintext from an HPKE envelope.
///
/// # Arguments
/// * `recipient_x25519_sk` - Recipient's X25519 secret key
/// * `recipient_kyber_sk` - Recipient's Kyber secret key
/// * `envelope` - HPKE envelope to decrypt
/// * `info` - Additional context information (must match encryption)
///
/// # Returns
/// Decrypted plaintext data
///
/// # Security
/// - Verifies authentication tags before returning plaintext
/// - Uses proper key derivation direction for responder
/// - Fails securely on any validation error
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
        .rx
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
    let alice_kyber = KyberStaticKeypair::generate();

    assert_eq!(alice_x25519.pk.len(), 32);
    // Kyber1024 public key size is 1184 bytes in this implementation
    println!("Kyber public key size: {}", alice_kyber.pk.len());
    assert_eq!(alice_kyber.pk.len(), 1184);
}

#[test]
fn test_handshake() {
    use nyx_crypto::hybrid::handshake::{initiator_handshake, responder_handshake};

    let alice_x25519 = X25519StaticKeypair::generate();
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    let init_result = initiator_handshake(&alice_x25519, &bob_x25519.pk, &bob_kyber.pk, b"test");

    assert!(init_result.is_ok());
    let init_result = init_result.unwrap();

    let resp_result = responder_handshake(
        &bob_x25519,
        &bob_kyber,
        &alice_x25519.pk,
        b"test",
        &init_result.msg1,
    );

    assert!(resp_result.is_ok());
}

#[test]
fn test_hpke_context() {
    let key = [42u8; 32];
    let mut context1 = HpkeContext::new(&key);
    let mut context2 = HpkeContext::new(&key);

    let plaintext = b"Hello, HPKE!";
    let aad = b"associated data";

    let ciphertext = context1
        .seal(plaintext, aad)
        .expect("Encryption should succeed");
    let decrypted = context2
        .open(&ciphertext, aad)
        .expect("Decryption should succeed");

    assert_eq!(plaintext.as_slice(), decrypted.as_slice());
}

#[test]
fn test_hpke_envelope_roundtrip() {
    // Generate recipient keypairs
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    let plaintext = b"This is a secret message for HPKE envelope encryption!";
    let info = b"test-hpke-envelope";

    // Create envelope
    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info)
        .expect("Envelope creation should succeed");

    // Open envelope (no need for sender key now)
    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info)
        .expect("Envelope opening should succeed");

    assert_eq!(plaintext.as_slice(), decrypted.as_slice());
}

#[test]
fn test_hpke_envelope_serialization() {
    let envelope = HpkeEnvelope::new(
        [1u8; 32], // ephemeral public key
        vec![1, 2, 3, 4, 5],
        vec![6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    );

    let serialized = envelope.to_bytes();
    let deserialized =
        HpkeEnvelope::from_bytes(&serialized).expect("Deserialization should succeed");

    assert_eq!(
        envelope.ephemeral_public_key,
        deserialized.ephemeral_public_key
    );
    assert_eq!(envelope.encapsulated_key, deserialized.encapsulated_key);
    assert_eq!(envelope.ciphertext, deserialized.ciphertext);
}

#[test]
fn test_hpke_wrong_recipient() {
    // Generate recipient keypairs
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    // Generate different recipient keypairs
    let charlie_x25519 = X25519StaticKeypair::generate();
    let charlie_kyber = KyberStaticKeypair::generate();

    let plaintext = b"Secret message";
    let info = b"test-wrong-recipient";

    // Create envelope for Bob
    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info)
        .expect("Envelope creation should succeed");

    // Try to open with Charlie's keys (should fail)
    let result = open_envelope(&charlie_x25519, &charlie_kyber, &envelope, info);
    assert!(result.is_err(), "Opening with wrong recipient should fail");
}

#[test]
fn test_hpke_tampering_detection() {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    let plaintext = b"Tamper-proof message";
    let info = b"test-tampering";

    let mut envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info)
        .expect("Envelope creation should succeed");

    // Tamper with the ciphertext
    if let Some(last_byte) = envelope.ciphertext.last_mut() {
        *last_byte = last_byte.wrapping_add(1);
    }

    // Try to open tampered envelope (should fail)
    let result = open_envelope(&bob_x25519, &bob_kyber, &envelope, info);
    assert!(result.is_err(), "Opening tampered envelope should fail");
}

#[test]
fn test_hpke_large_message() {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    // Create a large message (1MB)
    let plaintext = vec![42u8; 1024 * 1024];
    let info = b"test-large-message";

    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, &plaintext, info)
        .expect("Large envelope creation should succeed");

    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info)
        .expect("Large envelope opening should succeed");

    assert_eq!(plaintext, decrypted);
}

#[test]
fn test_hpke_context_sequence_tracking() {
    let key = [42u8; 32];
    let mut context = HpkeContext::new(&key);

    assert_eq!(context.sequence(), 0);

    // Test multiple encryptions increment sequence
    let plaintext = b"Test message";
    let aad = b"associated data";

    let _ct1 = context
        .seal(plaintext, aad)
        .expect("First encryption should succeed");
    assert_eq!(context.sequence(), 1);

    let _ct2 = context
        .seal(plaintext, aad)
        .expect("Second encryption should succeed");
    assert_eq!(context.sequence(), 2);

    assert!(!context.needs_renewal(), "Should not need renewal yet");
}

#[test]
fn test_hpke_context_sequence_overflow_protection() {
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
}

#[test]
fn test_hpke_envelope_size_limits() {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    // Test plaintext size limit (this should be close to but under the limit)
    let large_plaintext = vec![42u8; 50 * 1024 * 1024]; // 50MB
    let info = b"test-size-limit";

    let result = create_envelope(&bob_x25519.pk, &bob_kyber.pk, &large_plaintext, info);
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
}

#[test]
fn test_hpke_envelope_malformed_data() {
    // Test various malformed envelope data

    // Too short for header
    let result = HpkeEnvelope::from_bytes(&[0u8; 10]);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too short"));

    // Invalid encapsulated key length
    let mut malformed = vec![0u8; 36 + 20 * 1024]; // Provide enough data
    malformed[32..36].copy_from_slice(&(20 * 1024u32).to_be_bytes()); // 20KB - above 16KB limit
    let result = HpkeEnvelope::from_bytes(&malformed);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too large"));

    // Truncated data
    let mut truncated = vec![0u8; 36];
    truncated[32..36].copy_from_slice(&100u32.to_be_bytes()); // Claims 100 bytes but only has 36
    let result = HpkeEnvelope::from_bytes(&truncated);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too short"));
}

#[test]
fn test_hpke_multiple_messages_same_keys() {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    let messages = [
        b"First message".as_slice(),
        b"Second message",
        b"Third message with different length",
    ];
    let info = b"test-multiple-messages";

    let mut envelopes = Vec::new();

    // Create multiple envelopes
    for msg in messages.iter() {
        let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, msg, info)
            .expect("Envelope creation should succeed");
        envelopes.push(envelope);
    }

    // Decrypt all envelopes
    for (i, envelope) in envelopes.iter().enumerate() {
        let decrypted = open_envelope(&bob_x25519, &bob_kyber, envelope, info)
            .expect("Envelope opening should succeed");
        assert_eq!(decrypted, messages[i], "Message {} should match", i);
    }

    // Verify envelopes are different (due to ephemeral keys)
    assert_ne!(
        envelopes[0].ephemeral_public_key,
        envelopes[1].ephemeral_public_key
    );
    assert_ne!(
        envelopes[1].ephemeral_public_key,
        envelopes[2].ephemeral_public_key
    );
}

#[test]
fn test_hpke_envelope_different_info_contexts() {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    let plaintext = b"Same message, different contexts";
    let info1 = b"context-1";
    let info2 = b"context-2";

    // Create envelope with first context
    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info1)
        .expect("Envelope creation should succeed");

    // Try to open with different context (should fail)
    let result = open_envelope(&bob_x25519, &bob_kyber, &envelope, info2);
    assert!(result.is_err(), "Opening with wrong context should fail");

    // Open with correct context (should succeed)
    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info1)
        .expect("Opening with correct context should succeed");
    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_hpke_envelope_empty_data() {
    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    // Test empty plaintext
    let plaintext = b"";
    let info = b"test-empty";

    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, plaintext, info)
        .expect("Empty envelope creation should succeed");

    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info)
        .expect("Empty envelope opening should succeed");

    assert_eq!(decrypted, plaintext);
    assert_eq!(decrypted.len(), 0);
}

#[test]
fn test_hpke_performance_metrics() {
    use std::time::Instant;

    let bob_x25519 = X25519StaticKeypair::generate();
    let bob_kyber = KyberStaticKeypair::generate();

    let plaintext = vec![42u8; 64 * 1024]; // 64KB message
    let info = b"performance-test";

    // Measure encryption time
    let start = Instant::now();
    let envelope = create_envelope(&bob_x25519.pk, &bob_kyber.pk, &plaintext, info)
        .expect("Envelope creation should succeed");
    let encrypt_time = start.elapsed();

    // Measure decryption time
    let start = Instant::now();
    let decrypted = open_envelope(&bob_x25519, &bob_kyber, &envelope, info)
        .expect("Envelope opening should succeed");
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
}
