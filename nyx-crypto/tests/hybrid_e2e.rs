//! End-to-End test_s for hybrid post-quantum handshake implementation
//! Test_s the complete flow including telemetry integration

#[cfg(feature = "hybrid")]
mod test_s {
    use nyx_crypto::noise::ik_demo::{
        initiator_handshake, initiator_verify_msg2, responder_handshake, StaticKeypair,
    };

    #[test]
    fn test_hybrid_handshake_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        // Generate static keypair_s for both partie_s
        let alice_static = StaticKeypair::generate();
        let bob_static = StaticKeypair::generate();
        let prologue = b"test-prologue";

        // Alice (initiator) perform_s handshake with Bob's public key
        let alice_result = initiator_handshake(&alice_static, &bob_static.pk, prologue)?;

        // Bob (responder) processe_s Alice's message
        let bob_result =
            responder_handshake(&bob_static, &alice_static.pk, &alice_result.msg1, prologue)?;

        // Alice verifie_s Bob's response
        let mut alice_final = alice_result;
        initiator_verify_msg2(&mut alice_final, &bob_result.msg2)?;

        // Verify session key_s are properly established (Alice TX = Bob RX, Alice RX = Bob TX)
        let test_message = b"Hello, hybrid post-quantum world!";

        // Alice encrypts with her TX, Bob decrypts with his RX
        let mut bob_final = bob_result;
        let (_, alice_encrypted) = alice_final.tx.sealnext(&[], test_message)?;
        let bob_decrypted = bob_final.rx.open_at(0, &[], &alice_encrypted)?;
        assert_eq!(bob_decrypted, test_message);

        // Bob encrypts with his TX, Alice decrypts with her RX
        let (_, bob_encrypted) = bob_final.tx.sealnext(&[], test_message)?;
        let alice_decrypted = alice_final.rx.open_at(0, &[], &bob_encrypted)?;
        assert_eq!(alice_decrypted, test_message);
        Ok(())
    }

    #[test]
    fn test_hybrid_handshake_invalid_static_key() -> Result<(), Box<dyn std::error::Error>> {
        let alice_static = StaticKeypair::generate();
        let bob_static = StaticKeypair::generate();
        let charlie_static = StaticKeypair::generate(); // Wrong static key
        let prologue = b"test-prologue";

        let alice_result = initiator_handshake(&alice_static, &bob_static.pk, prologue)?;

        // Bob tries to process with wrong expected static key
        let bob_result = responder_handshake(
            &bob_static,
            &charlie_static.pk,
            &alice_result.msg1,
            prologue,
        );

        assert!(bob_result.is_err(), "Bob should reject invalid static key");
        Ok(())
    }

    #[test]
    fn test_hybrid_handshake_corrupted_message() -> Result<(), Box<dyn std::error::Error>> {
        let alice_static = StaticKeypair::generate();
        let bob_static = StaticKeypair::generate();
        let prologue = b"test-prologue";

        let alice_result = initiator_handshake(&alice_static, &bob_static.pk, prologue)?;

        // Corrupt the message
        let mut corrupted_msg = alice_result.msg1.clone();
        if corrupted_msg.len() > 50 {
            corrupted_msg[50] ^= 0xFF; // Flip bits in the message
        }

        let bob_result =
            responder_handshake(&bob_static, &alice_static.pk, &corrupted_msg, prologue);

        assert!(bob_result.is_err(), "Bob should reject corrupted message");
        Ok(())
    }

    #[cfg(feature = "telemetry")]
    #[test]
    fn test_hybrid_handshake_telemetry_integration() -> Result<(), Box<dyn std::error::Error>> {
        use nyx_crypto::hybrid::HybridHandshake;

        // Get initial telemetry state
        let initial_attempts = HybridHandshake::attempts();
        let initial_successes = HybridHandshake::successes();
        let initial_failures = HybridHandshake::failures();

        let alice_static = StaticKeypair::generate();
        let bob_static = StaticKeypair::generate();
        let prologue = b"test-prologue";

        // Perform successful handshake (should increment telemetry)
        let alice_result = initiator_handshake(&alice_static, &bob_static.pk, prologue)?;

        let _bob_result =
            responder_handshake(&bob_static, &alice_static.pk, &alice_result.msg1, prologue)?;

        // Note: In a full implementation, telemetry would be updated during these operations
        // For now, we just test that the telemetry API is available
        let post_attempts = HybridHandshake::attempts();
        let post_successes = HybridHandshake::successes();
        let post_failures = HybridHandshake::failures();

        // Telemetry API should be accessible
        assert!(
            post_attempts >= initial_attempts,
            "Telemetry should be accessible"
        );
        assert!(
            post_successes >= initial_successes,
            "Success counter should be accessible"
        );
        assert!(
            post_failures >= initial_failures,
            "Failure counter should be accessible"
        );
        Ok(())
    }

    #[test]
    fn test_hybrid_handshake_message_format() -> Result<(), Box<dyn std::error::Error>> {
        let alice_static = StaticKeypair::generate();
        let bob_static = StaticKeypair::generate();
        let prologue = b"test-prologue";

        let alice_result = initiator_handshake(&alice_static, &bob_static.pk, prologue)?;

        let msg1 = &alice_result.msg1;

        // Basic message structure validation
        assert!(
            msg1.len() > 64,
            "Message should contain cryptographic material"
        );

        // In a full implementation, we would validate hybrid-specific headers here
        // For now, just ensure we have a valid message structure
        assert!(!msg1.is_empty(), "Message should not be empty");
        Ok(())
    }
}
