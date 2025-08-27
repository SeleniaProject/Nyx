//! Cover traffic generation (Poisson)

use rand::Rng;
use rand_distr::{Distribution, Poisson};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use crate::errors::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverTrafficConfig {
    pub target_bandwidth: u64, // bytes per second
    pub poisson_lambda: f64,
    pub min_packet_size: usize,
    pub max_packet_size: usize,
    pub burst_probability: f64,
}

impl Default for CoverTrafficConfig {
    fn default() -> Self {
        Self {
            target_bandwidth: 100_000, // 100 KB/s
            poisson_lambda: 1.0,
            min_packet_size: 64,
            max_packet_size: 1280,
            burst_probability: 0.1,
        }
    }
}

pub struct CoverTrafficGenerator {
    config: CoverTrafficConfig,
    _last_update: Instant,      // Prefix with underscore to avoid unused warning
    _packets_generated: u64,    // Prefix with underscore to avoid unused warning
}

impl CoverTrafficGenerator {
    pub fn new(config: CoverTrafficConfig) -> Result<Self> {
        Ok(Self {
            config,
            _last_update: Instant::now(),
            _packets_generated: 0,
        })
    }

    pub fn update_target_bandwidth(&mut self, new_bandwidth: u64) -> Result<()> {
        self.config.target_bandwidth = new_bandwidth;
        Ok(())
    }
}

/// Generate dummy packet count per second using Poisson distribution
pub fn poisson_rate(lambda: f32, rng: &mut impl Rng) -> u32 {
    if lambda <= 0.0 {
        return 0;
    }
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
