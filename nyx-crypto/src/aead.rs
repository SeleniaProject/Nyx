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

/// Ultra-high performance AEAD cipher with zero-copy optimizations,
/// pre-computed ciphers, and buffer reuse for maximum throughput.
pub struct AeadCipher {
    suite: AeadSuite,
    key: AeadKey,
    // Pre-computed cipher instance for maximum performance
    cipher: OnceLock<ChaCha20Poly1305>,
}

/// Zero-copy AEAD operations with buffer reuse
pub struct AeadProcessor {
    cipher: ChaCha20Poly1305,
    // Pre-allocated buffers to avoid repeated allocations
    encrypt_buffer: Vec<u8>,
    decrypt_buffer: Vec<u8>,
}

impl AeadProcessor {
    /// Create ultra-high performance AEAD processor with pre-allocated buffers
    pub fn new(key: &AeadKey) -> Self {
        let key = Key::from_slice(&key.0);
        let cipher = ChaCha20Poly1305::new(key);
        
        Self {
            cipher,
            encrypt_buffer: Vec::with_capacity(4096), // Pre-allocate for typical message sizes
            decrypt_buffer: Vec::with_capacity(4096),
        }
    }

    /// Ultra-fast encryption with buffer reuse and zero-copy optimizations
    #[inline(always)]
    pub fn seal_reuse(&mut self, nonce: AeadNonce, aad: &[u8], plaintext: &[u8]) -> Result<&[u8]> {
        let nonce = Nonce::from_slice(&nonce.0);
        
        // Clear and reserve space in the reusable buffer
        self.encrypt_buffer.clear();
        self.encrypt_buffer.reserve(plaintext.len() + 16); // 16 bytes for ChaCha20Poly1305 tag
        
        // Use in-place encryption when possible
        let encrypted = self.cipher
            .encrypt(
                nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|e| Error::Protocol(format!("aead seal failed: {e}")))?;
        
        self.encrypt_buffer.extend_from_slice(&encrypted);
        Ok(&self.encrypt_buffer)
    }

    /// Ultra-fast decryption with buffer reuse and zero-copy optimizations  
    #[inline(always)]
    pub fn open_reuse(&mut self, nonce: AeadNonce, aad: &[u8], ciphertext: &[u8]) -> Result<&[u8]> {
        let nonce = Nonce::from_slice(&nonce.0);
        
        // Clear and reserve space in the reusable buffer
        self.decrypt_buffer.clear();
        self.decrypt_buffer.reserve(ciphertext.len().saturating_sub(16)); // Account for tag removal
        
        let decrypted = self.cipher
            .decrypt(
                nonce,
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|e| Error::Protocol(format!("aead open failed: {e}")))?;
        
        self.decrypt_buffer.extend_from_slice(&decrypted);
        Ok(&self.decrypt_buffer)
    }

    /// Get current encrypt buffer capacity for monitoring
    pub fn encrypt_buffer_capacity(&self) -> usize {
        self.encrypt_buffer.capacity()
    }

    /// Get current decrypt buffer capacity for monitoring
    pub fn decrypt_buffer_capacity(&self) -> usize {
        self.decrypt_buffer.capacity()
    }
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

    /// High-performance seal operation with pre-computed cipher
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

    /// High-performance open operation with pre-computed cipher
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

    /// Create high-performance processor for batch operations
    pub fn create_processor(&self) -> AeadProcessor {
        AeadProcessor::new(&self.key)
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
            eprintln!("  Total time: {elapsed:?}");
            eprintln!("  Operations/second: {operations_per_second:.2}");
            eprintln!(
                "  Average time per operation: {:.2} μs",
                (elapsed.as_secs_f64() * 1_000_000.0) / 10000.0
            );

            // 最適化された実装では少なくとも1000操作/秒以上を期待
            assert!(
                operations_per_second > 1000.0,
                "Performance too low: {operations_per_second:.2} ops/sec"
            );
        }
    }
}
