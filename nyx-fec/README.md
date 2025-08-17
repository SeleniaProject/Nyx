# nyx-fec

Nyx forward error correction utilities focused on fixed-size 1280-byte shards.

Highlights:
- Safe Rust, no `unsafe` and no external C/C++ backends by default.
- Reed-Solomon (GF(2^8)) wrappers specialized for `[u8; 1280]` shards.
- Helpers for packing variable payloads with a 2-byte length prefix.
  - `pack_into_shard`, `try_pack_into_shard`, `unpack_from_shard`, `try_unpack_from_shard`.
- Simple timing helpers and a stub for adaptive redundancy tuning.

## Crate features
- `raptorq`: enables the adaptive redundancy helper API surface. The actual
  codec implementation is not included; this is intentionally lightweight.
  Without this feature, the `raptorq` module is not compiled.
- `telemetry`: reserved for future metrics hooks.

This crate intentionally does not enable the `reed-solomon-erasure/simd-accel`
feature to avoid build-time C/C++ toolchain requirements and keep portability
high. If you need maximum throughput and can accept that dependency, depend on
`nyx-fec` from your application and enable `reed-solomon-erasure/simd-accel`
directly there for that specific build.

## Usage sketch
```rust
use nyx_fec::padding::{pack_into_shard, unpack_from_shard, SHARD_SIZE};
use nyx_fec::rs1280::{Rs1280, RsConfig};

let payload = b"hello";
let shard = pack_into_shard(payload);
let recovered = unpack_from_shard(&shard);
assert_eq!(recovered, payload);

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

For safer unpacking when dealing with untrusted shards, prefer:
```rust
use nyx_fec::padding::{try_unpack_from_shard};
if let Some(payload) = try_unpack_from_shard(&shard) {
  // use payload
}
```

## License
Dual-licensed under MIT or Apache-2.0.
