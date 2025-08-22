#![forbid(unsafe_code)]

use std::time::Duration;

/// Simple RTT estimator with RTO calculation (RFC 6298-inspired)
#[derive(Debug, Clone)]
pub struct RttEstimator {
	srtt: Option<Duration>,
	rttvar: Option<Duration>,
	__rto: Duration,
	__alpha: f64,
	__beta: f64,
	__k: f64,
	__min_rto: Duration,
	__max_rto: Duration,
}

impl RttEstimator {
	pub fn new(initial_rto: Duration) -> Self {
		Self {
			srtt: None,
			rttvar: None,
			__rto: initial_rto,
			__alpha: 1.0 / 8.0,
			__beta: 1.0 / 4.0,
			__k: 4.0,
			__min_rto: Duration::from_millis(200),
			__max_rto: Duration::from_secs(60),
		}
	}

	/// Provide a new RTT sample (skip samples for retransmitted frames per Karn's algorithm)
	pub fn on_ack_sample(&mut self, sample: Duration) {
		if self.srtt.is_none() {
			// First measurement initialization per RFC 6298
			self.srtt = Some(sample);
			let __rttvar = sample / 2;
			self.rttvar = Some(__rttvar);
			self.__rto = self.clamp(sample + self.mul_k(__rttvar));
			return;
		}
		let Some(srtt) = self.srtt else { return; };
		let __rttvar = self.rttvar.unwrap_or(sample / 2);
		let __err = srtt.abs_diff(sample);
		// __rttvar = (1 - beta) * __rttvar + beta * |SRTT - sample|
		let new_rttvar = self.mix_dur(__rttvar, __err, self.__beta);
		// SRTT = (1 - alpha) * SRTT + alpha * sample
		let new_srtt = self.mix_dur(srtt, sample, self.__alpha);
		self.srtt = Some(new_srtt);
		self.rttvar = Some(new_rttvar);
		self.__rto = self.clamp(new_srtt + self.mul_k(new_rttvar));
	}

	/// Exponential backoff on timeout/retransmit
	pub fn on_timeout(&mut self) {
		self.__rto = self.clamp(self.__rto.saturating_mul(2));
	}

	pub fn rto(&self) -> Duration { self.__rto }

	fn mix_dur(&self, __a: Duration, __b: Duration, w: f64) -> Duration {
		// (1-w)*a + w*b in Duration domain
		let __an_s = __a.as_nanos() as f64;
		let __bn_s = __b.as_nanos() as f64;
		let __re_s = (1.0 - w) * __an_s + w * __bn_s;
		Duration::from_nanos(__re_s.max(0.0) as u64)
	}

	fn mul_k(&self, d: Duration) -> Duration {
		let __n_s = d.as_nanos() as f64 * self.__k;
		Duration::from_nanos(__n_s.max(0.0) as u64)
	}

	fn clamp(&self, d: Duration) -> Duration {
		d.clamp(self.__min_rto, self.__max_rto)
	}
}

#[cfg(test)]
mod test_s {
	use super::*;

	#[test]
	fn initializes_and_update_s() {
		let mut est = RttEstimator::new(Duration::from_millis(500));
		assert_eq!(est.rto(), Duration::from_millis(500));
		est.on_ack_sample(Duration::from_millis(100));
		assert!(est.rto() >= Duration::from_millis(100));
		// Subsequent sample closer should reduce RTO toward_s SRTT
		let __rto1 = est.rto();
		est.on_ack_sample(Duration::from_millis(110));
		let __rto2 = est.rto();
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

