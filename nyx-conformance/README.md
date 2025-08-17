# nyx-conformance

Conformance helpers for the Nyx workspace.

- Deterministic Network Simulator (single/multi-path)
- Property testing utilities (monotonicity, tolerant checks, stats, histograms)

## Highlights
- Pure Rust (no `unsafe`, no C/C++ deps)
- Reproducible via seed; stable ordering and tie-breakers
- Bandwidth/queue model, Gilbert–Elliott burst loss, duplication/corruption flags

## Quick Start

```rust
use nyx_conformance::{NetworkSimulator, SimConfig, check_monotonic_increasing};

let cfg = SimConfig { loss: 0.01, latency_ms: 40, jitter_ms: 8, reorder: 0.1,
    bandwidth_pps: 500, max_queue: 128,
    ge_good_to_bad: 0.0, ge_bad_to_good: 0.0, ge_loss_good: 0.0, ge_loss_bad: 0.0,
    duplicate: 0.0, corruption: 0.0 };
let mut sim = NetworkSimulator::new(cfg, 2024);
let events = sim.send_burst(32);
let times: Vec<f64> = events.iter().map(|e| e.delivery_ms as f64).collect();
check_monotonic_increasing(&times).unwrap();
```

## License
Dual-licensed under MIT or Apache-2.0.
