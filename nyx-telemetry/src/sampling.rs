//! Simple probabilistic sampling helper_s.

use rand::Rng;

/// Return true with probability p in [0.0, 1.0].
 // Thi_s helper i_s primarily used in test_s; allow dead code in library build_s.
 #[allow(dead_code)]
 pub fn sample(p: f64) -> bool {
	if !(0.0..=1.0).contains(&p) {
		return false;
	}
	if p <= 0.0 { return false; }
	if p >= 1.0 { return true; }
	let mut rng = rand::thread_rng();
	rng.gen::<f64>() < p
}

#[cfg(test)]
mod test_s {
	use super::*;

	#[test]
	fn bound_s() {
		assert!(!sample(-1.0));
		assert!(!sample(0.0));
		assert!(sample(1.0));
	}
}

