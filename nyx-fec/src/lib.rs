#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! RaptorQ and Reed-Solomon FEC implementation for Nyx packets.
//!
//! NyxNet v1.0 includes complete RaptorQ fountain code implementation
//! with adaptive redundancy control, replacing traditional Reed-Solomon
//! for improved efficiency and resilience.
//!
//! Default Reed-Solomon parameters: data shards = 10, parity shards = 3 (≈30% overhead).
//! RaptorQ parameters: symbol size = 1280 bytes, adaptive redundancy 10-80%.

use reed_solomon_erasure::{galois_8::ReedSolomon, Error as RSError};

pub mod timing;
pub use timing::{Packet, TimingConfig, TimingObfuscator};

pub mod raptorq;
pub use raptorq::{
    AdaptiveRaptorQ, DecodingStats, EncodingStats, FECStats, NetworkCondition, RaptorQCodec,
};

pub mod padding;
pub use padding::{pad_outgoing, trim_incoming};

pub const DATA_SHARDS: usize = 10;
pub const PARITY_SHARDS: usize = 3;
pub const SHARD_SIZE: usize = 1280; // One Nyx packet per shard.

#[cfg(feature = "simd")]
#[cfg_attr(docsrs, doc(cfg(feature = "simd")))]
/// Compile-time feature flag enabling SIMD-accelerated encoding via C backend.
pub const SIMD_ACCEL_ENABLED: bool = true;
#[cfg(not(feature = "simd"))]
pub const SIMD_ACCEL_ENABLED: bool = false;

/// Return a human-readable runtime description for FEC backend.
pub fn fec_backend_description() -> &'static str {
    if SIMD_ACCEL_ENABLED {
        "SIMD-accelerated (no-std C backend)"
    } else {
        "Pure Rust (portable)"
    }
}

/// Nyx FEC codec.
pub struct NyxFec {
    rs: ReedSolomon, // GF(2^8) codec
}

// Bridge: Provide an adapter implementing `nyx_core::zero_copy::integration::fec_integration::FecCodec`
// to enable zero-copy integration without introducing a direct type dependency in nyx-core.
impl nyx_core::zero_copy::integration::fec_integration::FecCodec for RaptorQCodec {
    fn encode(&self, data: &[u8]) -> Vec<Vec<u8>> {
        // Convert raptorq::EncodingPacket into raw Vec<u8> representation.
        // The first packet is a sentinel carrying original length; keep it as the first symbol for downstream handling.
        let packets = RaptorQCodec::encode(self, data);
        packets.into_iter().map(|p| p.data().to_vec()).collect()
    }

    fn decode(
        &self,
        symbols: &[Vec<u8>],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        use ::raptorq::{EncodingPacket, PayloadId};
        // Rebuild EncodingPacket list; assume sentinel remains at index 0 if present.
        let mut packets: Vec<EncodingPacket> = Vec::with_capacity(symbols.len());
        for (i, s) in symbols.iter().enumerate() {
            // Derive a synthetic PayloadId if unknown. Reserve (0xFF, 0xFFFFFF) for sentinel at index 0.
            let pid = if i == 0 {
                PayloadId::new(0xFF, 0xFFFFFF)
            } else {
                // Map sequentially; in practice callers should carry true (block, esi) meta alongside payloads.
                PayloadId::new(0, i as u32 - 1)
            };
            packets.push(EncodingPacket::new(pid, s.clone()));
        }
        RaptorQCodec::decode(self, &packets).ok_or_else(|| "RaptorQ decode failed".into())
    }

    fn current_redundancy(&self) -> f32 {
        self.get_stats()
            .redundancy_history
            .last()
            .copied()
            .unwrap_or(0.0)
    }
}

impl NyxFec {
    /// Create codec with default parameters.
    pub fn new() -> Self {
        let rs = ReedSolomon::new(DATA_SHARDS, PARITY_SHARDS).expect("valid params");
        Self { rs }
    }

    /// Encode data shards in-place. `shards` must be length DATA_SHARDS + PARITY_SHARDS.
    /// First DATA_SHARDS entries are original data; remaining must be zero-filled mutable buffers.
    pub fn encode(&self, shards: &mut [&mut [u8]]) -> Result<(), RSError> {
        self.rs.encode(shards)
    }

    /// Attempt to reconstruct missing shards.
    ///
    /// `present` is a parallel boolean slice indicating which shards are intact.
    pub fn reconstruct(
        &self,
        shards: &mut [&mut [u8]],
        present: &mut [bool],
    ) -> Result<(), RSError> {
        let mut tuples: Vec<(&mut [u8], bool)> = shards
            .iter_mut()
            .enumerate()
            .map(|(i, s)| (&mut **s, present[i]))
            .collect();
        let res = self.rs.reconstruct(&mut tuples);
        if res.is_ok() {
            // Mark all shards as present now
            for p in present.iter_mut() {
                *p = true;
            }
        }
        res
    }

    /// Verify parity for provided shards without reconstruction.
    pub fn verify(&self, shards: &[&[u8]]) -> Result<bool, RSError> {
        self.rs.verify(shards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_shards() -> Vec<Vec<u8>> {
        (0..DATA_SHARDS)
            .map(|i| vec![i as u8; SHARD_SIZE])
            .collect()
    }

    #[test]
    fn encode_and_reconstruct() {
        let codec = NyxFec::new();
        let mut shards: Vec<Vec<u8>> = make_shards();
        // Add parity buffers
        shards.extend((0..PARITY_SHARDS).map(|_| vec![0u8; SHARD_SIZE]));

        // Mutable slice array
        let mut mut_slices: Vec<&mut [u8]> = shards.iter_mut().map(|v| v.as_mut_slice()).collect();
        codec.encode(&mut mut_slices).unwrap();
        let verify_vec = mut_slices.iter().map(|s| &**s).collect::<Vec<&[u8]>>();
        assert!(codec.verify(&verify_vec).unwrap());

        // Zero out two data shards to simulate loss
        let mut present: Vec<bool> = vec![true; DATA_SHARDS + PARITY_SHARDS];
        mut_slices[1].fill(0);
        mut_slices[5].fill(0);
        present[1] = false;
        present[5] = false;

        codec.reconstruct(&mut mut_slices, &mut present).unwrap();
        let verify_vec2 = mut_slices.iter().map(|s| &**s).collect::<Vec<&[u8]>>();
        assert!(codec.verify(&verify_vec2).unwrap());
    }
}
