# Hybrid Post-Quantum Handshake Implementation

## Overview

This document describes the implementation of the hybrid post-quantum handshake protocol in nyx-crypto, combining classical X25519 elliptic curve cryptography with post-quantum Kyber KEM for forward secrecy against quantum attacks.

## Architecture

### Core Components

1. **Classical Cryptography**: X25519 Elliptic Curve Diffie-Hellman
2. **Post-Quantum Cryptography**: Kyber KEM (Key Encapsulation Mechanism)
3. **Hybrid Protocol**: Combines both for quantum-resistant security
4. **Telemetry Integration**: Comprehensive metrics and monitoring
5. **HPKE Support**: RFC9180 Hybrid Public Key Encryption for envelope encryption

### Protocol Flow

```
Initiator (Alice)                    Responder (Bob)
--------------                      ---------------
1. Generate ephemeral X25519 key
2. Encrypt static X25519 pk with Bob's static pk
3. Encapsulate to Bob's Kyber pk
4. Mix classical DH + PQ shared secrets
5. Send MSG1 → [e_pk || ct_classic || ct_pq]
                                    ←
                                    6. Decrypt Alice's static pk
                                    7. Decapsulate Kyber ciphertext
                                    8. Mix classical DH + PQ secrets
                                    9. ← Send MSG2 (acknowledgment)
10. Verify MSG2
11. Establish secure channels (TX/RX)
```

## Implementation Details

### Key Types

```rust
// Classical X25519 static keypair
pub struct X25519StaticKeypair {
    pub pk: [u8; 32],
    pub sk: [u8; 32],
}

// Post-quantum Kyber static keypair
pub struct KyberStaticKeypair {
    pub pk: KyberPublicKey,
    pub sk: KyberSecretKey,
}
```

### Handshake Functions

#### Initiator Handshake
```rust
pub fn initiator_handshake(
    i_static: &X25519StaticKeypair,
    i_pq: &KyberStaticKeypair,
    r_static_pk_x: &[u8; 32],
    r_pq_pk: &KyberPublicKey,
) -> Result<InitiatorResult>
```

- Generates ephemeral X25519 keypair
- Performs classical DH operations
- Encapsulates to responder's Kyber public key
- Mixes classical and PQ shared secrets
- Returns MSG1 and session keys

#### Responder Handshake
```rust
pub fn responder_handshake(
    msg1: &[u8],
    i_static_pk_expected: &[u8; 32],
    r_static: &X25519StaticKeypair,
    r_pq: &KyberStaticKeypair,
) -> Result<ResponderResult>
```

- Parses MSG1 from initiator
- Decrypts initiator's static public key
- Decapsulates Kyber ciphertext
- Derives same mixed shared secret
- Returns MSG2 acknowledgment and session keys

### Security Properties

1. **Forward Secrecy**: Ephemeral keys ensure past sessions remain secure
2. **Quantum Resistance**: Kyber KEM provides post-quantum security
3. **Classical Security**: X25519 provides proven elliptic curve security
4. **Hybrid Security**: Security if either classical OR post-quantum assumptions hold

### Message Format

#### MSG1 (Initiator → Responder)
```
| Header (4 bytes) | e_pk (32 bytes) | ct_classic (48 bytes) | ct_pq_len (2 bytes) | ct_pq (variable) |
| "Nx" + ver + flags | X25519 ephemeral | Encrypted static pk | Kyber ct length | Kyber ciphertext |
```

#### MSG2 (Responder → Initiator)
```
| Header (4 bytes) | encrypted_ack (16 bytes) |
| "Nx" + ver + flags | ChaCha20Poly1305(ACK) |
```

### Telemetry Integration

The implementation includes comprehensive telemetry for monitoring:

```rust
pub struct HandshakeTelemetry {
    attempts: AtomicU64,
    successes: AtomicU64,
    failures: AtomicU64,
    classic_dh_ops: AtomicU64,
    pq_ops: AtomicU64,
}
```

Metrics tracked:
- Handshake attempt/success/failure rates
- Classical DH operation counts
- Post-quantum operation counts
- Error categorization and logging

### HPKE Integration

Support for RFC9180 Hybrid Public Key Encryption:

```rust
impl HybridHandshake {
    pub fn create_hpke_context(
        recipient_info: &[u8],
        context_info: &[u8],
    ) -> Result<([u8; 32], HpkeContext)>

    pub fn open_hpke_context(
        public_key: &[u8; 32],
        recipient_info: &[u8],
        context_info: &[u8],
    ) -> Result<HpkeContext>
}
```

## Testing

### End-to-End Tests

1. **Round-trip handshake**: Complete initiator-responder flow
2. **Key derivation**: Verify consistent session keys
3. **Message encryption**: Bidirectional secure communication
4. **Error handling**: Invalid keys, corrupted messages
5. **Telemetry validation**: Metrics accuracy
6. **HPKE envelope encryption**: Additional security layer

### Security Tests

1. **Static key validation**: Reject wrong identity keys
2. **Message integrity**: Detect tampering
3. **Replay protection**: Prevent message reuse
4. **Forward secrecy**: Independent sessions

## Performance Characteristics

### Computational Overhead
- X25519 operations: ~50μs per operation
- Kyber KEM operations: ~100-200μs per operation
- Total handshake: ~500μs typical
- Session encryption: ChaCha20Poly1305 performance

### Message Sizes
- MSG1: ~1.2KB (32 + 48 + 1088 + overhead)
- MSG2: ~20 bytes (4 + 16)
- Session overhead: 16 bytes per message (ChaCha20Poly1305 tag)

## Integration Guidelines

### Basic Usage

```rust
use nyx_crypto::hybrid::handshake::*;
use nyx_crypto::hybrid::{KyberStaticKeypair, X25519StaticKeypair};

// Generate long-term keypairs
let alice_x25519 = X25519StaticKeypair::generate();
let alice_kyber = KyberStaticKeypair::generate();
let bob_x25519 = X25519StaticKeypair::generate();
let bob_kyber = KyberStaticKeypair::generate();

// Alice initiates handshake
let alice_result = initiator_handshake(
    &alice_x25519,
    &alice_kyber,
    &bob_x25519.pk,
    &bob_kyber.pk,
)?;

// Bob responds
let bob_result = responder_handshake(
    &alice_result.msg1,
    &alice_x25519.pk,
    &bob_x25519,
    &bob_kyber,
)?;

// Alice verifies
let mut alice_final = alice_result;
initiator_verify_msg2(&mut alice_final, &bob_result.msg2)?;

// Secure communication
let message = b"Hello, quantum-resistant world!";
let encrypted = alice_final.tx.encrypt(message)?;
let decrypted = bob_result.rx.decrypt(&encrypted)?;
```

### Feature Flags

- `hybrid`: Enable hybrid X25519 + Kyber implementation
- `telemetry`: Enable metrics and monitoring
- `hpke`: Enable HPKE envelope encryption support

## Security Considerations

1. **Key Management**: Store static keys securely
2. **Ephemeral Keys**: Ensure proper ephemeral key generation
3. **Random Number Generation**: Use cryptographically secure RNG
4. **Side-Channel Protection**: Consider timing attack mitigations
5. **Quantum Timeline**: Monitor NIST PQC standardization updates

## Future Work

1. **ML-KEM Integration**: Migrate to NIST-standardized ML-KEM
2. **Additional PQ Algorithms**: Support for NTRU, SIKE alternatives
3. **Hardware Acceleration**: Optimize for specific platforms
4. **Formal Verification**: Mathematical proof of security properties
5. **Performance Optimization**: Reduce computational and bandwidth overhead
