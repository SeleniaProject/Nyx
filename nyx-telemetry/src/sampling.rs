//! Simple probabilistic sampling helpers.

use rand::Rng;

/// Return true with probability p in [0.0, 1.0].
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
mod tests {
	use super::*;

	#[test]
	fn bounds() {
		assert!(!sample(-1.0));
		assert!(!sample(0.0));
		assert!(sample(1.0));
	}
}

