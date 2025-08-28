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

/// Simple password-based keystore implementation with enhanced security
struct Keystore;

impl Keystore {
    /// Check for common weak password patterns
    ///
    /// # Security Considerations
    /// - Detects all-same-character passwords
    /// - Identifies sequential character patterns
    /// - Blocks common weak passwords
    /// - Prevents trivial dictionary attacks
    fn is_weak_password(password: &[u8]) -> bool {
        // Convert to string for pattern analysis (assuming UTF-8)
        let pass_str = match std::str::from_utf8(password) {
            Ok(s) => s.to_lowercase(),
            Err(_) => return false, // Binary passwords are assumed strong
        };

        // Check for all same characters
        if pass_str
            .chars()
            .all(|c| c == pass_str.chars().next().unwrap_or('\0'))
        {
            return true;
        }

        // Check for sequential patterns
        if pass_str == "12345678" || pass_str == "abcdefgh" || pass_str == "87654321" {
            return true;
        }

        // Check for common weak passwords
        const WEAK_PASSWORDS: &[&str] = &[
            "password", "123456", "qwerty", "admin", "root", "user", "test", "changeme", "default",
            "guest", "login", "pass", "secret",
        ];

        WEAK_PASSWORDS.iter().any(|&weak| pass_str.contains(weak))
    }
}

// SECURITY ENHANCEMENT: Increased PBKDF2 iterations to meet current security standards
// OWASP recommendation: minimum 120,000 for PBKDF2-HMAC-SHA256 (2021)
// NIST SP 800-63B: minimum 10,000, but industry best practice recommends 600,000+
const PBKDF2_ITERS: u32 = 600_000; // Enhanced security against rainbow table and brute force attacks
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12; // AES-GCM standard 96-bit

/// Encrypt plaintext with password-based key derivation
///
/// # Security Considerations
/// - Uses PBKDF2-HMAC-SHA256 with 600,000 iterations to resist brute force attacks
/// - Generates cryptographically secure random salt and nonce for each encryption
/// - Employs AES-256-GCM for authenticated encryption
/// - Automatically zeroizes sensitive key material after use
///
/// # Errors
/// Returns `Error::Protocol` if:
/// - Random number generation fails
/// - Encryption operation fails
/// - Memory allocation fails
///
/// # Examples
/// ```no_run
/// # use nyx_crypto::keystore::encrypt_with_password;
/// let password = b"secure_password_123";
/// let plaintext = b"secret_data";
/// let encrypted = encrypt_with_password(password, plaintext)?;
/// # Ok::<(), nyx_crypto::Error>(())
/// ```
pub fn encrypt_with_password(password: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    // SECURITY ENHANCEMENT: Comprehensive input validation to prevent DoS attacks
    if password.is_empty() {
        return Err(Error::Protocol(
            "SECURITY: password cannot be empty (authentication bypass prevention)".into(),
        ));
    }
    if password.len() < 8 {
        return Err(Error::Protocol(
            "SECURITY: password too short, minimum 8 bytes required (brute force prevention)"
                .into(),
        ));
    }
    if password.len() > 1024 {
        return Err(Error::Protocol(
            "SECURITY: password too long, maximum 1024 bytes allowed (DoS prevention)".into(),
        ));
    }
    if plaintext.len() > 10 * 1024 * 1024 {
        return Err(Error::Protocol(
            "SECURITY: plaintext too large, maximum 10MB allowed (memory exhaustion prevention)"
                .into(),
        ));
    }

    // SECURITY ENHANCEMENT: Check for weak passwords (basic patterns)
    if Keystore::is_weak_password(password) {
        return Err(Error::Protocol(
            "SECURITY: weak password detected, please use a stronger password".into(),
        ));
    }

    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];

    // SECURITY ENHANCEMENT: Use secure random number generation with explicit error handling
    getrandom(&mut salt)
        .map_err(|e| Error::Protocol(format!("SECURITY: secure random generation failed: {e}")))?;
    getrandom(&mut nonce)
        .map_err(|e| Error::Protocol(format!("SECURITY: secure random generation failed: {e}")))?;

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

/// Decrypt password-encrypted data
///
/// # Security Considerations
/// - Validates minimum blob size to prevent buffer underflow attacks
/// - Uses constant-time operations where possible
/// - Automatically zeroizes sensitive key material after use
/// - Resistant to padding oracle attacks due to AES-GCM authentication
///
/// # Errors
/// Returns `Error::Protocol` if:
/// - Blob is too short to contain valid encrypted data
/// - Salt or nonce extraction fails
/// - Decryption or authentication fails
/// - Invalid blob format detected
///
/// # Examples
/// ```no_run
/// # use nyx_crypto::keystore::{encrypt_with_password, decrypt_with_password};
/// let password = b"secure_password_123";
/// let plaintext = b"secret_data";
/// let encrypted = encrypt_with_password(password, plaintext)?;
/// let decrypted = decrypt_with_password(password, &encrypted)?;
/// assert_eq!(decrypted, plaintext);
/// # Ok::<(), nyx_crypto::Error>(())
/// ```
pub fn decrypt_with_password(password: &[u8], blob: &[u8]) -> Result<Vec<u8>> {
    // SECURITY ENHANCEMENT: Comprehensive input validation
    if password.is_empty() {
        return Err(Error::Protocol("password cannot be empty".into()));
    }
    if password.len() > 1024 {
        return Err(Error::Protocol("password too long (max 1024 bytes)".into()));
    }
    if blob.len() > 10 * 1024 * 1024 + SALT_LEN + NONCE_LEN + 16 {
        return Err(Error::Protocol("encrypted blob too large".into()));
    }
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
    use std::io::Write;

    #[allow(dead_code)]
    pub fn save(path: &str, password: &[u8], plaintext: &[u8]) -> Result<()> {
        let blob = encrypt_with_password(password, plaintext)?;
        let mut f = std::fs::File::create(path)
            .map_err(|e| Error::Protocol(format!("keystore create: {e}")))?;
        f.write_all(&blob)
            .map_err(|e| Error::Protocol(format!("keystore write: {e}")))?;
        Ok(())
    }

    #[allow(dead_code)]
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
        let pw = b"StrongP@ssw0rd123!";
        let data = b"top-secret";
        let blob = encrypt_with_password(pw, data)?;
        assert!(blob.len() > SALT_LEN + NONCE_LEN);
        let out = decrypt_with_password(pw, &blob)?;
        assert_eq!(out, data);
        Ok(())
    }

    #[test]
    fn wrong_password_fail_s() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let blob = encrypt_with_password(b"StrongP@ssw0rd123!", b"data")?;
        assert!(decrypt_with_password(b"DifferentStr0ngP@ss!", &blob).is_err());
        Ok(())
    }
}
