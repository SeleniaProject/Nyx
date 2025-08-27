//! ML-KEM (Module-Lattice-Based Key-Encapsulation Mechanism) wrapper.
//! Secure implementation using RustCrypto's ml-kem crate to replace vulnerable pqc_kyber.
//! This provides NIST-standardized post-quantum cryptography (FIPS 203).
#![forbid(unsafe_code)]

use crate::{Error, Result};
use rand::{CryptoRng, RngCore};

#[cfg(feature = "kyber")]
use ml_kem::{Keypair, kem::{Encapsulate, Decapsulate}, MlKem768};

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
    use ml_kem::kem::KeyGen;
    
    // Create deterministic RNG from seed for secure key generation
    use rand_chacha::ChaCha20Rng;
    use rand::SeedableRng;
    let mut rng = ChaCha20Rng::from_seed(seed);
    
    let keypair = MlKem768::keygen(&mut rng);
    let secret_key_bytes = keypair.private_key().as_bytes();
    let public_key_bytes = keypair.public_key().as_bytes();
    
    let mut secret = [0u8; sizes::SECRET_KEY];
    let mut public = [0u8; sizes::PUBLIC_KEY];
    secret.copy_from_slice(secret_key_bytes);
    public.copy_from_slice(public_key_bytes);
    
    Ok((secret, public))
}

/// Generate a random ML-KEM-768 keypair using the provided RNG.
pub fn keypair<R: CryptoRng + RngCore>(rng: &mut R) -> Result<(SecretKey, PublicKey)> {
    use ml_kem::kem::KeyGen;
    
    let keypair = MlKem768::keygen(rng);
    let secret_key_bytes = keypair.private_key().as_bytes();
    let public_key_bytes = keypair.public_key().as_bytes();
    
    let mut secret = [0u8; sizes::SECRET_KEY];
    let mut public = [0u8; sizes::PUBLIC_KEY];
    secret.copy_from_slice(secret_key_bytes);
    public.copy_from_slice(public_key_bytes);
    
    Ok((secret, public))
}

/// Encapsulate to a public key, returning (ciphertext, shared_secret).
/// Uses ML-KEM-768 secure encapsulation mechanism.
pub fn encapsulate<R: CryptoRng + RngCore>(
    pk: &PublicKey,
    rng: &mut R,
) -> Result<(Ciphertext, SharedSecret)> {
    use ml_kem::{PublicKey as MlKemPubKey, kem::Encapsulate};
    
    let public_key = MlKemPubKey::from_bytes(pk)
        .map_err(|e| Error::Protocol(format!("Invalid ML-KEM public key: {e:?}")))?;
    
    let (shared_secret, ciphertext) = public_key.encapsulate(rng)
        .map_err(|e| Error::Protocol(format!("ML-KEM encapsulation failed: {e:?}")))?;
    
    let mut ct_bytes = [0u8; sizes::CIPHERTEXT];
    let mut ss_bytes = [0u8; sizes::SHARED_SECRET];
    ct_bytes.copy_from_slice(ciphertext.as_bytes());
    ss_bytes.copy_from_slice(shared_secret.as_bytes());
    
    Ok((ct_bytes, ss_bytes))
}

/// Decapsulate a ciphertext with a secret key to recover the shared secret.
/// Uses ML-KEM-768 secure decapsulation mechanism.
pub fn decapsulate(ct: &Ciphertext, sk: &SecretKey) -> Result<SharedSecret> {
    use ml_kem::{PrivateKey as MlKemPrivKey, Ciphertext as MlKemCiphertext, kem::Decapsulate};
    
    let private_key = MlKemPrivKey::from_bytes(sk)
        .map_err(|e| Error::Protocol(format!("Invalid ML-KEM private key: {e:?}")))?;
    
    let ciphertext = MlKemCiphertext::from_bytes(ct)
        .map_err(|e| Error::Protocol(format!("Invalid ML-KEM ciphertext: {e:?}")))?;
    
    let shared_secret = private_key.decapsulate(&ciphertext)
        .map_err(|e| Error::Protocol(format!("ML-KEM decapsulation failed: {e:?}")))?;
    
    let mut ss_bytes = [0u8; sizes::SHARED_SECRET];
    ss_bytes.copy_from_slice(shared_secret.as_bytes());
    
    Ok(ss_bytes)
}
