use ed25519_dalek::{Signer, SigningKey, VerifyingKey, Signature};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registration {
	pub node_id: String,
	pub public_addr: String,
	pub private_addr: String,
	pub timestamp: i64,
}

#[derive(thiserror::Error, Debug)]
pub enum RvError {
	#[error("sign: {0}")] Sign(String),
	#[error("verify failed")] Verify,
}

pub type Result<T> = std::result::Result<T, RvError>;

/// Sign registration payload using Ed25519.
pub fn sign_registration(sk: &SigningKey, reg: &Registration) -> Result<Vec<u8>> {
	let msg = serde_json::to_vec(reg).map_err(|e| RvError::Sign(e.to_string()))?;
	let sig: Signature = sk.sign(&msg);
	let mut out = Vec::with_capacity(32 + 64 + msg.len());
	out.extend_from_slice(sk.verifying_key().as_bytes());
	out.extend_from_slice(&sig.to_bytes());
	out.extend_from_slice(&msg);
	Ok(out)
}

/// Verify signed registration, returning payload if valid.
pub fn verify_registration(signed: &[u8]) -> Result<Registration> {
	if signed.len() < 96 { return Err(RvError::Verify); }
	let mut pk_bytes = [0u8; 32];
	pk_bytes.copy_from_slice(&signed[..32]);
	let pk = VerifyingKey::from_bytes(&pk_bytes).map_err(|_| RvError::Verify)?;
	let mut sig_bytes = [0u8; 64];
	sig_bytes.copy_from_slice(&signed[32..96]);
	let sig = Signature::from_bytes(&sig_bytes);
	let msg = &signed[96..];
	pk.verify_strict(msg, &sig).map_err(|_| RvError::Verify)?;
	serde_json::from_slice(msg).map_err(|_| RvError::Verify)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rand::rngs::OsRng;

	#[test]
	fn sign_and_verify() {
		let sk = SigningKey::generate(&mut OsRng);
		let reg = Registration { node_id: "n1".into(), public_addr: "1.2.3.4:5".into(), private_addr: "10.0.0.1:5".into(), timestamp: 12345 };
		let s = sign_registration(&sk, &reg).unwrap();
		let out = verify_registration(&s).unwrap();
		assert_eq!(out, reg);
	}
}
