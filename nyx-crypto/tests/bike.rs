//! BIKE KEM Integration Tests
//!
//! This test suite will be activated once BIKE KEM implementation is available.
//! Currently tests verify that the API returns appropriate NotImplemented errors.

#![cfg(feature = "bike")]

use nyx_crypto::bike::{decapsulate, encapsulate, keygen};
use nyx_crypto::Error;
use rand::thread_rng;

#[test]
fn test_bike_keygen_not_implemented() {
    let mut rng = thread_rng();
    let result = keygen(&mut rng);

    assert!(matches!(result, Err(Error::NotImplemented(_))));
    if let Err(Error::NotImplemented(msg)) = result {
        assert!(msg.contains("BIKE KEM"));
        assert!(msg.contains("ML-KEM-768"));
    }
}

#[test]
fn test_bike_encapsulate_not_implemented() {
    use nyx_crypto::bike::{sizes, PublicKey};

    let mut rng = thread_rng();
    let dummy_pk = PublicKey::from_bytes([0u8; sizes::PUBLIC_KEY]);

    let result = encapsulate(&dummy_pk, &mut rng);

    assert!(matches!(result, Err(Error::NotImplemented(_))));
    if let Err(Error::NotImplemented(msg)) = result {
        assert!(msg.contains("encapsulation"));
    }
}

#[test]
fn test_bike_decapsulate_not_implemented() {
    use nyx_crypto::bike::{sizes, Ciphertext, SecretKey};

    let dummy_sk = SecretKey::from_bytes([0u8; sizes::SECRET_KEY]);
    let dummy_ct = Ciphertext::from_bytes([0u8; sizes::CIPHERTEXT]);

    let result = decapsulate(&dummy_sk, &dummy_ct);

    assert!(matches!(result, Err(Error::NotImplemented(_))));
    if let Err(Error::NotImplemented(msg)) = result {
        assert!(msg.contains("decapsulation"));
    }
}

#[test]
fn test_bike_public_key_operations() {
    use nyx_crypto::bike::{sizes, PublicKey};

    let bytes = [42u8; sizes::PUBLIC_KEY];
    let pk = PublicKey::from_bytes(bytes);

    // Test byte conversions
    assert_eq!(pk.as_bytes(), &bytes);
    assert_eq!(pk.to_bytes(), bytes);

    // Test clone
    let pk2 = pk.clone();
    assert_eq!(pk, pk2);

    // Test debug formatting (should not expose key material)
    let debug_str = format!("{:?}", pk);
    assert!(debug_str.contains("BikePublicKey"));
    assert!(debug_str.contains("size"));
}

#[test]
fn test_bike_secret_key_operations() {
    use nyx_crypto::bike::{sizes, SecretKey};

    let bytes = [42u8; sizes::SECRET_KEY];
    let sk = SecretKey::from_bytes(bytes);

    // Test byte access
    assert_eq!(sk.as_bytes(), &bytes);

    // Test debug formatting (should redact key material)
    let debug_str = format!("{:?}", sk);
    assert!(debug_str.contains("BikeSecretKey"));
    assert!(debug_str.contains("REDACTED"));
    assert!(!debug_str.contains("42")); // Should not expose actual bytes
}

#[test]
fn test_bike_ciphertext_operations() {
    use nyx_crypto::bike::{sizes, Ciphertext};

    let bytes = [123u8; sizes::CIPHERTEXT];
    let ct = Ciphertext::from_bytes(bytes);

    // Test byte conversions
    assert_eq!(ct.as_bytes(), &bytes);
    assert_eq!(ct.to_bytes(), bytes);

    // Test clone
    let ct2 = ct.clone();
    assert_eq!(ct.as_bytes(), ct2.as_bytes());

    // Test debug formatting
    let debug_str = format!("{:?}", ct);
    assert!(debug_str.contains("BikeCiphertext"));
}

// Future tests to be implemented when BIKE becomes available:

/// Test BIKE key generation produces valid keys
///
/// When implementing, verify:
/// - Keys are different for each call (randomness)
/// - Keys have correct sizes
/// - Public key can be serialized/deserialized
#[ignore]
#[test]
fn test_bike_keygen_roundtrip() {
    // TODO: Implement when BIKE is available
}

/// Test BIKE encapsulation/decapsulation roundtrip
///
/// When implementing, verify:
/// - Encapsulation produces different ciphertexts for same pk (randomness)
/// - Decapsulation recovers the same shared secret
/// - Shared secrets match between sender and receiver
#[ignore]
#[test]
fn test_bike_encap_decap_roundtrip() {
    // TODO: Implement when BIKE is available
}

/// Test BIKE with invalid inputs
///
/// When implementing, verify:
/// - Invalid public key size is rejected
/// - Invalid ciphertext size is rejected
/// - Corrupted ciphertext produces error (not panic)
#[ignore]
#[test]
fn test_bike_invalid_inputs() {
    // TODO: Implement when BIKE is available
}

/// Test BIKE constant-time properties
///
/// When implementing, verify:
/// - Decapsulation time does not depend on ciphertext validity
/// - No timing side-channels leak secret key information
#[ignore]
#[test]
fn test_bike_timing_side_channels() {
    // TODO: Implement when BIKE is available
    // Consider using criterion for timing measurements
}

/// Test BIKE key zeroization
///
/// When implementing, verify:
/// - Secret keys are zeroized on drop
/// - Intermediate values are zeroized
/// - Shared secrets are zeroized on drop
#[ignore]
#[test]
fn test_bike_zeroization() {
    // TODO: Implement when BIKE is available
}
