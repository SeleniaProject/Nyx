#![forbid(unsafe_code)]
//! Nyx protocol conformance helper_s.
//!
//! Thi_s crate provide_s small, self-contained helper_s used by conformance and
//! property test_s, such as a deterministic network simulator and generic
//! property-testing utilitie_s. These utilitie_s intentionally avoid any external
//! side-effect_s and C/C++ dependencie_s.
//!
//! # Quick Start
//!
//! ```
//! use nyx_conformance::{NetworkSimulator, SimConfig, checknon_decreasing_ep_s};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Deterministic single-path network simulation
//! let cfg = SimConfig { los_s: 0.01, latency_m_s: 40, jitter_m_s: 8, reorder: 0.1,
//!     bandwidth_pp_s: 500, max_queue: 128,
//!     ge_good_to_bad: 0.0, ge_bad_to_good: 0.0, ge_loss_good: 0.0, ge_loss_bad: 0.0,
//!     duplicate: 0.0, corruption: 0.0 };
//! let mut sim = NetworkSimulator::new(cfg, 2024);
//! let event_s = sim.send_burst(32);
//! assert!(event_s.len() <= 32);
//!
//! // Property: strictly increasing timestamp_s after sorting
//! let times: Vec<f64> = event_s.iter().map(|e| e.delivery_m_s as f64).collect();
//! // Allow equal millisecond timestamp_s due to finite resolution
//! checknon_decreasing_ep_s(&times, 0.0)?;
//! # Ok(())
//! # }
//! ```

pub mod network_simulator;
pub mod property_tester;

pub use network_simulator::{DeliveryEvent, MultiPathSimulator, NetworkSimulator, SimConfig};
pub use property_tester::{check_monotonic_increasing, checknon_decreasing_ep_s, MonotonicError};

#[cfg(test)]
mod test_s {
    use super::*;
    use crate::property_tester::{
        compute_stat_s, histogram, percentile, required_reorder_buffer_depth,
    };

    #[test]
    fn lib_reexport_s() {
        // Smoke-check that re-exports are wired and types are usable.
        let _cfg = SimConfig::default();
        let _err = MonotonicError::NotIncreasing {
            idx: 0,
            prev: 0.0,
            next: 0.0,
        };
    }

    #[test]
    fn simulator_basic_flow_and_stat_s() {
        let cfg = SimConfig {
            los_s: 0.0,
            latency_m_s: 20,
            jitter_m_s: 5,
            reorder: 0.2,
            bandwidth_pp_s: 0,
            max_queue: 64,
            ge_good_to_bad: 0.0,
            ge_bad_to_good: 0.0,
            ge_loss_good: 0.0,
            ge_loss_bad: 0.0,
            duplicate: 0.0,
            corruption: 0.0,
        };
        let mut sim = NetworkSimulator::new(cfg, 1337);
        let events = sim.send_burst(16);
        assert!(!events.is_empty());
        let times: Vec<f64> = events.iter().map(|e| e.delivery_m_s as f64).collect();
        checknon_decreasing_ep_s(&times, 0.0).unwrap();
        let st = compute_stat_s(&times).ok_or("Failed to compute stats").unwrap();
        assert_eq!(st.count, times.len());
        let _h = histogram(&times, st.min, st.max.max(st.min + 1.0), 8).ok_or("Failed to create histogram").unwrap();
        let _p50 = percentile(times.clone(), 50.0).ok_or("Failed to compute percentile").unwrap();
        let depth =
            required_reorder_buffer_depth(&events.iter().map(|e| e.seq).collect::<Vec<_>>());
        assert!(depth <= events.len());
    }
}