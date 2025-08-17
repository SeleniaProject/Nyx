//! VDF stub (not cryptographically secure)

use sha2::{Digest, Sha256};

/// 繰り返し回数 `iters` だけ SHA-256 を連鎖させる簡易VDF。
pub fn eval(seed: &[u8], iters: u32) -> [u8; 32] {
	let mut h = Sha256::new();
	h.update(seed);
	let mut out: [u8; 32] = h.finalize_reset().into();
	for _ in 0..iters {
		h.update(&out);
		out = h.finalize_reset().into();
	}
	out
}

#[cfg(test)]
mod tests { use super::*; #[test] fn different_iters_change_output() { let a = eval(b"x", 1); let b = eval(b"x", 2); assert_ne!(a, b); } }
