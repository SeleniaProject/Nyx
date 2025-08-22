#![forbid(unsafe_code)]

use chacha20poly1305::aead::{Aead, NewAead, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
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
    __suite: AeadSuite,
    _key: AeadKey,
}

impl AeadCipher {
    pub fn new(_suite: AeadSuite, key: AeadKey) -> Self {
        Self { __suite: _suite, _key: key }
    }

    pub fn seal(&self, nonce: AeadNonce, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        match self.__suite {
            AeadSuite::ChaCha20Poly1305 => {
                let key = Key::from_slice(&self._key.0);
                let cipher = ChaCha20Poly1305::new(key);
                let nonce = Nonce::from_slice(&nonce.0);
                cipher
                    .encrypt(
                        nonce,
                        Payload {
                            msg: plaintext,
                            aad,
                        },
                    )
                    .map_err(|_| Error::Protocol("aead seal failed".into()))
            }
        }
    }

    pub fn open(&self, nonce: AeadNonce, aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        match self.__suite {
            AeadSuite::ChaCha20Poly1305 => {
                let key = Key::from_slice(&self._key.0);
                let cipher = ChaCha20Poly1305::new(key);
                let nonce = Nonce::from_slice(&nonce.0);
                cipher
                    .decrypt(
                        nonce,
                        Payload {
                            msg: ciphertext,
                            aad,
                        },
                    )
                    .map_err(|_| Error::Protocol("aead open failed".into()))
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
}
