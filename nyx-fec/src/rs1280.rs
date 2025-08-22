//! Reed-Solomon erasure coding for fixed-size 1280-byte shard_s.
use crate::padding::SHARD_SIZE;
use crate::{Error, Result};
use reed_solomon_erasure::galois_8::ReedSolomon;

#[derive(Debug, Clone, Copy)]
pub struct RsConfig {
    pub data_shards: usize,
    pub parity_shards: usize,
}

impl RsConfig {
    pub fn total_shards(&self) -> usize {
        self.data_shards + self.parity_shards
    }
}

pub struct Rs1280 {
    rs: ReedSolomon,
}

impl Rs1280 {
    pub fn new(cfg: RsConfig) -> Result<Self> {
        let rs = ReedSolomon::new(cfg.data_shards, cfg.parity_shards)
            .map_err(|e| Error::Protocol(format!("RS init failed: {e}")))?;
        Ok(Self { rs })
    }

    /// Given D data shards and P parity shards, fill parity in-place.
    pub fn encode_parity(
        &self,
        data: &[&[u8; SHARD_SIZE]],
        parity: &mut [&mut [u8; SHARD_SIZE]],
    ) -> Result<()> {
        let data_slices: Vec<&[u8]> = data.iter().map(|s| s.as_slice()).collect();
        let mut parity_slices: Vec<&mut [u8]> =
            parity.iter_mut().map(|s| s.as_mut_slice()).collect();
        self.rs
            .encode_sep(&data_slices, &mut parity_slices)
            .map_err(|e| Error::Protocol(format!("RS encode failed: {e}")))
    }

    /// Reconstruct missing shards; None entries will be recovered in-place.
    pub fn reconstruct(&self, shards: &mut [Option<[u8; SHARD_SIZE]>]) -> Result<()> {
        // Validate number of shards first to catch misuse early.
        if shards.len() != self.rs.total_shard_count() {
            return Err(Error::Protocol(
                "shard count does not match RS config".into(),
            ));
        }
        // Convert to Option<Vec<u8>> which the crate supports for reconstruction
        let mut tmp: Vec<Option<Vec<u8>>> = shards
            .iter()
            .map(|o| o.as_ref().map(|a| a.as_slice().to_vec()))
            .collect();
        self.rs
            .reconstruct(&mut tmp)
            .map_err(|e| Error::Protocol(format!("RS reconstruct failed: {e}")))?;
        // Copy back into fixed arrays
        for (dst, src) in shards.iter_mut().zip(tmp.into_iter()) {
            match (dst, src) {
                (slot @ None, Some(vec)) => {
                    if vec.len() != SHARD_SIZE {
                        return Err(Error::Protocol("invalid shard size".into()));
                    }
                    let mut a = [0u8; SHARD_SIZE];
                    a.copy_from_slice(&vec);
                    *slot = Some(a);
                }
                (Some(a), Some(vec)) => {
                    if vec.len() != SHARD_SIZE {
                        return Err(Error::Protocol("invalid shard size".into()));
                    }
                    a.copy_from_slice(&vec);
                }
                (Some(_), None) => {
                    return Err(Error::Protocol(
                        "reconstruct returned None for present shard".into(),
                    ))
                }
                (None, None) => {
                    return Err(Error::Protocol(
                        "reconstruct failed to produce shard".into(),
                    ))
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rs_roundtrip_one_loss() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let cfg = RsConfig {
            data_shards: 4,
            parity_shards: 2,
        };
        let rs = Rs1280::new(cfg)?;
        let mut shards: Vec<[u8; SHARD_SIZE]> = (0..cfg.total_shards())
            .map(|i| {
                let mut a = [0u8; SHARD_SIZE];
                a[0] = i as u8;
                a[1] = (i * 3) as u8;
                a
            })
            .collect();

        let (data, parity) = shards.split_at_mut(cfg.data_shards);
        let data_refs: Vec<&[u8; SHARD_SIZE]> = data.iter().collect();
        let mut parity_refs: Vec<&mut [u8; SHARD_SIZE]> = parity.iter_mut().collect();
        rs.encode_parity(&data_refs, &mut parity_refs)?;

        // Lose one random shard
        let mut mix: Vec<Option<[u8; SHARD_SIZE]>> = shards.into_iter().map(Some).collect();
        mix[2] = None;
        rs.reconstruct(&mut mix)?;
        assert!(mix.iter().all(|o| o.is_some()));
        Ok(())
    }

    #[test]
    fn rs_roundtrip_two_losses_with_two_parity(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let cfg = RsConfig {
            data_shards: 4,
            parity_shards: 2,
        };
        let rs = Rs1280::new(cfg)?;
        let mut shards: Vec<[u8; SHARD_SIZE]> = (0..cfg.total_shards())
            .map(|i| {
                let mut a = [0u8; SHARD_SIZE];
                a[0] = (i as u8).wrapping_mul(7);
                a[SHARD_SIZE - 1] = (i as u8).wrapping_mul(11);
                a
            })
            .collect();

        let (data, parity) = shards.split_at_mut(cfg.data_shards);
        let data_refs: Vec<&[u8; SHARD_SIZE]> = data.iter().collect();
        let mut parity_refs: Vec<&mut [u8; SHARD_SIZE]> = parity.iter_mut().collect();
        rs.encode_parity(&data_refs, &mut parity_refs)?;

        let mut mix: Vec<Option<[u8; SHARD_SIZE]>> = shards.into_iter().map(Some).collect();
        // Lose two arbitrary shards (<= parity), should still recover
        mix[1] = None; // data shard
        mix[5] = None; // parity shard
        rs.reconstruct(&mut mix)?;
        assert!(mix.iter().all(|o| o.is_some()));
        Ok(())
    }

    #[test]
    fn rs_reconstruct_fails_when_losses_exceed_parity(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let cfg = RsConfig {
            data_shards: 4,
            parity_shards: 2,
        };
        let rs = Rs1280::new(cfg)?;
        let mut shards: Vec<[u8; SHARD_SIZE]> = (0..cfg.total_shards())
            .map(|i| {
                let mut a = [0u8; SHARD_SIZE];
                a[0] = i as u8;
                a
            })
            .collect();

        let (data, parity) = shards.split_at_mut(cfg.data_shards);
        let data_refs: Vec<&[u8; SHARD_SIZE]> = data.iter().collect();
        let mut parity_refs: Vec<&mut [u8; SHARD_SIZE]> = parity.iter_mut().collect();
        rs.encode_parity(&data_refs, &mut parity_refs)?;

        let mut mix: Vec<Option<[u8; SHARD_SIZE]>> = shards.into_iter().map(Some).collect();
        // Lose three shards (> parity), reconstruction should fail
        mix[0] = None;
        mix[2] = None;
        mix[4] = None;
        let err = rs.reconstruct(&mut mix).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("RS reconstruct failed") || msg.contains("reconstruct failed"));
        Ok(())
    }

    #[test]
    fn rs_reconstruct_preserves_present_shards(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let cfg = RsConfig {
            data_shards: 3,
            parity_shards: 2,
        };
        let rs = Rs1280::new(cfg)?;
        let mut shards: Vec<[u8; SHARD_SIZE]> = (0..cfg.total_shards())
            .map(|i| {
                let mut a = [0u8; SHARD_SIZE];
                a[0] = (i as u8).wrapping_mul(13);
                a
            })
            .collect();

        let (data, parity) = shards.split_at_mut(cfg.data_shards);
        let data_refs: Vec<&[u8; SHARD_SIZE]> = data.iter().collect();
        let mut parity_refs: Vec<&mut [u8; SHARD_SIZE]> = parity.iter_mut().collect();
        rs.encode_parity(&data_refs, &mut parity_refs)?;

        let before = shards.clone();
        let mut mix: Vec<Option<[u8; SHARD_SIZE]>> = shards.into_iter().map(Some).collect();
        mix[4] = None; // lose one
        rs.reconstruct(&mut mix)?;
        for (i, o) in mix.into_iter().enumerate() {
            let a = o.unwrap();
            if i != 4 {
                assert_eq!(a, before[i]);
            }
        }
        Ok(())
    }

    #[test]
    fn rs_reconstruct_rejects_wrong_count() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let cfg = RsConfig {
            data_shards: 2,
            parity_shards: 1,
        };
        let rs = Rs1280::new(cfg)?;
        let mut shards: Vec<Option<[u8; SHARD_SIZE]>> = vec![None; 2]; // should be 3
        let err = rs.reconstruct(&mut shards).unwrap_err();
        assert!(format!("{}", err).contains("shard count"));
        Ok(())
    }
}
