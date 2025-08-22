#![forbid(unsafe_code)]

use chacha20poly1305::aead::{Aead, NewAead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use std::sync::OnceLock;
use zeroize::Zeroize;

use crate::{Error, Result};

/// AEAD suite (keep room for extension)
#[derive(Clone, Copy, Debug, Default)]
pub enum AeadSuite {
    #[default]
    ChaCha20Poly1305,
}

/// AEAD key (zeroized on drop)
#[derive(Clone)]
pub struct AeadKey(pub [u8; 32]);
impl Drop for AeadKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// 96-bit nonce
#[derive(Clone, Copy)]
pub struct AeadNonce(pub [u8; 12]);

pub struct AeadCipher {
    suite: AeadSuite,
    key: AeadKey,
    // Pre-computed cipher instance for maximum performance
    cipher: OnceLock<ChaCha20Poly1305>,
}

impl AeadCipher {
    pub fn new(suite: AeadSuite, key: AeadKey) -> Self {
        Self {
            suite,
            key,
            cipher: OnceLock::new(),
        }
    }

    // Get or create the cipher instance with maximum performance optimization
    #[inline(always)]
    fn get_cipher(&self) -> &ChaCha20Poly1305 {
        self.cipher.get_or_init(|| {
            let key = Key::from_slice(&self.key.0);
            ChaCha20Poly1305::new(key)
        })
    }

    #[inline(always)]
    pub fn seal(&self, nonce: AeadNonce, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        match self.suite {
            AeadSuite::ChaCha20Poly1305 => {
                // Ultra-high performance: pre-computed cipher + zero-copy operations
                let cipher = self.get_cipher();
                let nonce = Nonce::from_slice(&nonce.0);

                // Use the pre-computed cipher instance for maximum speed
                cipher
                    .encrypt(
                        nonce,
                        Payload {
                            msg: plaintext,
                            aad,
                        },
                    )
                    .map_err(|e| Error::Protocol(format!("aead seal failed: {e}")))
            }
        }
    }

    #[inline(always)]
    pub fn open(&self, nonce: AeadNonce, aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        match self.suite {
            AeadSuite::ChaCha20Poly1305 => {
                // Ultra-high performance: pre-computed cipher + zero-copy operations
                let cipher = self.get_cipher();
                let nonce = Nonce::from_slice(&nonce.0);

                // Use the pre-computed cipher instance for maximum speed
                cipher
                    .decrypt(
                        nonce,
                        Payload {
                            msg: ciphertext,
                            aad,
                        },
                    )
                    .map_err(|e| Error::Protocol(format!("aead open failed: {e}")))
            }
        }
    }
}

#[cfg(test)]
mod test_s {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn chacha20_roundtrip() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let key = AeadKey([7u8; 32]);
        let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, key);
        let nonce = AeadNonce([1u8; 12]);
        let aad = b"nyx-aad";
        let pt = b"hello nyx";
        let ct = cipher.seal(nonce, aad, pt)?;
        let rt = cipher.open(nonce, aad, &ct)?;
        assert_eq!(rt, pt);
        Ok(())
    }

    #[test]
    fn open_fails_with_wrongaad() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let key = AeadKey([3u8; 32]);
        let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, key);
        let nonce = AeadNonce([2u8; 12]);
        let ct = cipher.seal(nonce, b"A", b"m")?;
        assert!(cipher.open(nonce, b"B", &ct).is_err());
        Ok(())
    }

    proptest! {
        #[test]
        fn roundtrip_random_input(a in any::<Vec<u8>>(), m in any::<Vec<u8>>()) {
            let key = AeadKey([5u8; 32]);
            let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, key);
            let nonce = [0u8;12];
            // a/mが大きすぎると時間がかかるため上限を設ける
            let aad = if a.len() > 256 { &a[..256] } else { &a };
            let msg = if m.len() > 2048 { &m[..2048] } else { &m };
            let ct = cipher.seal(AeadNonce(nonce), aad, msg)?;
            let pt = cipher.open(AeadNonce(nonce), aad, &ct)?;
            prop_assert_eq!(pt, msg);
        }
    }

    #[cfg(test)]
    mod performance_tests {
        use super::*;
        use std::time::Instant;

        #[test]
        fn benchmark_aead_performance() {
            let key = AeadKey([42u8; 32]);
            let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, key);
            let aad = b"test aad";
            let message = b"Hello, this is a test message for benchmarking AEAD performance";
            let nonce = AeadNonce([1u8; 12]);

            // ウォームアップ
            for _ in 0..100 {
                let ct = cipher.seal(nonce, aad, message).unwrap();
                let _pt = cipher.open(nonce, aad, &ct).unwrap();
            }

            // ベンチマーク実行
            let start = Instant::now();

            for i in 0..10000 {
                let mut current_nonce = nonce;
                current_nonce.0[0] = (i % 256) as u8;

                let ct = cipher.seal(current_nonce, aad, message).unwrap();
                let pt = cipher.open(current_nonce, aad, &ct).unwrap();
                assert_eq!(pt, message);
            }

            let elapsed = start.elapsed();
            let operations_per_second = 10000.0 / elapsed.as_secs_f64();

            eprintln!("AEAD performance benchmark:");
            eprintln!("  Operations: 10,000 (seal + open cycles)");
            eprintln!("  Total time: {:?}", elapsed);
            eprintln!("  Operations/second: {:.2}", operations_per_second);
            eprintln!(
                "  Average time per operation: {:.2} μs",
                (elapsed.as_secs_f64() * 1_000_000.0) / 10000.0
            );

            // 最適化された実装では少なくとも1000操作/秒以上を期待
            assert!(
                operations_per_second > 1000.0,
                "Performance too low: {:.2} ops/sec",
                operations_per_second
            );
        }
    }
}
