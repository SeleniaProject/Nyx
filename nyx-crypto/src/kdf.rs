#![forbid(unsafe_code)]

use hkdf::Hkdf;
use sha2::Sha256;

/// Thin wrapper around HKDF-SHA256 extract+expand.
pub fn hkdf_expand(prk: &[u8], info: &[u8], out: &mut [u8]) {
	// 既に prk として扱う: salt は指定済み想定
	let hk = Hkdf::<Sha256>::from_prk(prk).expect("prk length");
	hk.expand(info, out).expect("hkdf expand");
}

/// Build a 96-bit nonce from base nonce and counter (RFC8439-style XOR of last 8 bytes).
pub fn aead_nonce_xor(base: &[u8; 12], seq: u64) -> [u8; 12] {
	let mut n = [0u8; 12];
	n.copy_from_slice(base);
	// 最後の 8 バイトと XOR
	let ctr = seq.to_be_bytes();
	for i in 0..8 { n[4 + i] ^= ctr[i]; }
	n
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn nonce_xor_changes_tail() {
		let base = [0u8;12];
		let n0 = aead_nonce_xor(&base, 0);
		let n1 = aead_nonce_xor(&base, 1);
		assert_ne!(n0, n1);
		assert_eq!(&n0[..4], &n1[..4]);
	}
}

