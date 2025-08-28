//! ML-KEM (Module-Lattice-Based Key-Encapsulation Mechanism) wrapper.
//! Secure implementation using RustCrypto's ml-kem crate (FIPS 203, formerly Kyber).
//! This replaces the legacy pqc_kyber and is enabled behind the `kyber` feature.
#![forbid(unsafe_code)]

use crate::{Error, Result};
use rand::{CryptoRng, RngCore};
#[cfg(feature = "kyber")]
use {
    ml_kem::{
        array::Array, kem::Decapsulate, kem::DecapsulationKey, kem::Encapsulate,
        kem::EncapsulationKey, EncodedSizeUser, KemCore, MlKem768, MlKem768Params,
    },
    rand_chacha::ChaCha20Rng,
    rand_core_06::SeedableRng as SeedableRng06,
};

/// Sizes for ML-KEM-768 (equivalent to Kyber-768).
#[allow(dead_code)]
pub mod sizes {
    pub const CIPHERTEXT: usize = 1088; // ML-KEM-768 ciphertext size
    pub const PUBLIC_KEY: usize = 1184; // ML-KEM-768 public key size
    pub const SECRET_KEY: usize = 2400; // ML-KEM-768 secret key size
    pub const SHARED_SECRET: usize = 32; // ML-KEM shared secret size
}

/// Public key bytes for ML-KEM-768.
pub type PublicKey = [u8; sizes::PUBLIC_KEY];
/// Secret key bytes for ML-KEM-768.
pub type SecretKey = [u8; sizes::SECRET_KEY];
/// Ciphertext bytes for ML-KEM-768 encapsulation.
pub type Ciphertext = [u8; sizes::CIPHERTEXT];
/// Shared secret bytes.
pub type SharedSecret = [u8; sizes::SHARED_SECRET];

/// Deterministically derive a keypair from a 32-byte seed.
/// Note: ML-KEM uses secure deterministic key generation.
pub fn derive(seed: [u8; 32]) -> Result<(SecretKey, PublicKey)> {
    #[cfg(feature = "kyber")]
    {
        // Use a deterministic RNG derived from the provided seed
        let mut rng = ChaCha20Rng::from_seed(seed);
        let (dk, ek) = MlKem768::generate(&mut rng);

        let ek_bytes = ek.as_bytes();
        let dk_bytes = dk.as_bytes();

        let mut sk = [0u8; sizes::SECRET_KEY];
        sk.copy_from_slice(dk_bytes.as_ref());
        let mut pk = [0u8; sizes::PUBLIC_KEY];
        pk.copy_from_slice(ek_bytes.as_ref());
        Ok((sk, pk))
    }
    #[cfg(not(feature = "kyber"))]
    {
        Err(Error::NotImplemented(
            "ML-KEM feature is disabled".to_string(),
        ))
    }
}

/// Generate a random ML-KEM-768 keypair using the provided RNG.
pub fn keypair<R: CryptoRng + RngCore>(rng: &mut R) -> Result<(SecretKey, PublicKey)> {
    #[cfg(feature = "kyber")]
    {
        let (dk, ek) = MlKem768::generate(rng);
        let ek_bytes = ek.as_bytes();
        let dk_bytes = dk.as_bytes();

        let mut sk = [0u8; sizes::SECRET_KEY];
        sk.copy_from_slice(dk_bytes.as_ref());
        let mut pk = [0u8; sizes::PUBLIC_KEY];
        pk.copy_from_slice(ek_bytes.as_ref());
        Ok((sk, pk))
    }
    #[cfg(not(feature = "kyber"))]
    {
        Err(Error::NotImplemented(
            "ML-KEM feature is disabled".to_string(),
        ))
    }
}

/// Encapsulate to a public key, returning (ciphertext, shared_secret).
/// Uses ML-KEM-768 secure encapsulation mechanism.
pub fn encapsulate<R: CryptoRng + RngCore>(
    pk: &PublicKey,
    rng: &mut R,
) -> Result<(Ciphertext, SharedSecret)> {
    #[cfg(feature = "kyber")]
    {
        // Recreate encapsulation key from encoded bytes
        let pk_arr: Array<u8, <EncapsulationKey<MlKem768Params> as EncodedSizeUser>::EncodedSize> =
            Array::from(*pk);
        let ek = EncapsulationKey::<MlKem768Params>::from_bytes(&pk_arr);

        let (ct, ss) = ek
            .encapsulate(rng)
            .map_err(|_| Error::Crypto("ML-KEM encapsulation failed".to_string()))?;

        let mut ct_out = [0u8; sizes::CIPHERTEXT];
        ct_out.copy_from_slice(ct.as_ref());

        let mut ss_out = [0u8; sizes::SHARED_SECRET];
        ss_out.copy_from_slice(ss.as_ref());

        Ok((ct_out, ss_out))
    }
    #[cfg(not(feature = "kyber"))]
    {
        Err(Error::NotImplemented(
            "ML-KEM feature is disabled".to_string(),
        ))
    }
}

/// Decapsulate a ciphertext with a secret key to recover the shared secret.
/// Uses ML-KEM-768 secure decapsulation mechanism.
pub fn decapsulate(ct: &Ciphertext, sk: &SecretKey) -> Result<SharedSecret> {
    #[cfg(feature = "kyber")]
    {
        // Recreate decapsulation key and ciphertext from encoded bytes
        let sk_arr: Array<u8, <DecapsulationKey<MlKem768Params> as EncodedSizeUser>::EncodedSize> =
            Array::from(*sk);
        let dk = DecapsulationKey::<MlKem768Params>::from_bytes(&sk_arr);

        let ct_arr: Array<u8, <MlKem768 as KemCore>::CiphertextSize> = Array::from(*ct);

        let ss = dk
            .decapsulate(&ct_arr)
            .map_err(|_| Error::Crypto("ML-KEM decapsulation failed".to_string()))?;

        let mut ss_out = [0u8; sizes::SHARED_SECRET];
        ss_out.copy_from_slice(ss.as_ref());
        Ok(ss_out)
    }
    #[cfg(not(feature = "kyber"))]
    {
        Err(Error::NotImplemented(
            "ML-KEM feature is disabled".to_string(),
        ))
    }
}
