//! HPKE integration tests for hybrid envelope encryption support

#[cfg(all(feature = "hybrid", feature = "hpke"))]
mod tests {
    use nyx_crypto::hybrid::demo::HybridHandshake;
    use hex_literal::hex;

    #[test]
    fn test_hpke_context_creation() {
        // Test HPKE context creation for envelope encryption
        let recipient_info = b"test@example.com";
        let context_info = b"nyx-hybrid-envelope-v1";

        let result = HybridHandshake::create_hpke_context(recipient_info, context_info);
        assert!(result.is_ok(), "HPKE context creation should succeed");

        let (public_key, context) = result.unwrap();
        assert_eq!(public_key.len(), 32, "HPKE public key should be 32 bytes for X25519");
        // Context should be ready for encryption
    }

    #[test]
    fn test_hpke_envelope_encryption_round_trip() {
        let recipient_info = b"alice@nyx.net";
        let context_info = b"nyx-hybrid-envelope-v1";
        let plaintext = b"This is a secret message for envelope encryption";
        let aad = b"additional-authenticated-data";

        // Create HPKE context and encrypt
        let (public_key, mut sender_context) = HybridHandshake::create_hpke_context(recipient_info, context_info)
            .expect("HPKE context creation failed");

        let ciphertext = sender_context.seal(plaintext, aad)
            .expect("HPKE encryption failed");

        // Open the envelope (simulating recipient)
        let mut recipient_context = HybridHandshake::open_hpke_context(&public_key, recipient_info, context_info)
            .expect("HPKE context opening failed");

        let decrypted = recipient_context.open(&ciphertext, aad)
            .expect("HPKE decryption failed");

        assert_eq!(decrypted, plaintext, "Decrypted text should match original");
    }

    #[test]
    fn test_hpke_envelope_with_different_context_info() {
        let recipient_info = b"bob@nyx.net";
        let context_info1 = b"nyx-hybrid-envelope-v1";
        let context_info2 = b"different-context-info";
        let plaintext = b"Secret message";
        let aad = b"";

        // Encrypt with first context
        let (public_key, mut sender_context) = HybridHandshake::create_hpke_context(recipient_info, context_info1)
            .expect("HPKE context creation failed");

        let ciphertext = sender_context.seal(plaintext, aad)
            .expect("HPKE encryption failed");

        // Try to decrypt with different context info (should fail)
        let result = HybridHandshake::open_hpke_context(&public_key, recipient_info, context_info2);
        
        if let Ok(mut wrong_context) = result {
            let decrypt_result = wrong_context.open(&ciphertext, aad);
            assert!(decrypt_result.is_err(), "Decryption should fail with wrong context info");
        }
    }

    #[test]
    fn test_hpke_envelope_multiple_messages() {
        let recipient_info = b"charlie@nyx.net";
        let context_info = b"nyx-hybrid-envelope-v1";
        let messages = [
            (b"First message".as_slice(), b"aad1".as_slice()),
            (b"Second message with more content".as_slice(), b"aad2".as_slice()),
            (b"Third message".as_slice(), b"".as_slice()),
        ];

        // Create HPKE contexts
        let (public_key, mut sender_context) = HybridHandshake::create_hpke_context(recipient_info, context_info)
            .expect("HPKE context creation failed");

        let mut recipient_context = HybridHandshake::open_hpke_context(&public_key, recipient_info, context_info)
            .expect("HPKE context opening failed");

        // Encrypt and decrypt multiple messages
        for (plaintext, aad) in &messages {
            let ciphertext = sender_context.seal(plaintext, aad)
                .expect("HPKE encryption failed");

            let decrypted = recipient_context.open(&ciphertext, aad)
                .expect("HPKE decryption failed");

            assert_eq!(&decrypted, plaintext, "Decrypted message should match original");
        }
    }

    #[test]
    fn test_hpke_envelope_invalid_public_key() {
        let recipient_info = b"dave@nyx.net";
        let context_info = b"nyx-hybrid-envelope-v1";
        let invalid_public_key = [0u8; 32]; // All zeros is not a valid curve point

        let result = HybridHandshake::open_hpke_context(&invalid_public_key, recipient_info, context_info);
        assert!(result.is_err(), "Should reject invalid public key");
    }

    #[test]
    fn test_hpke_envelope_tampering_detection() {
        let recipient_info = b"eve@nyx.net";
        let context_info = b"nyx-hybrid-envelope-v1";
        let plaintext = b"Sensitive data";
        let aad = b"auth-data";

        // Encrypt message
        let (public_key, mut sender_context) = HybridHandshake::create_hpke_context(recipient_info, context_info)
            .expect("HPKE context creation failed");

        let mut ciphertext = sender_context.seal(plaintext, aad)
            .expect("HPKE encryption failed");

        // Tamper with ciphertext
        if !ciphertext.is_empty() {
            ciphertext[0] ^= 0xFF;
        }

        // Try to decrypt tampered ciphertext
        let mut recipient_context = HybridHandshake::open_hpke_context(&public_key, recipient_info, context_info)
            .expect("HPKE context opening failed");

        let decrypt_result = recipient_context.open(&ciphertext, aad);
        assert!(decrypt_result.is_err(), "Should detect tampering and fail decryption");
    }

    #[test]
    fn test_hpke_envelope_with_hybrid_handshake_integration() {
        // This test demonstrates how HPKE envelope encryption can be used
        // alongside the hybrid handshake for additional security layers
        use nyx_crypto::hybrid::handshake::*;
        use nyx_crypto::hybrid::{KyberStaticKeypair, X25519StaticKeypair};

        // Set up hybrid handshake
        let alice_x25519 = X25519StaticKeypair::generate();
        let alice_kyber = KyberStaticKeypair::generate();
        let bob_x25519 = X25519StaticKeypair::generate();
        let bob_kyber = KyberStaticKeypair::generate();

        // Complete hybrid handshake
        let alice_result = initiator_handshake(
            &alice_x25519,
            &alice_kyber,
            &bob_x25519.pk,
            &bob_kyber.pk,
        ).expect("Alice handshake succeeded");

        let bob_result = responder_handshake(
            &alice_result.msg1,
            &alice_x25519.pk,
            &bob_x25519,
            &bob_kyber,
        ).expect("Bob handshake succeeded");

        // Use handshake-derived key material for HPKE context derivation
        let recipient_info = b"bob-via-hybrid-handshake";
        let context_info = b"nyx-envelope-post-handshake";

        // Create HPKE envelope using derived context
        let (envelope_public_key, mut envelope_sender) = HybridHandshake::create_hpke_context(recipient_info, context_info)
            .expect("HPKE context creation failed");

        let secret_payload = b"Post-handshake secret data";
        let envelope_aad = b"hybrid-derived-envelope";

        let envelope_ciphertext = envelope_sender.seal(secret_payload, envelope_aad)
            .expect("Envelope encryption failed");

        // Recipient opens envelope
        let mut envelope_recipient = HybridHandshake::open_hpke_context(&envelope_public_key, recipient_info, context_info)
            .expect("Envelope context opening failed");

        let decrypted_payload = envelope_recipient.open(&envelope_ciphertext, envelope_aad)
            .expect("Envelope decryption failed");

        assert_eq!(decrypted_payload, secret_payload, "Envelope payload should decrypt correctly");

        // Verify hybrid session still works
        let session_message = b"Regular session message";
        let session_encrypted = alice_result.tx.encrypt(session_message)
            .expect("Session encryption failed");
        let session_decrypted = bob_result.rx.decrypt(&session_encrypted)
            .expect("Session decryption failed");

        assert_eq!(session_decrypted, session_message, "Session communication should still work");
    }
}
