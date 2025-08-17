#![forbid(unsafe_code)]
//! Nyx protocol conformance helpers.
//!
//! This crate provides small, self-contained helpers used by conformance and
//! property tests, such as a deterministic network simulator and generic
//! property-testing utilities. These utilities intentionally avoid any external
//! side-effects and C/C++ dependencies.
//!
//! # Quick Start
//!
//! ```
//! use nyx_conformance::{NetworkSimulator, SimConfig, check_non_decreasing_eps};
//!
//! // Deterministic single-path network simulation
//! let cfg = SimConfig { loss: 0.01, latency_ms: 40, jitter_ms: 8, reorder: 0.1,
//!     bandwidth_pps: 500, max_queue: 128,
//!     ge_good_to_bad: 0.0, ge_bad_to_good: 0.0, ge_loss_good: 0.0, ge_loss_bad: 0.0,
//!     duplicate: 0.0, corruption: 0.0 };
//! let mut sim = NetworkSimulator::new(cfg, 2024);
//! let events = sim.send_burst(32);
//! assert!(events.len() <= 32);
//!
//! // Property: strictly increasing timestamps after sorting
//! let times: Vec<f64> = events.iter().map(|e| e.delivery_ms as f64).collect();
//! // Allow equal millisecond timestamps due to finite resolution
//! check_non_decreasing_eps(&times, 0.0).unwrap();
//! ```

pub mod network_simulator;
pub mod property_tester;

pub use network_simulator::{DeliveryEvent, NetworkSimulator, SimConfig, MultiPathSimulator};
pub use property_tester::{check_monotonic_increasing, check_non_decreasing_eps, MonotonicError};

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn lib_reexports() {
		// Smoke-check that re-exports are wired and types are usable.
	let _cfg = SimConfig::default();
		let _ = MonotonicError::NotIncreasing { idx: 0, prev: 0.0, next: 0.0 };
	}
}


