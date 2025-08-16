#![forbid(unsafe_code)]

use std::time::Duration;

/// Simple RTT estimator with RTO calculation (RFC 6298-inspired)
#[derive(Debug, Clone)]
pub struct RttEstimator {
	srtt: Option<Duration>,
	rttvar: Option<Duration>,
	rto: Duration,
	alpha: f64,
	beta: f64,
	k: f64,
	min_rto: Duration,
	max_rto: Duration,
}

impl RttEstimator {
	pub fn new(initial_rto: Duration) -> Self {
		Self {
			srtt: None,
			rttvar: None,
			rto: initial_rto,
			alpha: 1.0 / 8.0,
			beta: 1.0 / 4.0,
			k: 4.0,
			min_rto: Duration::from_millis(200),
			max_rto: Duration::from_secs(60),
		}
	}

	/// Provide a new RTT sample (skip samples for retransmitted frames per Karn's algorithm)
	pub fn on_ack_sample(&mut self, sample: Duration) {
		if self.srtt.is_none() {
			// First measurement initialization per RFC 6298
			self.srtt = Some(sample);
			let rttvar = sample / 2;
			self.rttvar = Some(rttvar);
			self.rto = self.clamp(sample + self.mul_k(rttvar));
			return;
		}
		let srtt = self.srtt.unwrap();
		let rttvar = self.rttvar.unwrap_or(sample / 2);
		let err = if srtt > sample { srtt - sample } else { sample - srtt };
		// RTTVAR = (1 - beta) * RTTVAR + beta * |SRTT - sample|
		let new_rttvar = self.mix_dur(rttvar, err, self.beta);
		// SRTT = (1 - alpha) * SRTT + alpha * sample
		let new_srtt = self.mix_dur(srtt, sample, self.alpha);
		self.srtt = Some(new_srtt);
		self.rttvar = Some(new_rttvar);
		self.rto = self.clamp(new_srtt + self.mul_k(new_rttvar));
	}

	/// Exponential backoff on timeout/retransmit
	pub fn on_timeout(&mut self) {
		self.rto = self.clamp(self.rto.saturating_mul(2));
	}

	pub fn rto(&self) -> Duration { self.rto }

	fn mix_dur(&self, a: Duration, b: Duration, w: f64) -> Duration {
		// (1-w)*a + w*b in Duration domain
		let a_ns = a.as_nanos() as f64;
		let b_ns = b.as_nanos() as f64;
		let res = (1.0 - w) * a_ns + w * b_ns;
		Duration::from_nanos(res.max(0.0) as u64)
	}

	fn mul_k(&self, d: Duration) -> Duration {
		let ns = d.as_nanos() as f64 * self.k;
		Duration::from_nanos(ns.max(0.0) as u64)
	}

	fn clamp(&self, d: Duration) -> Duration {
		d.clamp(self.min_rto, self.max_rto)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn initializes_and_updates() {
		let mut est = RttEstimator::new(Duration::from_millis(500));
		assert_eq!(est.rto(), Duration::from_millis(500));
		est.on_ack_sample(Duration::from_millis(100));
		assert!(est.rto() >= Duration::from_millis(100));
		// Subsequent sample closer should reduce RTO towards SRTT
		let rto1 = est.rto();
		est.on_ack_sample(Duration::from_millis(110));
		let rto2 = est.rto();
		assert!(rto2 <= rto1);
	}

	#[test]
	fn backoff_on_timeout() {
		let mut est = RttEstimator::new(Duration::from_millis(300));
		est.on_timeout();
		assert_eq!(est.rto(), Duration::from_millis(600));
		est.on_timeout();
		assert_eq!(est.rto(), Duration::from_millis(1200).clamp(Duration::from_millis(200), Duration::from_secs(60)));
	}
}

