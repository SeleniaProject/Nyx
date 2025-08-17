//! Kyber KEM thin wrapper over `pqc_kyber`.
//! Provides minimal, allocation-friendly helpers for keypair and KEM ops.
#![forbid(unsafe_code)]

use crate::{Error, Result};
// pqc_kyber expects RNGs implementing traits from the `rand` crate (not rand_core)
use rand::{CryptoRng, RngCore};

#[cfg(feature = "kyber")]
use pqc_kyber as kyber_impl;

/// Sizes re-exported for callers that want to preallocate.
#[allow(dead_code)]
pub mod sizes {
    pub use pqc_kyber::{
        KYBER_CIPHERTEXTBYTES as CIPHERTEXT, KYBER_PUBLICKEYBYTES as PUBLIC_KEY,
        KYBER_SECRETKEYBYTES as SECRET_KEY, KYBER_SSBYTES as SHARED_SECRET,
    };
}

/// Public key bytes for Kyber.
pub type PublicKey = [u8; kyber_impl::KYBER_PUBLICKEYBYTES];
/// Secret key bytes for Kyber.
pub type SecretKey = [u8; kyber_impl::KYBER_SECRETKEYBYTES];
/// Ciphertext bytes for Kyber encapsulation.
pub type Ciphertext = [u8; kyber_impl::KYBER_CIPHERTEXTBYTES];
/// Shared secret bytes.
pub type SharedSecret = [u8; kyber_impl::KYBER_SSBYTES];

/// Deterministically derive a keypair from a 32-byte seed.
pub fn derive(seed: [u8; 32]) -> Result<(SecretKey, PublicKey)> {
    let kp =
        kyber_impl::derive(&seed).map_err(|e| Error::Protocol(format!("kyber derive: {e}")))?;
    Ok((kp.secret, kp.public))
}

/// Generate a random Kyber keypair using the provided RNG.
pub fn keypair<R: CryptoRng + RngCore>(rng: &mut R) -> Result<(SecretKey, PublicKey)> {
    let kp =
        kyber_impl::keypair(rng).map_err(|e| Error::Protocol(format!("kyber keypair: {e}")))?;
    Ok((kp.secret, kp.public))
}

/// Encapsulate to a public key, returning (ciphertext, shared_secret).
pub fn encapsulate<R: CryptoRng + RngCore>(
    pk: &PublicKey,
    rng: &mut R,
) -> Result<(Ciphertext, SharedSecret)> {
    let (ct, ss) = kyber_impl::encapsulate(&pk[..], rng)
        .map_err(|e| Error::Protocol(format!("kyber encapsulate: {e}")))?;
    Ok((ct, ss))
}

/// Decapsulate a ciphertext with a secret key to recover the shared secret.
pub fn decapsulate(ct: &Ciphertext, sk: &SecretKey) -> Result<SharedSecret> {
    let ss = kyber_impl::decapsulate(&ct[..], &sk[..])
        .map_err(|e| Error::Protocol(format!("kyber decapsulate: {e}")))?;
    Ok(ss)
}
