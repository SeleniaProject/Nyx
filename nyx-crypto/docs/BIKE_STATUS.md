# BIKE KEM Implementation Status

## Summary

BIKE KEM support is **not currently implemented** in Nyx. This document explains the rationale and provides a roadmap for future implementation.

## Current State

- **Module Created**: `nyx-crypto/src/bike.rs` (placeholder)
- **Feature Flag**: `bike` (reserved)
- **API Surface**: Complete type definitions and function signatures
- **Implementation**: Returns `Error::NotImplemented`
- **Alternative**: ML-KEM-768 provides production-ready post-quantum security

## Why Not Implemented?

### 1. No Pure Rust Implementation Available

As of October 2025, all available BIKE implementations depend on C/C++ libraries:

- `bike-kem`: FFI bindings to C implementation
- `pqcrypto-bike`: Uses C codebase via FFI
- No audited Pure Rust implementation exists

**Project Requirement**: Nyx strictly forbids C/C++ dependencies for:
- Memory safety guarantees
- WebAssembly compatibility
- Simplified build process
- Security audit surface reduction

### 2. NIST Standardization Status

- **BIKE**: Round 3 alternate candidate, **not standardized**
- **ML-KEM** (Kyber): **FIPS 203 standard** (finalized August 2024)

NIST selected ML-KEM as the primary post-quantum KEM standard. The cryptographic community has converged on ML-KEM for production use.

### 3. Security Considerations

- **ML-KEM-768**: Extensively analyzed, multiple independent audits
- **BIKE**: Less mature, smaller security analysis corpus
- **Implementation Risk**: Custom cryptography implementation requires expert review

### 4. Maintenance Burden

Implementing BIKE from scratch would require:
- 3-6 months of development time
- Cryptographic expertise for constant-time operations
- Ongoing security maintenance
- Protection against side-channel attacks

## Alternative: ML-KEM-768

Nyx uses ML-KEM-768 for post-quantum security, which provides:

- **NIST Standard**: FIPS 203 compliance
- **Pure Rust**: RustCrypto's `ml-kem` crate (no C dependencies)
- **Security Level**: AES-192 equivalent (Category 3)
- **Performance**: ~1ms keygen, ~1.5ms encapsulation on modern CPUs
- **Audited**: Multiple independent security reviews

## Specification Compliance

The Nyx Protocol v1.0 Specification lists BIKE as an **optional** feature:

> **Feature Differences (v0.1 → v1.0)**
> | Cryptography | X25519, Kyber optional | PQ-Only mode (Kyber/**BIKE**), Hybrid DH, HPKE support |

The specification allows implementations to choose their post-quantum algorithms. Nyx's choice of ML-KEM over BIKE is compliant with the spec's optional nature.

## Future Implementation Plan

When a production-grade Pure Rust BIKE implementation becomes available:

### Phase 1: Integration Preparation
- [ ] Evaluate available Pure Rust BIKE libraries
- [ ] Review security audit reports
- [ ] Benchmark performance characteristics
- [ ] Assess API compatibility with Nyx's KEM trait

### Phase 2: Implementation
- [ ] Add BIKE dependency to `Cargo.toml`
- [ ] Implement core KEM operations in `bike.rs`:
  - `keygen()`: Constant-time key generation
  - `encapsulate()`: Secure encapsulation
  - `decapsulate()`: Constant-time decapsulation with proper error handling
- [ ] Add comprehensive test suite:
  - Unit tests (roundtrip, invalid inputs)
  - Property tests (fuzzing)
  - Timing side-channel tests
  - Known Answer Tests (KATs)

### Phase 3: Hybrid Integration
- [ ] Update `nyx-crypto/src/hybrid.rs` to support BIKE mode
- [ ] Implement X25519 + BIKE key combination
- [ ] Add HKDF-based shared secret derivation
- [ ] Update capability negotiation to advertise BIKE support

### Phase 4: Testing & Documentation
- [ ] Add integration tests with `nyx-stream`
- [ ] Benchmark against ML-KEM
- [ ] Update protocol documentation
- [ ] Create migration guide for users

### Phase 5: CI/CD
- [ ] Add GitHub Actions workflow for BIKE feature
- [ ] Add performance benchmarks
- [ ] Configure security scanning

## API Design (Already Defined)

The placeholder implementation defines the complete API:

```rust
// Key generation
pub fn keygen<R: CryptoRng + RngCore>(rng: &mut R) 
    -> Result<(PublicKey, SecretKey)>

// Encapsulation
pub fn encapsulate<R: CryptoRng + RngCore>(
    pk: &PublicKey, 
    rng: &mut R
) -> Result<(Ciphertext, SharedSecret)>

// Decapsulation  
pub fn decapsulate(
    sk: &SecretKey, 
    ct: &Ciphertext
) -> Result<SharedSecret>
```

This API is designed to be:
- **Compatible** with ML-KEM interface
- **Secure** with zeroizing types
- **Ergonomic** with proper error handling
- **Future-proof** for actual implementation

## Monitoring Pure Rust BIKE Development

We are tracking:
- RustCrypto working groups: https://github.com/RustCrypto
- PQCRYPTO Rust projects: https://github.com/rustpq
- Academic implementations transitioning to production

## Decision Log

**2025-10-01**: Decision to implement placeholder with detailed documentation
- **Rationale**: Prioritize working network stack over unimplemented crypto
- **Trade-off**: Spec compliance (optional) vs. security and development velocity
- **Risk Mitigation**: ML-KEM provides equivalent security with better maturity
- **Reversibility**: API surface ready for future implementation

## References

- [NIST PQC Standardization](https://csrc.nist.gov/projects/post-quantum-cryptography)
- [BIKE Specification](https://bikesuite.org/)
- [ML-KEM (FIPS 203)](https://csrc.nist.gov/pubs/fips/203/final)
- [RustCrypto ml-kem](https://github.com/RustCrypto/KEMs/tree/master/ml-kem)
- [Nyx Protocol v1.0 Spec](../spec/Nyx_Protocol_v1.0_Spec_EN.md)
