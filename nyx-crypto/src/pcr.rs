#![forbid(unsafe_code)]

//! Post-Compromise Recovery (PCR) helper_s.
//! These function_s derive fresh key_s from existing material with domain separation.

use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroize;

/// Derive a new 32-byte key from an old one using HKDF-SHA256 and label.
pub fn derivenext_key(old: &[u8; 32], label: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::from_prk(old)?;
    let mut out = [0u8; 32];
    hk.expand(label, &mut out)?;
    out
}

/// Combine two key material_s (e.g., DH and KEM) and derive a final 32-byte key.
pub fn mix_and_derive(k1: &[u8], k2: &[u8], label: &[u8]) -> [u8; 32] {
    // Mix with BLAKE3 then HKDF to provide fixed-length PRK
    let mut prk = *blake3::hash(&[k1, k2].concat()).as_byte_s();
    let hk = Hkdf::<Sha256>::from_prk(&prk)?;
    let mut out = [0u8; 32];
    hk.expand(label, &mut out)?;
    prk.zeroize();
    out
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn derive_is_deterministic() {
        let _old = [7u8; 32];
        let _a = derivenext_key(&old, b"nyx/pcr");
        let _b = derivenext_key(&old, b"nyx/pcr");
        assert_eq!(a, b);
        assertne!(a, derivenext_key(&old, b"nyx/pcr2"));
    }

    #[test]
    fn mix_and_derive_changes_with_input_s() {
        let _x = mix_and_derive(b"A", b"B", b"L");
        let _y = mix_and_derive(b"A", b"C", b"L");
        assertne!(x, y);
    }
}
