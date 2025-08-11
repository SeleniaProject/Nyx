# HPKE Rekeying in Nyx Stream Layer

This document describes the automated HPKE-based session rekey mechanism integrated into `nyx-stream`.

## Overview
Periodic rekeying limits key lifetime and reduces cryptanalytic exposure. Nyx implements a lightweight policy-driven HPKE rekey manager that:
- Triggers rekey on either elapsed time or packet count threshold.
- Maintains a grace window where the previous symmetric key remains valid for decryption only.
- Generates a Rekey Frame carrying a freshly sealed 32‑byte session key (HPKE X25519-HKDF-SHA256 + ChaCha20-Poly1305).
- Applies new key locally immediately after frame generation.
- Accepts inbound Rekey Frames, installing the new key and starting a new grace period.

## Components
| File | Purpose |
|------|---------|
| `nyx-stream/src/hpke_rekey_manager.rs` | Policy evaluation, key rotation state, grace logic |
| `nyx-stream/src/hpke_rekey.rs` | Rekey frame format, seal/open helpers, inbound processing |
| `nyx-stream/src/tx.rs` | Outbound integration: evaluate per send, generate frame, queue for control channel |
| `nyx-telemetry/src/lib.rs` | Rekey metrics registration & helpers |

## Policy (`RekeyPolicy`)
Fields:
- `time_interval`: max duration for a key
- `packet_interval`: max number of packets protected by a key
- `grace_period`: duration old key is accepted for decryption

Rekey if `now - last_rekey >= time_interval || packets_since_rekey >= packet_interval`.

## Rekey Frame
Serialized layout (length-prefixed):
```
[ EncLen(16) | EncappedKey | CtLen(16) | Ciphertext ]
```
Ciphertext HPKE-seals a new 32‑byte Nyx session key.

## Outbound Flow
1. Application sends data via `TxQueue::send_with_path`.
2. `HpkeRekeyManager::on_packet_sent()` updates counters & returns decision.
3. If `Initiate`, `seal_for_rekey()` constructs frame + new session key.
4. Manager installs new key (old moved to grace slot).
5. Frame bytes stored in `pending_rekey_frames` (to be sent over control channel / CRYPTO stream).

## Inbound Flow
`process_inbound_rekey(manager, sk_r, bytes, info)`:
1. Parse frame.
2. Open with receiver private key -> new session key.
3. `manager.accept_remote_rekey(new_key)`; old key enters grace period.

## Grace Decryption
`HpkeRekeyManager::try_decrypt` attempts with current key; if fails and grace still active, retries previous key. Successful grace usages increment a metric.

## Telemetry Metrics
(All `IntCounter`)
- `nyx_hpke_rekey_initiated_total` – decisions to start rekey
- `nyx_hpke_rekey_applied_total` – successful new key installations (both outbound & inbound)
- `nyx_hpke_rekey_grace_used_total` – decrypt operations that used prior key in grace window
- `nyx_hpke_rekey_fail_total` – failures (generation / decryption / parse)

Gauges or histograms can be added later if needed (e.g., key lifetime distribution).

## Public APIs
```rust
TxQueue::enable_hpke_rekey(manager, peer_public_key).await;
TxQueue::drain_rekey_frames().await;            // Retrieve raw frames (one-shot)
TxQueue::flush_rekey_frames(|bytes| { /* send */ true }); // Attempt send, retain unsent
process_inbound_rekey(&mut manager, &receiver_sk, frame_bytes, b"nyx-hpke-rekey");
```

## Testing
- `hpke_rekey_integration_tests.rs`: verifies packet threshold triggers frame generation.
- `hpke_rekey::inbound_process_success`: validates inbound processing installs new key.

## Future Work
- Wire `flush_rekey_frames` into a real control / CRYPTO channel dispatcher.
- Add negative tests: malformed frame, wrong key, grace expiry behavior.
- Key lifetime histogram & last-failure reason metric.
- Configurable HPKE suite negotiation if multiple KEM/KDF/AEAD supported later.

## Security Notes
- Old key retained only for `grace_period`; short grace reduces exposure to replay / key compromise.
- Rekey frames must be authenticated at higher layer (e.g. integrity via existing channel protections) to prevent spoofed rotations.
- Consider rate limiting rekey attempts to mitigate potential DoS via forced rotations.
