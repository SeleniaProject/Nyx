# nyx-fec

Nyx forward error correction utilities focused on fixed-size 1280-byte shards with adaptive redundancy tuning capabilities.

## Features

- **Safe Rust**: No `unsafe` code and no external C/C++ backends by default
- **Reed-Solomon FEC**: GF(2^8) wrappers specialized for `[u8; 1280]` shards
- **Adaptive RaptorQ**: Intelligent redundancy tuning based on network conditions
- **Variable Payload Support**: Helpers for packing variable payloads with 2-byte length prefix
- **Performance Optimized**: Comprehensive benchmarking and memory-efficient algorithms

## Adaptive Redundancy Tuning

The crate includes a sophisticated adaptive redundancy tuning system that automatically adjusts FEC parameters based on real-time network conditions:

```rust
use nyx_fec::raptorq::{AdaptiveRedundancyTuner, NetworkMetrics};

let mut tuner = AdaptiveRedundancyTuner::new();

// Update with current network conditions
let metrics = NetworkMetrics::new(
    150,    // RTT in ms
    25,     // Jitter in ms  
    0.02,   // Loss rate (2%)
    1500    // Bandwidth in kbps
);

let redundancy = tuner.update(metrics);
println!("Recommended redundancy: TX={:.1}%, RX={:.1}%", 
         redundancy.tx * 100.0, redundancy.rx * 100.0);
```

### Adaptive Algorithm Features

- **PID Controller**: Proportional-Integral-Derivative control for stable adaptation
- **Loss Rate Tracking**: Moving average with trend analysis
- **Network Quality Score**: Combined RTT, jitter, and loss assessment
- **Bandwidth Awareness**: Adjusts redundancy based on available bandwidth
- **Historical Context**: Maintains measurement history for informed decisions
- **Stability Detection**: Reduces redundancy adjustments in stable conditions

## Crate Features

- `raptorq`: Enables adaptive redundancy tuning API (default: disabled)
- `telemetry`: Reserved for future metrics hooks

## Basic Usage

### Shard Packing/Unpacking
```rust
use nyx_fec::padding::{pack_into_shard, unpack_from_shard, SHARD_SIZE};

let payload = b"hello world";
let shard = pack_into_shard(payload);
let recovered = unpack_from_shard(&shard);
assert_eq!(recovered, payload);
```

### Reed-Solomon Encoding
```rust
use nyx_fec::rs1280::{Rs1280, RsConfig};
use nyx_fec::padding::SHARD_SIZE;

let cfg = RsConfig { data_shards: 4, parity_shards: 2 };
let rs = Rs1280::new(cfg).unwrap();

let mut shards: Vec<[u8; SHARD_SIZE]> = (0..cfg.total_shards())
    .map(|i| { let mut a = [0u8; SHARD_SIZE]; a[0] = i as u8; a })
    .collect();

let (data, parity) = shards.split_at_mut(cfg.data_shards);
let data_refs: Vec<&[u8; SHARD_SIZE]> = data.iter().collect();
let mut parity_refs: Vec<&mut [u8; SHARD_SIZE]> = parity.iter_mut().collect();

rs.encode_parity(&data_refs, &mut parity_refs).unwrap();
```

### Safe Unpacking
```rust
use nyx_fec::padding::try_unpack_from_shard;

if let Some(payload) = try_unpack_from_shard(&shard) {
    // Use trusted payload
    println!("Received: {:?}", payload);
}
```

## Performance

The adaptive tuning system is designed for high-performance applications:

- **Single Update**: < 1μs typical latency
- **Batch Processing**: Optimized for continuous measurement streams  
- **Memory Efficient**: Bounded history with configurable limits
- **Zero Allocations**: After initialization in steady state

Run benchmarks with:
```bash
cargo bench --features raptorq
```

## Testing

Comprehensive test coverage includes:

- **Unit Tests**: Core algorithm validation
- **Integration Tests**: End-to-end scenario testing
- **Property Tests**: Invariant verification across input ranges
- **Performance Tests**: Regression detection

```bash
# Run all tests
cargo test --features raptorq

# Run specific test category
cargo test --features raptorq adaptive_raptorq_comprehensive
```

## Architecture

The adaptive redundancy system uses a multi-layered approach:

1. **Network Metrics Collection**: RTT, jitter, loss rate, bandwidth
2. **Quality Assessment**: Combined scoring with weighted factors
3. **PID Control Loop**: Stable feedback-based adjustment
4. **Multi-Factor Modulation**: Bandwidth, stability, and trend considerations
5. **Bounded Output**: Safety constraints and practical limits

This ensures robust performance across diverse network conditions while maintaining computational efficiency.

## License
Dual-licensed under MIT or Apache-2.0.
