//! Cover traffic generation (Poisson)

use rand::Rng;
use rand_distr::{Distribution, Poisson};

/// Generate dummy packet count per second using Poisson distribution
pub fn poisson_rate(lambda: f32, rng: &mut impl Rng) -> u32 {
	if lambda <= 0.0 { return 0; }
	// Poisson expects f64
	let dist = Poisson::new(lambda as f64).unwrap_or_else(|_| Poisson::new(0.0).unwrap());
	dist.sample(rng) as u32
}

/// Enhanced cover traffic with adaptive rate control
pub fn adaptive_cover_rate(base_lambda: f32, load_factor: f32, rng: &mut impl Rng) -> u32 {
	// Adjust rate based on current network load
	let adjusted_lambda = base_lambda * (1.0 - load_factor.min(0.8));
	poisson_rate(adjusted_lambda, rng)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rand::thread_rng;

	#[test]
	fn test_poisson_rate() {
		let mut rng = thread_rng();
		let rate = poisson_rate(2.0, &mut rng);
		assert!(rate < 20); // Reasonable upper bound for lambda=2.0
	}

	#[test]
	fn test_zero_lambda() {
		let mut rng = thread_rng();
		assert_eq!(poisson_rate(0.0, &mut rng), 0);
		assert_eq!(poisson_rate(-1.0, &mut rng), 0);
	}

	#[test]
	fn test_adaptive_cover() {
		let mut rng = thread_rng();
		let high_load = adaptive_cover_rate(5.0, 0.8, &mut rng);
		let low_load = adaptive_cover_rate(5.0, 0.0, &mut rng);
		// High load should generally produce lower rates (though probabilistic)
		assert!(high_load <= 50); // Sanity check
		assert!(low_load <= 50); // Sanity check
	}
}
