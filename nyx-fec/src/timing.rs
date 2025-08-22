//! Timing utilitie_s for smoothing metric_s like RTT and los_s.

#[derive(Debug, Clone, Copy)]
pub struct Ema {
	alpha: f32,
	value: Option<f32>,
}

impl Ema {
	pub fn new(alpha: f32) -> Self { Self { alpha: alpha.clamp(0.0, 1.0), value: None } }
	pub fn observe(&mut self, sample: f32) -> f32 {
		match self.value {
			None => { self.value = Some(sample); sample }
			Some(v) => { let n = self.alpha * sample + (1.0 - self.alpha) * v; self.value = Some(n); n }
		}
	}
	pub fn get(&self) -> Option<f32> { self.value }
}

#[cfg(test)]
mod tests {
	use super::*;
	
	#[test]
	fn ema_moves_towards_samples() {
		let mut ema = Ema::new(0.5);
		assert!(ema.get().is_none());
		let v1 = ema.observe(10.0);
		assert_eq!(v1, 10.0);
		let v2 = ema.observe(0.0);
		assert!(v2 < v1);
	}

	#[test]
	fn alpha_is_clamped() {
		let mut ema = Ema::new(2.0); // will clamp to 1.0
		let v1 = ema.observe(10.0);
		assert_eq!(v1, 10.0);
		// alpha == 1.0 -> next value equals the new sample
		let v2 = ema.observe(0.0);
		assert_eq!(v2, 0.0);
	}

	#[test]
	fn ema_converges_to_constant_signal() -> Result<(), Box<dyn std::error::Error>> {
		let mut ema = Ema::new(0.2);
		for _ in 0..50 { ema.observe(5.0); }
		let v = ema.get().ok_or("no value observed")?;
		assert!((v - 5.0).abs() < 1e-3);
		Ok(())
	}
}

