#![forbid(unsafe_code)]

//! Simple password-based keystore for small secret_s.
//! - Derive_s a 256-bit key via PBKDF2-HMAC-SHA256
//! - Encrypt_s with AES-GCM-256 (pure Rust)
//! - Zeroize_s key material
//!
//!   Thi_s i_s intended for developer tooling and test_s, not HSM-grade storage.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use getrandom::getrandom;
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use zeroize::Zeroize;

use crate::{Error, Result};

const PBKDF2_ITERS: u32 = 120_000; // Balanced for test_s; tune in product build_s
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12; // AES-GCM standard 96-bit

/// Envelope format: [salt(16) | nonce(12) | ciphertext+tag]
pub fn encrypt_with_password(password: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    getrandom(&mut salt).map_err(|e| Error::Protocol(format!("rng: {e}")))?;
    getrandom(&mut nonce).map_err(|e| Error::Protocol(format!("rng: {e}")))?;

    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password, &salt, PBKDF2_ITERS, &mut key);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| Error::Protocol("keystore encrypt failed".into()))?;

    // Assemble envelope
    let mut out = Vec::with_capacity(SALT_LEN + NONCE_LEN + ct.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);

    // Zeroize sensitive
    salt.zeroize();
    nonce.zeroize();
    key.zeroize();
    Ok(out)
}

pub fn decrypt_with_password(password: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < SALT_LEN + NONCE_LEN + 16 {
        // at least one tag
        return Err(Error::Protocol("keystore blob too short".into()));
    }
    let salt: [u8; SALT_LEN] = blob[0..SALT_LEN]
        .try_into()
        .map_err(|_| Error::Crypto("invalid salt length".into()))?;
    let nonce: [u8; NONCE_LEN] = blob[SALT_LEN..SALT_LEN + NONCE_LEN]
        .try_into()
        .map_err(|_| Error::Crypto("invalid nonce length".into()))?;
    let ct = &blob[SALT_LEN + NONCE_LEN..];

    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(password, &salt, PBKDF2_ITERS, &mut key);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let pt = cipher
        .decrypt(Nonce::from_slice(&nonce), ct)
        .map_err(|_| Error::Protocol("keystore decrypt failed".into()))?;
    // Zeroize
    let mut salt_z = salt;
    salt_z.zeroize();
    let mut nonce_z = nonce;
    nonce_z.zeroize();
    key.zeroize();
    Ok(pt)
}

#[cfg(feature = "runtime")]
mod fsio {
    use super::*;
    use std::fs;
    use std::io::Write;

    pub fn save(path: &str, password: &[u8], plaintext: &[u8]) -> Result<()> {
        let blob = encrypt_with_password(password, plaintext)?;
        let mut f = std::fs::File::create(path)
            .map_err(|e| Error::Protocol(format!("keystore create: {e}")))?;
        f.write_all(&blob)
            .map_err(|e| Error::Protocol(format!("keystore write: {e}")))?;
        Ok(())
    }

    pub fn load(path: &str, password: &[u8]) -> Result<Vec<u8>> {
        let blob =
            std::fs::read(path).map_err(|e| Error::Protocol(format!("keystore read: {e}")))?;
        decrypt_with_password(password, &blob)
    }
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn roundtrip() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let pw = b"password";
        let data = b"top-secret";
        let blob = encrypt_with_password(pw, data)?;
        assert!(blob.len() > SALT_LEN + NONCE_LEN);
        let out = decrypt_with_password(pw, &blob)?;
        assert_eq!(out, data);
        Ok(())
    }

    #[test]
    fn wrong_password_fail_s() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let blob = encrypt_with_password(b"pw1", b"data")?;
        assert!(decrypt_with_password(b"pw2", &blob).is_err());
        Ok(())
    }
}
