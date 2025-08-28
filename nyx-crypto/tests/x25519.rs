#![cfg(feature = "classic")]
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use x25519_dalek::{PublicKey, StaticSecret};

/// Security-enhanced X25519 key agreement test using cryptographically secure random keys.
/// This test ensures that the key agreement protocol works correctly with properly generated keys,
/// avoiding any hardcoded values that could compromise security in production environments.
#[test]
fn x25519_key_agreement_basic() {
    // Use cryptographically secure deterministic RNG for reproducible test results
    // while avoiding any hardcoded secret values that could be accidentally copied to production
    let mut rng = ChaCha20Rng::from_seed([42u8; 32]); // Test-only deterministic seed

    // Generate secure random private keys for both parties
    let mut alice_secret_bytes = [0u8; 32];
    let mut bob_secret_bytes = [0u8; 32];
    rng.fill_bytes(&mut alice_secret_bytes);
    rng.fill_bytes(&mut bob_secret_bytes);

    // Create private keys from secure random bytes
    let alice_sk = StaticSecret::from(alice_secret_bytes);
    let bob_sk = StaticSecret::from(bob_secret_bytes);

    // Derive public keys from private keys
    let alice_pk = PublicKey::from(&alice_sk);
    let bob_pk = PublicKey::from(&bob_sk);

    // Perform Diffie-Hellman key exchange
    let alice_ss = alice_sk.diffie_hellman(&bob_pk);
    let bob_ss = bob_sk.diffie_hellman(&alice_pk);

    // Verify that both parties derive the same shared secret
    assert_eq!(alice_ss.as_bytes(), bob_ss.as_bytes());

    // Security check: Ensure the shared secret is not all zeros (cryptographic failure indicator)
    assert_ne!(alice_ss.as_bytes(), &[0u8; 32]);
}
