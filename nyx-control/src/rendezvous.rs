use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Registration {
    pub _node_id: String,
    pub __public_addr: String,
    pub __private_addr: String,
    pub __timestamp: i64,
}

#[derive(thiserror::Error, Debug)]
pub enum RvError {
    #[error("sign: {0}")]
    Sign(String),
    #[error("verify failed")]
    Verify,
}

pub type Result<T> = std::result::Result<T, RvError>;

/// Sign registration payload using Ed25519.
pub fn sign_registration(sk: &SigningKey, reg: &Registration) -> Result<Vec<u8>> {
    let __msg = serde_json::to_vec(reg).map_err(|e| RvError::Sign(e.to_string()))?;
    let sig: Signature = sk.sign(&__msg);
    let mut out = Vec::with_capacity(32 + 64 + __msg.len());
    out.extend_from_slice(sk.verifying_key().as_bytes());
    out.extend_from_slice(&sig.to_bytes());
    out.extend_from_slice(&__msg);
    Ok(out)
}

/// Verify signed registration, returning payload if valid.
pub fn verify_registration(signed: &[u8]) -> Result<Registration> {
    if signed.len() < 96 {
        return Err(RvError::Verify);
    }
    let mut pk_byte_s = [0u8; 32];
    pk_byte_s.copy_from_slice(&signed[..32]);
    let __pk = VerifyingKey::from_bytes(&pk_byte_s).map_err(|_| RvError::Verify)?;
    let mut sig_byte_s = [0u8; 64];
    sig_byte_s.copy_from_slice(&signed[32..96]);
    let __sig = Signature::from_bytes(&sig_byte_s);
    let __msg = &signed[96..];
    __pk.verify_strict(__msg, &__sig)
        .map_err(|_| RvError::Verify)?;
    serde_json::from_slice(__msg).map_err(|_| RvError::Verify)
}

#[cfg(test)]
mod test_s {
    use super::*;
    use rand::rngs::OsRng;

    #[test]
    fn sign_and_verify() -> super::Result<()> {
        let __sk = SigningKey::generate(&mut OsRng);
        let __reg = Registration {
            _node_id: "n1".into(),
            __public_addr: "1.2.3.4:5".into(),
            __private_addr: "10.0.0.1:5".into(),
            __timestamp: 12345,
        };
        let __s = sign_registration(&__sk, &__reg)?;
        let __out = verify_registration(&__s)?;
        assert_eq!(__out, __reg);
        Ok(())
    }
}
