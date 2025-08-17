use pasetors::{claims::Claims, claims::ClaimsValidationRules, keys::SymmetricKey, local, token::UntrustedToken, version4::V4};
use rand::RngCore;
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum PushError {
	#[error("crypto: {0}")] Crypto(String),
}

pub type Result<T> = std::result::Result<T, PushError>;

/// Create a new random symmetric key suitable for PASETO v4.local (32 bytes).
pub fn generate_key() -> SymmetricKey<V4> {
	let mut bytes = [0u8; 32];
	rand::thread_rng().fill_bytes(&mut bytes);
	SymmetricKey::<V4>::from(&bytes).expect("valid key length")
}

/// Issues a short-lived opaque token encoding device id and audience.
pub fn issue_token(key: &SymmetricKey<V4>, device_id: &str, audience: &str, ttl_secs: i64) -> Result<String> {
	let mut claims = Claims::new().map_err(|e| PushError::Crypto(e.to_string()))?;
	claims.add_additional("device_id", json!(device_id)).map_err(|e| PushError::Crypto(e.to_string()))?;
	claims.audience(audience).map_err(|e| PushError::Crypto(e.to_string()))?;
	let exp = TimeLike::now_plus_seconds(ttl_secs).to_rfc3339();
	claims.expiration(&exp).map_err(|e| PushError::Crypto(e.to_string()))?;
	local::encrypt(key, &claims, None, None).map_err(|e| PushError::Crypto(e.to_string()))
}

/// Verifies token and returns device_id if valid and audience matches.
pub fn verify_token(key: &SymmetricKey<V4>, token: &str, expected_aud: &str) -> Result<String> {
	let untrusted = UntrustedToken::try_from(token).map_err(|e| PushError::Crypto(e.to_string()))?;
	let rules = ClaimsValidationRules::new();
	let trusted = local::decrypt(key, &untrusted, &rules, None, None).map_err(|e| PushError::Crypto(e.to_string()))?;
	let payload = trusted.payload_claims().ok_or_else(|| PushError::Crypto("no payload".into()))?;
	let aud = payload.get_claim("aud").and_then(|v| v.as_str()).ok_or_else(|| PushError::Crypto("no aud".into()))?;
	if aud != expected_aud { return Err(PushError::Crypto("aud mismatch".into())); }
	let v = payload.get_claim("device_id").ok_or_else(|| PushError::Crypto("no device_id".into()))?;
	Ok(v.as_str().unwrap_or_default().to_string())
}

// Minimal time helper to avoid extra deps; pasetors uses chrono under the hood.
struct TimeLike;
impl TimeLike { fn now_plus_seconds(secs: i64) -> chrono::DateTime<chrono::Utc> { chrono::Utc::now() + chrono::Duration::seconds(secs) } }

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn token_roundtrip() {
		let key = generate_key();
		let t = issue_token(&key, "dev1", "nyx", 60).unwrap();
		let dev = verify_token(&key, &t, "nyx").unwrap();
		assert_eq!(dev, "dev1");
	}
}
