# RaptorQ FEC Guide

This document describes NyxNet's RaptorQ-based Forward Error Correction (FEC) implementation and how to use it effectively.

## Overview

- Symbol size: 1280 bytes (one Nyx packet per symbol)
- Redundancy: fixed ratio via `RaptorQCodec` or adaptive via `AdaptiveRaptorQ`
- Sentinel: a small sentinel packet encodes the original data length to ensure exact recovery

## API Quickstart (Rust)

```rust
use nyx_fec::{RaptorQCodec, AdaptiveRaptorQ};

// Fixed redundancy (30%)
let codec = RaptorQCodec::new(0.3);
let data = vec![0u8; 4096];
let packets = codec.encode(&data);
let recovered = codec.decode(&packets).expect("decode");
assert_eq!(recovered, data);

// Adaptive redundancy (initial 10%, min 5%, max 60%)
let mut adaptive = AdaptiveRaptorQ::new(0.10, 16, 0.05, 0.60);
adaptive.record(false); // packet received
adaptive.record(true);  // packet lost
let packets2 = adaptive.encode(&data);
let recovered2 = adaptive.decode(&packets2).expect("decode");
```

## Encoding Details

`RaptorQCodec::encode` emits:
- A replicated sentinel packet (block=0xFF, esi=0xFFFFFF) that carries the original data length (u64, big-endian)
- Source and repair symbols derived from the input

The sentinel enables exact-length recovery without out-of-band metadata. Sentinel packets are filtered during decoding.

## Adaptive Strategy

`AdaptiveRaptorQ` maintains a sliding window of loss outcomes and (optionally) network conditions:
- Loss rate, RTT, bandwidth estimate, congestion level
- A weighted score drives target redundancy within configured bounds
- Change is limited by `adaptation_speed` and `stability_threshold` to avoid oscillation

## Metrics & Stats

- `RaptorQCodec::get_stats()` exposes total blocks, repair symbol counts, redundancy history, and average encoding time
- `AdaptiveRaptorQ::get_stats()` aggregates encoding/decoding statistics and current redundancy

## Benchmarks

Run with your Rust toolchain:

```bash
cargo bench -p nyx-fec
```

Recommended methodology:
- Vary redundancy ratios (0.05 .. 0.80) and loss patterns (burst/random)
- Measure encoding/decoding throughput and success probability vs. symbol loss
- Validate recovery under MTU-aligned payloads (1280-byte multiples) and non-multiples

## Integration Notes

- Zero-copy path: `nyx-core` integrates via a trait adapter so the FEC layer can be swapped or disabled
- When FEC is disabled, timing obfuscation can still be applied via `nyx_fec::timing`


