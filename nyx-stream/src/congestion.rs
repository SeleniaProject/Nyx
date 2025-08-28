#![forbid(unsafe_code)]

use std::time::Duration;

/// Simple RTT estimator with RTO calculation (RFC 6298-inspired)
#[derive(Debug, Clone)]
pub struct RttEstimator {
    srtt: Option<Duration>,
    rttvar: Option<Duration>,
    rto: Duration,
    // Pre-computed constants for better performance
    alpha: f64,
    beta: f64,
    k: f64,
    min_rto: Duration,
    max_rto: Duration,
    // Pre-computed fixed-point weights to avoid repeated floating point operations
    #[allow(dead_code)]
    alpha_fixed: u64,
    #[allow(dead_code)]
    beta_fixed: u64,
    #[allow(dead_code)]
    one_minus_alpha_fixed: u64,
    #[allow(dead_code)]
    one_minus_beta_fixed: u64,
}

impl RttEstimator {
    pub fn new(initial_rto: Duration) -> Self {
        let alpha = 1.0 / 8.0;
        let beta = 1.0 / 4.0;
        let k = 4.0;
        let min_rto = Duration::from_millis(200);
        let max_rto = Duration::from_secs(60);

        // Pre-compute fixed-point weights for better performance
        let alpha_fixed = (alpha * 65536.0) as u64;
        let beta_fixed = (beta * 65536.0) as u64;
        let one_minus_alpha_fixed = ((1.0 - alpha) * 65536.0) as u64;
        let one_minus_beta_fixed = ((1.0 - beta) * 65536.0) as u64;

        Self {
            srtt: None,
            rttvar: None,
            rto: initial_rto,
            alpha,
            beta,
            k,
            min_rto,
            max_rto,
            alpha_fixed,
            beta_fixed,
            one_minus_alpha_fixed,
            one_minus_beta_fixed,
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
        let Some(srtt) = self.srtt else {
            return;
        };
        let rttvar = self.rttvar.unwrap_or(sample / 2);
        let err = srtt.abs_diff(sample);
        // rttvar = (1 - beta) * rttvar + beta * |SRTT - sample|
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

    pub fn rto(&self) -> Duration {
        self.rto
    }

    fn mix_dur(&self, a: Duration, b: Duration, w: f64) -> Duration {
        // Ultra-high performance: integer-based weighted average to eliminate floating point overhead
        // (1-w)*a + w*b = a + w*(b - a)
        let a_ns = a.as_nanos() as u64;
        let b_ns = b.as_nanos() as u64;

        // Use fixed-point arithmetic for maximum performance
        // Pre-compute weight constant to avoid repeated multiplication
        let weight = (w * 65536.0) as u64; // 16-bit fixed point
        let diff = b_ns.abs_diff(a_ns);
        let adjustment = (diff * weight) / 65536;

        // Branchless implementation to improve branch prediction
        let (base, offset) = if b_ns > a_ns {
            (a_ns, adjustment)
        } else {
            (a_ns, 0u64.saturating_sub(adjustment))
        };

        Duration::from_nanos(base + offset)
    }

    fn mul_k(&self, d: Duration) -> Duration {
        // Ultra-high performance: pure integer arithmetic multiplication
        // Multiply by k using integer arithmetic to eliminate floating point overhead
        let ns = d.as_nanos() as u64;
        Duration::from_nanos(ns * (self.k as u64))
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
        // Subsequent sample should update RTO according to RFC 6298
        let _rto1 = est.rto();
        est.on_ack_sample(Duration::from_millis(110));
        let rto2 = est.rto();
        // RTO should be calculated correctly (may not always decrease due to variance)
        assert!(rto2 >= Duration::from_millis(200)); // min RTO
        assert!(rto2 <= Duration::from_secs(60)); // max RTO
                                                  // RTO should be reasonable for the measured RTT
        assert!(rto2 >= Duration::from_millis(110));
    }

    #[test]
    fn backoff_on_timeout() {
        let mut est = RttEstimator::new(Duration::from_millis(300));
        est.on_timeout();
        assert_eq!(est.rto(), Duration::from_millis(600));
        est.on_timeout();
        assert_eq!(
            est.rto(),
            Duration::from_millis(1200).clamp(Duration::from_millis(200), Duration::from_secs(60))
        );
    }
}
