//! WASM-facing HPKE (RFC9180) bindings.
//!
//! This exposes a minimal, browser-safe subset using X25519-HKDF-SHA256 and
//! ChaCha20-Poly1305 matching `nyx-crypto` defaults. All state is ephemeral
//! and returned/accepted as base64 (URL-safe no-pad) strings for JS ergonomics.

use base64::engine::{general_purpose, Engine};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use hpke::{Deserializable, Serializable};
#[cfg(feature = "hpke")]
use nyx_crypto::hpke as core_hpke;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HpkeKeypair {
    sk: String,
    pk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HpkeSealResult {
    enc: String,
    ct: String,
}

fn b64e(bytes: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn b64d(s: &str) -> Result<Vec<u8>, JsValue> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(s.as_bytes())
        .map_err(|e| JsValue::from_str(&format!("base64 decode error: {}", e)))
}

#[wasm_bindgen]
pub fn hpke_generate_keypair() -> Result<JsValue, JsValue> {
    #[cfg(feature = "hpke")]
    {
        let (sk, pk) = core_hpke::generate_keypair();
        let kp = HpkeKeypair {
            sk: b64e(&<core_hpke::PrivateKey as Serializable>::to_bytes(&sk)),
            pk: b64e(&<core_hpke::PublicKey as Serializable>::to_bytes(&pk)),
        };
        return serde_wasm_bindgen::to_value(&kp).map_err(|e| JsValue::from_str(&e.to_string()));
    }
    #[cfg(not(feature = "hpke"))]
    {
        Err(JsValue::from_str("hpke feature disabled"))
    }
}

#[wasm_bindgen]
pub fn hpke_seal(
    pk_b64: String,
    info_b64: String,
    aad_b64: String,
    pt_b64: String,
) -> Result<JsValue, JsValue> {
    #[cfg(feature = "hpke")]
    {
        let pk_bytes = b64d(&pk_b64)?;
        let info = b64d(&info_b64)?;
        let aad = b64d(&aad_b64)?;
        let pt = b64d(&pt_b64)?;
        let pk = <core_hpke::PublicKey as Deserializable>::from_bytes(&pk_bytes)
            .map_err(|_| JsValue::from_str("invalid public key"))?;
        let (enc, ct) = core_hpke::seal(&pk, &info, &aad, &pt)
            .map_err(|_| JsValue::from_str("hpke seal failed"))?;
        let out = HpkeSealResult {
            enc: b64e(&enc),
            ct: b64e(&ct),
        };
        return serde_wasm_bindgen::to_value(&out).map_err(|e| JsValue::from_str(&e.to_string()));
    }
    #[cfg(not(feature = "hpke"))]
    {
        Err(JsValue::from_str("hpke feature disabled"))
    }
}

#[wasm_bindgen]
pub fn hpke_open(
    sk_b64: String,
    enc_b64: String,
    info_b64: String,
    aad_b64: String,
    ct_b64: String,
) -> Result<JsValue, JsValue> {
    #[cfg(feature = "hpke")]
    {
        let sk_bytes = b64d(&sk_b64)?;
        let enc = b64d(&enc_b64)?;
        let info = b64d(&info_b64)?;
        let aad = b64d(&aad_b64)?;
        let ct = b64d(&ct_b64)?;
        let sk = <core_hpke::PrivateKey as Deserializable>::from_bytes(&sk_bytes)
            .map_err(|_| JsValue::from_str("invalid private key"))?;
        let pt = core_hpke::open(&sk, &enc, &info, &aad, &ct)
            .map_err(|_| JsValue::from_str("hpke open failed"))?;
        return Ok(JsValue::from_str(&b64e(&pt)));
    }
    #[cfg(not(feature = "hpke"))]
    {
        Err(JsValue::from_str("hpke feature disabled"))
    }
}

/// Generate a fresh session key and seal it to the receiver's public key using HPKE.
/// Returns JSON { enc, ct, sk } where all are base64url strings and sk is 32 bytes.
#[wasm_bindgen]
pub fn hpke_generate_and_seal_session(
    pk_b64: String,
    info_b64: String,
    aad_b64: String,
) -> Result<JsValue, JsValue> {
    #[cfg(feature = "hpke")]
    {
        let pk_bytes = b64d(&pk_b64)?;
        let info = b64d(&info_b64)?;
        let aad = b64d(&aad_b64)?;
        let pk = <core_hpke::PublicKey as Deserializable>::from_bytes(&pk_bytes)
            .map_err(|_| JsValue::from_str("invalid public key"))?;
        let (enc, ct, session) = core_hpke::generate_and_seal_session(&pk, &info, &aad)
            .map_err(|_| JsValue::from_str("hpke generate_and_seal_session failed"))?;
        let out = serde_json::json!({
            "enc": b64e(&enc),
            "ct": b64e(&ct),
            "sk": b64e(&session.0),
        });
        return serde_wasm_bindgen::to_value(&out).map_err(|e| JsValue::from_str(&e.to_string()));
    }
    #[cfg(not(feature = "hpke"))]
    {
        Err(JsValue::from_str("hpke feature disabled"))
    }
}

/// Open a sealed session key and return the 32-byte session key as base64url.
#[wasm_bindgen]
pub fn hpke_open_session(
    sk_b64: String,
    enc_b64: String,
    info_b64: String,
    aad_b64: String,
    ct_b64: String,
) -> Result<String, JsValue> {
    #[cfg(feature = "hpke")]
    {
        let sk_bytes = b64d(&sk_b64)?;
        let enc = b64d(&enc_b64)?;
        let info = b64d(&info_b64)?;
        let aad = b64d(&aad_b64)?;
        let ct = b64d(&ct_b64)?;
        let sk = <core_hpke::PrivateKey as Deserializable>::from_bytes(&sk_bytes)
            .map_err(|_| JsValue::from_str("invalid private key"))?;
        let session = core_hpke::open_session(&sk, &enc, &info, &aad, &ct)
            .map_err(|_| JsValue::from_str("hpke open_session failed"))?;
        Ok(b64e(&session.0))
    }
    #[cfg(not(feature = "hpke"))]
    {
        Err(JsValue::from_str("hpke feature disabled"))
    }
}
