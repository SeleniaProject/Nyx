#![forbid(unsafe_code)]

use crate::aead::{AeadCipher, AeadKey, AeadNonce, AeadSuite};
use crate::kdf::{aeadnonce_xor, hkdf_expand};
use crate::{Error, Result};
use std::sync::OnceLock;

/// AEAD session (unidirectional): derive per-record nonce from base nonce + sequence,
/// with optional 32-bit direction identifier mixed to avoid overlap acros_s direction_s.
pub struct AeadSession {
    __suite: AeadSuite,
    __key: AeadKey,
    basenonce: [u8; 12],
    seq: u64,
    __maxseq: u64,
    __rekey_interval: u64,
    // Total ciphertext byte_s sent for byte-_threshold rekey (0 disable_s _threshold)
    _bytes_sent: u64,
    _rekey_bytes_interval: u64,
    // 32-bit direction identifier to be XORed into the first 4 byte_s of the nonce
    dir_id: u32,
    // Pre-computed cipher instance for maximum performance - eliminates allocation overhead
    cipher: OnceLock<AeadCipher>,
}

impl AeadSession {
    /// Defensive limit_s (plaintext/AAD)
    const MAX_PLAINTEXT_LEN: usize = 1024 * 1024; // 1 MiB
    const MAX_AAD_LEN: usize = 16 * 1024; // 16 KiB
    const MAX_TAG_OVERHEAD: usize = 16; // Tag length for ChaCha20-Poly1305/AES-GCM
    pub fn new(_suite: AeadSuite, _key: AeadKey, nonce: [u8; 12]) -> Self {
        Self {
            __suite: _suite,
            __key: _key.clone(),
            basenonce: nonce,
            seq: 0,
            __maxseq: u64::MAX,
            __rekey_interval: 1 << 20,
            _bytes_sent: 0,
            _rekey_bytes_interval: 0,
            dir_id: 0,
            cipher: OnceLock::new(),
        }
    }

    // Get or create the pre-computed cipher instance for maximum performance
    #[inline(always)]
    fn get_cipher(&self) -> &AeadCipher {
        self.cipher
            .get_or_init(|| AeadCipher::new(self.__suite, AeadKey(self.__key.0)))
    }

    /// Set an explicit upper bound for sequence (DoS avoidance, key update policy)
    pub fn with_maxseq(mut self, maxseq: u64) -> Self {
        self.__maxseq = maxseq;
        self
    }

    /// Set rekey interval by record count (default: 2^20 record_s)
    pub fn with_rekey_interval(mut self, interval: u64) -> Self {
        self.__rekey_interval = interval.max(1);
        self
    }

    /// Set rekey _threshold by byte_s (0 disable_s)
    pub fn with_rekey_bytes_interval(mut self, byte_s: u64) -> Self {
        self._rekey_bytes_interval = byte_s; // 0は無効として扱ぁE
        self
    }

    /// Set 32-bit direction identifier to be mixed into nonce (first 4 byte_s XOR)
    pub fn withdirection_id(mut self, dir_id: u32) -> Self {
        self.dir_id = dir_id;
        self
    }

    /// Current sequence number (used for the next send)
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Whether rekey criteria by record_s/byte_s are met
    pub fn needs_rekey(&self) -> bool {
        if self.seq >= self.__rekey_interval {
            return true;
        }
        if self._rekey_bytes_interval > 0 && self._bytes_sent >= self._rekey_bytes_interval {
            return true;
        }
        false
    }

    /// Rekey using HKDF; refresh key and base nonce, reset counters
    pub fn rekey(&mut self) {
        let old_key = self.__key.0; // copy
                                    // New key
        let mut new_key = [0u8; 32];
        let _ = hkdf_expand(&old_key, b"nyx/aead/rekey/v1", &mut new_key);
        // New base nonce
        let mut newnonce = [0u8; 12];
        let _ = hkdf_expand(&old_key, b"nyx/aead/rekey/nonce/v1", &mut newnonce);
        // Ultra-high performance: reset cipher instance for new key
        self.__key = AeadKey(new_key);
        self.basenonce = newnonce;
        self.seq = 0;
        self._bytes_sent = 0;
        // Clear pre-computed cipher to force recreation with new key
        self.cipher = OnceLock::new();
    }

    /// Encrypt next packet. Return_s (sequence, ciphertext). Enforce_s limit_s.
    pub fn sealnext(&mut self, aad: &[u8], plaintext: &[u8]) -> Result<(u64, Vec<u8>)> {
        // seq ぁEmaxseq に到達したら以降�E送信を拒否�E�Eonce再利用防止�E�E
        if self.seq >= self.__maxseq {
            return Err(Error::Protocol("aead sequence exhausted".into()));
        }
        if plaintext.len() > Self::MAX_PLAINTEXT_LEN {
            return Err(Error::Protocol("plaintext too long".into()));
        }
        if aad.len() > Self::MAX_AAD_LEN {
            return Err(Error::Protocol("aad too long".into()));
        }
        // Mix direction id into the first 4 byte_s then XOR counter (RFC8439 style)
        let mut base = self.basenonce;
        let dir = self.dir_id.to_be_bytes();
        for i in 0..4 {
            base[i] ^= dir[i];
        }
        let n = AeadNonce(aeadnonce_xor(&base, self.seq));
        // Ultra-high performance: use pre-computed cipher instance
        let cipher = self.get_cipher();
        let ct = cipher.seal(n, aad, plaintext)?;
        let used = self.seq;
        self.seq = self.seq.saturating_add(1);
        // タグも含む暗号斁E��を加算（おおよそ�E上限としてDoS耐性に寁E��！E
        self._bytes_sent = self._bytes_sent.saturating_add(ct.len() as u64);
        Ok((used, ct))
    }

    /// Decrypt at a given sequence number (reordering/retransmit handled by caller)
    pub fn open_at(&self, seq: u64, aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
        if aad.len() > Self::MAX_AAD_LEN {
            return Err(Error::Protocol("aad too long".into()));
        }
        if ciphertext.len() > (Self::MAX_PLAINTEXT_LEN + Self::MAX_TAG_OVERHEAD) {
            return Err(Error::Protocol("ciphertext too long".into()));
        }
        if ciphertext.len() < Self::MAX_TAG_OVERHEAD {
            return Err(Error::Protocol("ciphertext too short".into()));
        }
        let mut base = self.basenonce;
        let dir = self.dir_id.to_be_bytes();
        for i in 0..4 {
            base[i] ^= dir[i];
        }
        let n = AeadNonce(aeadnonce_xor(&base, seq));
        // Ultra-high performance: use pre-computed cipher instance
        let cipher = self.get_cipher();
        cipher.open(n, aad, ciphertext)
    }

    /// Export key material for additional cryptographic operation_s
    /// Use_s HKDF to derive key_s from the current session key
    pub fn export_key_material(&self, context: &[u8], length: usize) -> Result<Vec<u8>> {
        use hkdf::Hkdf;
        use sha2::Sha256;

        // Use current session key as IKM for HKDF
        let hk = Hkdf::<Sha256>::new(None, &self.__key.0);
        let mut output = vec![0u8; length];

        // Create info by combining context with session meta_data
        let mut info = Vec::new();
        info.extend_from_slice(b"nyx-session-export-v1:");
        info.extend_from_slice(context);
        info.extend_from_slice(&self.seq.to_be_bytes());
        info.extend_from_slice(&self.dir_id.to_be_bytes());

        hk.expand(&info, &mut output)
            .map_err(|_| Error::Protocol("Key material export failed".into()))?;

        Ok(output)
    }
}

impl Drop for AeadSession {
    fn drop(&mut self) {
        // Ultra-high performance: clear cipher instance first
        self.cipher = OnceLock::new();
        // Zeroize base nonce and key explicitly (AeadKey also zeroize_s)
        self.basenonce.fill(0);
        // Explicitly zeroize key (safe even if AeadKey Drop also doe_s it)
        self.__key.0.fill(0);
        // Reset counter_s (not strictly sensitive but good hygiene)
        self.seq = 0;
        self.__maxseq = 0;
        self.__rekey_interval = 0;
        self._bytes_sent = 0;
        self._rekey_bytes_interval = 0;
        self.dir_id = 0;
    }
}

impl core::fmt::Debug for AeadSession {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AeadSession")
            .field("suite", &"ChaCha20Poly1305")
            .field("seq", &self.seq)
            .field("maxseq", &self.__maxseq)
            .field("rekey_interval", &self.__rekey_interval)
            .field("bytes_sent", &self._bytes_sent)
            .field("rekey_bytes_interval", &self._rekey_bytes_interval)
            .field("dir_id", &self.dir_id)
            .finish()
    }
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn seq_increments_and_refuses_after_max() -> core::result::Result<(), Box<dyn std::error::Error>>
    {
        let key = AeadKey([9u8; 32]);
        let base = [0u8; 12];
        let mut ses_s = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base)
            .with_maxseq(2)
            .withdirection_id(1);
        let aad = b"aad";
        let (s0, c0) = ses_s.sealnext(aad, b"m0")?;
        assert_eq!(s0, 0);
        let (s1, c1) = ses_s.sealnext(aad, b"m1")?;
        assert_eq!(s1, 1);
        assert!(ses_s.sealnext(aad, b"m3").is_err());
        assert_eq!(ses_s.open_at(s0, aad, &c0).unwrap(), b"m0");
        assert_eq!(ses_s.open_at(s1, aad, &c1).unwrap(), b"m1");
        Ok(())
    }

    #[test]
    fn open_fails_with_wrongseq() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let key = AeadKey([1u8; 32]);
        let base = [0u8; 12];
        let mut ses_s =
            AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).withdirection_id(7);
        let aad = b"aad";
        let (s, c) = ses_s.sealnext(aad, b"m")?;
        assert_eq!(s, 0);
        assert!(ses_s.open_at(1, aad, &c).is_err());
        Ok(())
    }

    #[test]
    fn refuses_too_long_input_s() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let key = AeadKey([2u8; 32]);
        let base = [0u8; 12];
        let mut ses_s =
            AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).withdirection_id(3);
        let long_pt = vec![0u8; (AeadSession::MAX_PLAINTEXT_LEN + 1) as usize];
        assert!(ses_s.sealnext(b"ok", &long_pt).is_err());
        let long_aad = vec![0u8; (AeadSession::MAX_AAD_LEN + 1) as usize];
        assert!(ses_s.sealnext(&long_aad, b"ok").is_err());
        Ok(())
    }

    #[test]
    fn open_rejects_too_short_or_too_longciphertext(
    ) -> core::result::Result<(), Box<dyn std::error::Error>> {
        let key = AeadKey([4u8; 32]);
        let base = [9u8; 12];
        let ses_s =
            AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).withdirection_id(0xAABBCCDD);
        assert!(ses_s.open_at(0, b"", &[0u8; 15]).is_err());
        let big = vec![0u8; AeadSession::MAX_PLAINTEXT_LEN + AeadSession::MAX_TAG_OVERHEAD + 1];
        assert!(ses_s.open_at(0, b"", &big).is_err());
        Ok(())
    }

    #[test]
    fn rekey_resetssequence_and_key_s() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let key = AeadKey([7u8; 32]);
        let base = [3u8; 12];
        let mut tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base)
            .with_rekey_interval(2)
            .withdirection_id(1);
        let rx_old = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([7u8; 32]), base)
            .withdirection_id(1);
        let (_, c0) = tx.sealnext(b"aad", b"m0")?;
        let (_, c1) = tx.sealnext(b"aad", b"m1")?;
        assert!(tx.needs_rekey());
        assert_eq!(rx_old.open_at(0, b"aad", &c0).unwrap(), b"m0");
        assert_eq!(rx_old.open_at(1, b"aad", &c1).unwrap(), b"m1");
        tx.rekey();
        assert_eq!(tx.seq(), 0);
        let (s2, c2) = tx.sealnext(b"aad", b"m2")?;
        assert_eq!(s2, 0);
        let pt2 = tx.open_at(0, b"aad", &c2)?;
        assert_eq!(pt2, b"m2");
        assert!(rx_old.open_at(0, b"aad", &c2).is_err());
        Ok(())
    }

    #[test]
    fn rekey_both_sides_compat() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let init_key = AeadKey([11u8; 32]);
        let base = [5u8; 12];
        let mut tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, init_key, base)
            .with_rekey_interval(1)
            .withdirection_id(2);
        let mut rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([11u8; 32]), base)
            .with_rekey_interval(1)
            .withdirection_id(2);
        let (s0, c0) = tx.sealnext(b"aad", b"hello")?;
        assert_eq!(rx.open_at(s0, b"aad", &c0).unwrap(), b"hello");
        assert!(tx.needs_rekey());
        tx.rekey();
        rx.rekey();
        let (s1, c1) = tx.sealnext(b"aad", b"world")?;
        assert_eq!(s1, 0);
        assert_eq!(rx.open_at(0, b"aad", &c1).unwrap(), b"world");
        Ok(())
    }

    #[test]
    fn differentdirection_id_fails_decrypt() -> core::result::Result<(), Box<dyn std::error::Error>>
    {
        let key = AeadKey([33u8; 32]);
        let base = [1u8; 12];
        let mut a = AeadSession::new(AeadSuite::ChaCha20Poly1305, key.clone(), base)
            .withdirection_id(0x11111111);
        let b =
            AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).withdirection_id(0x22222222);
        let (s, c) = a.sealnext(b"aad", b"msg")?;
        assert!(b.open_at(s, b"aad", &c).is_err());
        Ok(())
    }

    #[test]
    fn rekey_by_bytes_threshold() -> core::result::Result<(), Box<dyn std::error::Error>> {
        let key = AeadKey([22u8; 32]);
        let base = [7u8; 12];
        let mut tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base)
            .with_rekey_interval(u64::MAX)
            .with_rekey_bytes_interval(20);
        assert!(!tx.needs_rekey());
        let (_s0, _c0) = tx.sealnext(b"a", b"hello")?;
        assert!(tx.needs_rekey());
        tx.rekey();
        assert_eq!(tx.seq(), 0);
        assert!(!tx.needs_rekey());
        let _ = tx.sealnext(b"a", b"x")?;
        assert!(!tx.needs_rekey());
        Ok(())
    }
}
