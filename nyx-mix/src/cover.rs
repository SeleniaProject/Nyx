//! Cover traffic generation (Poisson)

use rand::Rng;
use rand_distr::{Distribution, Poisson};

/// 1秒あたりのダミーパケット数をポアソン分布から生成
pub fn poisson_rate(lambda: f32, rng: &mut impl Rng) -> u32 {
	if lambda <= 0.0 { return 0; }
	// Poisson は f64 想定
	let dist = Poisson::new(lambda as f64).unwrap_or_else(|_| Poisson::new(0.0).unwrap());
	dist.sample(rng) as u32
}

#[cfg(test)]
mod tests {
	use super::*; use rand::thread_rng;
	#[test]
	fn zero_lambda_zero_rate() { let mut rng = thread_rng(); assert_eq!(poisson_rate(0.0, &mut rng), 0); }
}
