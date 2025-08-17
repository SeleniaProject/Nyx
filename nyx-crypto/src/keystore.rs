#![forbid(unsafe_code)]

//! Simple password-based keystore for small secrets.
//! - Derives a 256-bit key via PBKDF2-HMAC-SHA256
//! - Encrypts with AES-GCM-256 (pure Rust)
//! - Zeroizes key material
//!
//!   This is intended for developer tooling and tests, not HSM-grade storage.

use aes_gcm::{Aes256Gcm, Key, Nonce};
use aes_gcm::aead::{Aead, KeyInit};
use pbkdf2::pbkdf2_hmac;
use sha2::Sha256;
use getrandom::getrandom;
use zeroize::Zeroize;

use crate::{Error, Result};

const PBKDF2_ITERS: u32 = 120_000; // Balanced for tests; tune in product builds
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
	if blob.len() < SALT_LEN + NONCE_LEN + 16 { // at least one tag
		return Err(Error::Protocol("keystore blob too short".into()));
	}
	let salt: [u8; SALT_LEN] = blob[0..SALT_LEN].try_into().unwrap();
	let nonce: [u8; NONCE_LEN] = blob[SALT_LEN..SALT_LEN+NONCE_LEN].try_into().unwrap();
	let ct = &blob[SALT_LEN+NONCE_LEN..];

	let mut key = [0u8; 32];
	pbkdf2_hmac::<Sha256>(password, &salt, PBKDF2_ITERS, &mut key);
	let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
	let pt = cipher
		.decrypt(Nonce::from_slice(&nonce), ct)
		.map_err(|_| Error::Protocol("keystore decrypt failed".into()))?;
	// Zeroize
	let mut salt_z = salt; salt_z.zeroize();
	let mut nonce_z = nonce; nonce_z.zeroize();
	key.zeroize();
	Ok(pt)
}

#[cfg(feature = "runtime")]
mod fsio {
	use super::*;
	use tokio::{fs, io::AsyncWriteExt};

	pub async fn save(path: &str, password: &[u8], plaintext: &[u8]) -> Result<()> {
		let blob = encrypt_with_password(password, plaintext)?;
		let mut f = fs::File::create(path).await.map_err(|e| Error::Protocol(format!("keystore create: {e}")))?;
		f.write_all(&blob).await.map_err(|e| Error::Protocol(format!("keystore write: {e}")))?;
		Ok(())
	}

	pub async fn load(path: &str, password: &[u8]) -> Result<Vec<u8>> {
		let blob = fs::read(path).await.map_err(|e| Error::Protocol(format!("keystore read: {e}")))?;
		decrypt_with_password(password, &blob)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn roundtrip() {
		let pw = b"password";
		let data = b"top-secret";
		let blob = encrypt_with_password(pw, data).unwrap();
		assert!(blob.len() > SALT_LEN + NONCE_LEN);
		let out = decrypt_with_password(pw, &blob).unwrap();
		assert_eq!(out, data);
	}

	#[test]
	fn wrong_password_fails() {
		let blob = encrypt_with_password(b"pw1", b"data").unwrap();
		assert!(decrypt_with_password(b"pw2", &blob).is_err());
	}
}

