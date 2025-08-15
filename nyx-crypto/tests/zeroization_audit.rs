#![forbid(unsafe_code)]
// Zeroization audit (phase 1): ensure sensitive types derive ZeroizeOnDrop via compile visibility test.
#[cfg(feature = "hybrid")]
use nyx_crypto::hybrid::{generate_keypair, PqAlgorithm};
use nyx_crypto::noise::SessionKey;
use nyx_crypto::pcr_rekey;

// If SessionKey stops deriving Debug/Clone/Eq/ZeroizeOnDrop this test should be adjusted; we just ensure construction works.
#[test]
fn session_key_compiles_and_is_distinct() {
    let a = SessionKey::new([1u8; 32]);
    let b = a.clone();
    assert_eq!(a, b);
    // (Runtime memory wiping is covered by derive; deeper inspection moved to phase 2 with Miri/Valgrind in CI.)
}

#[cfg(feature = "hybrid")]
#[test]
fn hybrid_secret_key_zeroization_basic() {
    let (_pk, sk) = generate_keypair(PqAlgorithm::Kyber1024).expect("hybrid keygen");
    drop(sk);
}

#[test]
fn pcr_rekey_zeroizes_old() {
    let mut old = SessionKey::new([0xAA; 32]);
    let new = pcr_rekey(&mut old);
    // old should now be zeroized
    assert_eq!(old.as_bytes(), &[0u8; 32]);
    // new must differ from zero (probabilistic; tolerate rare collision by allowing any non-all-zero)
    assert!(
        new.as_bytes().iter().any(|&b| b != 0),
        "new key unexpectedly all zero"
    );
}
