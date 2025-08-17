use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Configuration for the network simulator.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SimConfig {
	/// Packet loss probability in [0.0, 1.0].
	pub loss: f64,
	/// Mean one-way latency in milliseconds.
	pub latency_ms: u64,
	/// Jitter range (+/-) in milliseconds applied uniformly.
	pub jitter_ms: u64,
	/// Probability of reordering two consecutive packets in [0.0, 1.0].
	/// Note: reordering is applied locally before the final sort by delivery
	/// time, so it primarily affects the mapping between sequence numbers and
	/// their drawn latencies rather than the final chronological order.
	pub reorder: f64,
	/// Bandwidth in packets per second (pps). 0 = unlimited (no queueing delay).
	pub bandwidth_pps: u64,
	/// Maximum queue size (packets). When full, tail-drop if enqueue would exceed.
	pub max_queue: usize,
	/// Gilbert-Elliott model parameters for burst loss; if disabled, uses `loss` only.
	pub ge_good_to_bad: f64,
	pub ge_bad_to_good: f64,
	pub ge_loss_good: f64,
	pub ge_loss_bad: f64,
	/// Probability of duplicating a packet (creates a second delivery event at +1ms).
	pub duplicate: f64,
	/// Probability of bit-corruption flag (metadata only; consumer can decide drop).
	pub corruption: f64,
}

impl Default for SimConfig {
	fn default() -> Self {
		Self {
			loss: 0.0,
			latency_ms: 30,
			jitter_ms: 5,
			reorder: 0.0,
			bandwidth_pps: 0,
			max_queue: 1024,
			ge_good_to_bad: 0.0,
			ge_bad_to_good: 0.0,
			ge_loss_good: 0.0,
			ge_loss_bad: 0.0,
			duplicate: 0.0,
			corruption: 0.0,
		}
	}
}

/// A scheduled delivery event for a simulated packet.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeliveryEvent {
	/// Monotonic simulated clock time when the packet is delivered.
	pub delivery_ms: u64,
	/// Sequential id assigned at enqueue time.
	pub seq: u64,
	/// Whether the simulator marked the packet as corrupted.
	pub corrupted: bool,
}

/// Deterministic, seedable network simulator producing delivery schedule.
///
/// The simulator does not model bandwidth. It only accounts for loss, base
/// latency, jitter, and simple local reordering of adjacent packets.
pub struct NetworkSimulator {
	cfg: SimConfig,
	rng: StdRng,
	next_seq: u64,
	now_ms: u64,
	ge_bad_state: bool,
	queue_depth: usize,
	last_departure_ms: u64,
}

impl NetworkSimulator {
	/// Create a new simulator with a given seed for reproducibility.
	pub fn new(cfg: SimConfig, seed: u64) -> Self {
		Self {
			cfg,
			rng: StdRng::seed_from_u64(seed),
			next_seq: 0,
			now_ms: 0,
			ge_bad_state: false,
			queue_depth: 0,
			last_departure_ms: 0,
		}
	}

	/// Advance simulated time by delta milliseconds.
	pub fn advance(&mut self, delta_ms: u64) { self.now_ms = self.now_ms.saturating_add(delta_ms); }

	/// Enqueue `n` packets and return a vector of their delivery events
	/// (lossy packets are omitted). Events are sorted by delivery time, with a
	/// stable tie-breaker on sequence id.
	pub fn send_burst(&mut self, n: usize) -> Vec<DeliveryEvent> {
		let mut evs = Vec::with_capacity(n);
		for _ in 0..n {
			let seq = self.alloc_seq();
			// Loss
			if self.packet_lost() { continue; }

			// Base latency plus jitter in [-jitter, +jitter]
			let jitter = if self.cfg.jitter_ms == 0 {
				0i64
			} else {
				let j = self.rng.gen_range(0..=self.cfg.jitter_ms) as i64;
				let sign = if self.rng.gen::<bool>() { 1 } else { -1 };
				sign * j
			};

			// Simple bandwidth/queue model: if bandwidth_pps > 0, schedule departures
			// minimally spaced by 1000/bandwidth_pps ms. Tail-drop if queue would exceed.
			let base_latency = self.cfg.latency_ms;
			let depart_ms = if self.cfg.bandwidth_pps == 0 {
				self.now_ms
			} else {
				let min_gap = (1000 / self.cfg.bandwidth_pps.max(1)) as u64;
				// Enforce FIFO departure schedule with limited queue
				if self.queue_depth >= self.cfg.max_queue { continue; }
				let next_depart = if self.last_departure_ms == 0 { self.now_ms } else { self.last_departure_ms + min_gap };
				self.last_departure_ms = next_depart;
				self.queue_depth += 1;
				next_depart
			};

			let base = depart_ms.saturating_add(base_latency);
			let delivery = if jitter.is_negative() {
				base.saturating_sub(jitter.unsigned_abs())
			} else {
				base.saturating_add(jitter as u64)
			};

			let corrupted = self.rng.gen::<f64>() < self.cfg.corruption;
			evs.push(DeliveryEvent { delivery_ms: delivery, seq, corrupted });

			// Duplicate one extra copy with +1ms delivery when enabled
			if self.cfg.duplicate > 0.0 && self.rng.gen::<f64>() < self.cfg.duplicate {
				evs.push(DeliveryEvent { delivery_ms: delivery.saturating_add(1), seq, corrupted });
			}
		}

		// Local reordering: with probability `reorder`, swap each adjacent pair
		if self.cfg.reorder > 0.0 && evs.len() > 1 {
			for i in (1..evs.len()).step_by(2) {
				if self.rng.gen::<f64>() < self.cfg.reorder {
					evs.swap(i - 1, i);
				}
			}
		}

		// Sort by delivery time, then by sequence number for stability.
		evs.sort_by_key(|e| (e.delivery_ms, e.seq));
		// Drain queued departures considered delivered in this batch window
		if self.cfg.bandwidth_pps > 0 {
			// Decrease queue by the number of unique sequence ids delivered
			let delivered = evs.iter().map(|e| e.seq).collect::<std::collections::BTreeSet<_>>().len();
			self.queue_depth = self.queue_depth.saturating_sub(delivered);
		}
		evs
	}

	fn alloc_seq(&mut self) -> u64 {
		let s = self.next_seq;
		self.next_seq = self.next_seq.wrapping_add(1);
		s
	}
}

impl NetworkSimulator {
	fn packet_lost(&mut self) -> bool {
		// If GE parameters are disabled, fall back to simple Bernoulli loss.
		if self.cfg.ge_good_to_bad == 0.0 && self.cfg.ge_bad_to_good == 0.0 {
			return self.rng.gen::<f64>() < self.cfg.loss;
		}
		// Update state transitions
		if self.ge_bad_state {
			if self.rng.gen::<f64>() < self.cfg.ge_bad_to_good { self.ge_bad_state = false; }
		} else if self.rng.gen::<f64>() < self.cfg.ge_good_to_bad {
			self.ge_bad_state = true;
		}
		let p = if self.ge_bad_state { self.cfg.ge_loss_bad } else { self.cfg.ge_loss_good };
		self.rng.gen::<f64>() < p
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn deterministic_with_seed() {
	let cfg = SimConfig { loss: 0.2, latency_ms: 50, jitter_ms: 10, reorder: 0.5, bandwidth_pps: 1000, max_queue: 64, ge_good_to_bad: 0.0, ge_bad_to_good: 0.0, ge_loss_good: 0.0, ge_loss_bad: 0.0, duplicate: 0.1, corruption: 0.0 };
		let mut a = NetworkSimulator::new(cfg, 42);
		let mut b = NetworkSimulator::new(cfg, 42);
		let ea = a.send_burst(32);
		let eb = b.send_burst(32);
		assert_eq!(ea, eb);
	}

	#[test]
	fn delivery_sorted_and_stable() {
	let cfg = SimConfig { loss: 0.0, latency_ms: 10, jitter_ms: 0, reorder: 1.0, bandwidth_pps: 0, max_queue: 8, ge_good_to_bad: 0.0, ge_bad_to_good: 0.0, ge_loss_good: 0.0, ge_loss_bad: 0.0, duplicate: 0.0, corruption: 0.0 };
		let mut sim = NetworkSimulator::new(cfg, 7);
		let evs = sim.send_burst(5);
		assert!(evs.windows(2).all(|w| w[0].delivery_ms <= w[1].delivery_ms));
	}

	#[test]
	fn bandwidth_queue_and_tail_drop() {
		// Very limited bandwidth -> only a few departures fit without exceeding max_queue
		let cfg = SimConfig { loss: 0.0, latency_ms: 1, jitter_ms: 0, reorder: 0.0,
			bandwidth_pps: 10, max_queue: 3,
			ge_good_to_bad: 0.0, ge_bad_to_good: 0.0, ge_loss_good: 0.0, ge_loss_bad: 0.0,
			duplicate: 0.0, corruption: 0.0 };
		let mut sim = NetworkSimulator::new(cfg, 1);
		// Enqueue 10 packets; only up to max_queue should be accepted in this batch
		let evs = sim.send_burst(10);
		assert!(evs.len() <= cfg.max_queue);
		assert!(sim.queue_depth <= cfg.max_queue);
	}

	#[test]
	fn duplicate_and_corruption_flags() {
		let cfg = SimConfig { loss: 0.0, latency_ms: 1, jitter_ms: 0, reorder: 0.0,
			bandwidth_pps: 0, max_queue: 128,
			ge_good_to_bad: 0.0, ge_bad_to_good: 0.0, ge_loss_good: 0.0, ge_loss_bad: 0.0,
			duplicate: 1.0, corruption: 1.0 };
		let mut sim = NetworkSimulator::new(cfg, 2);
		let evs = sim.send_burst(5);
		// With duplicate=1.0, each accepted packet yields two events
		assert_eq!(evs.len() % 2, 0);
		assert!(evs.iter().all(|e| e.corrupted));
		// For each seq, exactly two events should exist and be 1ms apart (since jitter=0)
		use std::collections::BTreeMap;
		let mut by_seq: BTreeMap<u64, Vec<&DeliveryEvent>> = BTreeMap::new();
		for e in &evs { by_seq.entry(e.seq).or_default().push(e); }
		for (_s, v) in by_seq.iter() {
			assert_eq!(v.len(), 2);
			let d0 = v[0].delivery_ms.min(v[1].delivery_ms);
			let d1 = v[0].delivery_ms.max(v[1].delivery_ms);
			assert!(d1.saturating_sub(d0) <= 1);
		}
	}

	#[test]
	fn gilbert_elliott_burst_loss() {
		// Configure strong bursts: once in bad state, drop almost always
		let cfg = SimConfig { loss: 0.0, latency_ms: 1, jitter_ms: 0, reorder: 0.0,
			bandwidth_pps: 0, max_queue: 1024,
			ge_good_to_bad: 0.5, ge_bad_to_good: 0.1, ge_loss_good: 0.01, ge_loss_bad: 0.9,
			duplicate: 0.0, corruption: 0.0 };
		let mut sim = NetworkSimulator::new(cfg, 3);
		let evs = sim.send_burst(200);
		// Expect some loss overall
		assert!(evs.len() < 200);
	}

	#[test]
	fn multipath_weighted_distribution() {
		let cfg = SimConfig { loss: 0.0, latency_ms: 5, jitter_ms: 1, reorder: 0.0,
			bandwidth_pps: 0, max_queue: 128,
			ge_good_to_bad: 0.0, ge_bad_to_good: 0.0, ge_loss_good: 0.0, ge_loss_bad: 0.0,
			duplicate: 0.0, corruption: 0.0 };
		let seeds = [10u64, 11u64, 12u64];
		let weights = Some(vec![2.0, 1.0, 1.0]);
		let mut m = MultiPathSimulator::new_n(cfg, &seeds, weights);
		let n = 40;
		let evs = m.send_burst(n);
		assert_eq!(evs.len(), n);
		// Count per-seq modulo assumption: each path allocates independent seq starting at 0
		// We can't tell path directly from event, but distribution should be stable across seeds.
		// Basic check: merged is time-sorted and non-decreasing by delivery.
		assert!(evs.windows(2).all(|w| w[0].delivery_ms <= w[1].delivery_ms));
	}
}

/// Multi-path simulator that fans out a flow over N paths and merges delivery schedule.
pub struct MultiPathSimulator {
	paths: Vec<NetworkSimulator>,
	weights: Vec<f64>,
	rr_cursor: usize,
}

impl MultiPathSimulator {
	/// Construct a multipath simulator from N identical configs but different seeds.
	pub fn new_n(cfg: SimConfig, seeds: &[u64], weights: Option<Vec<f64>>) -> Self {
		let paths = seeds.iter().copied().map(|s| NetworkSimulator::new(cfg, s)).collect::<Vec<_>>();
		let w = weights.unwrap_or_else(|| vec![1.0; seeds.len()]);
		assert_eq!(paths.len(), w.len());
		Self { paths, weights: w, rr_cursor: 0 }
	}

	/// Send `n` packets split across paths by weighted round-robin.
	pub fn send_burst(&mut self, n: usize) -> Vec<DeliveryEvent> {
		if self.paths.is_empty() || n == 0 { return Vec::new(); }
		// Precompute integer quotas by normalized weights
		let sum_w: f64 = self.weights.iter().sum();
		let mut quotas = self.weights.iter().map(|w| ((*w / sum_w) * n as f64).floor() as usize).collect::<Vec<_>>();
		let mut assigned: usize = quotas.iter().sum();
		// Distribute remaining via round-robin starting from rr_cursor
		let mut idx = self.rr_cursor % self.paths.len();
		while assigned < n { quotas[idx] += 1; assigned += 1; idx = (idx + 1) % self.paths.len(); }
		self.rr_cursor = idx;

		// Collect per-path events and merge by (time, seq-within-path-id, path-index)
		let mut merged: Vec<(u64, u64, usize, DeliveryEvent)> = Vec::with_capacity(n);
		for (pi, (p, q)) in self.paths.iter_mut().zip(quotas.into_iter()).enumerate() {
			let mut evs = p.send_burst(q);
			for e in evs.drain(..) {
				// Make sequence globally unique using path index in the tiebreak key only
				merged.push((e.delivery_ms, e.seq, pi, e.clone()));
			}
		}
		merged.sort_by_key(|k| (k.0, k.1, k.2));
		merged.into_iter().map(|(_, _, _, e)| e).collect()
	}
}

