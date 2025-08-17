#![forbid(unsafe_code)]

use crate::aead::{AeadCipher, AeadKey, AeadNonce, AeadSuite};
use crate::kdf::{aead_nonce_xor, hkdf_expand};
use crate::{Error, Result};

/// AEAD session (unidirectional): derive per-record nonce from base nonce + sequence,
/// with optional 32-bit direction identifier mixed to avoid overlap across directions.
pub struct AeadSession {
    suite: AeadSuite,
    key: AeadKey,
    base_nonce: [u8; 12],
    seq: u64,
    max_seq: u64,
    rekey_interval: u64,
    // Total ciphertext bytes sent for byte-threshold rekey (0 disables threshold)
    bytes_sent: u64,
    rekey_bytes_interval: u64,
    // 32-bit direction identifier to be XORed into the first 4 bytes of the nonce
    dir_id: u32,
}

impl AeadSession {
    /// Defensive limits (plaintext/AAD)
    const MAX_PLAINTEXT_LEN: usize = 1024 * 1024; // 1 MiB
    const MAX_AAD_LEN: usize = 16 * 1024; // 16 KiB
    const MAX_TAG_OVERHEAD: usize = 16; // Tag length for ChaCha20-Poly1305/AES-GCM
    pub fn new(suite: AeadSuite, key: AeadKey, base_nonce: [u8; 12]) -> Self {
        Self {
            suite,
            key,
            base_nonce,
            seq: 0,
            max_seq: u64::MAX,
            rekey_interval: 1 << 20,
            bytes_sent: 0,
            rekey_bytes_interval: 0,
            dir_id: 0,
        }
    }

    /// Set an explicit upper bound for sequence (DoS avoidance, key update policy)
    pub fn with_max_seq(mut self, max_seq: u64) -> Self { self.max_seq = max_seq; self }

    /// Set rekey interval by record count (default: 2^20 records)
    pub fn with_rekey_interval(mut self, interval: u64) -> Self {
        self.rekey_interval = interval.max(1);
        self
    }

    /// Set rekey threshold by bytes (0 disables)
    pub fn with_rekey_bytes_interval(mut self, bytes: u64) -> Self {
        self.rekey_bytes_interval = bytes; // 0は無効として扱う
        self
    }

    /// Set 32-bit direction identifier to be mixed into nonce (first 4 bytes XOR)
    pub fn with_direction_id(mut self, dir_id: u32) -> Self {
        self.dir_id = dir_id;
        self
    }

    /// Current sequence number (used for the next send)
    pub fn seq(&self) -> u64 { self.seq }

    /// Whether rekey criteria by records/bytes are met
    pub fn needs_rekey(&self) -> bool {
        if self.seq >= self.rekey_interval { return true; }
        if self.rekey_bytes_interval > 0 && self.bytes_sent >= self.rekey_bytes_interval { return true; }
        false
    }

    /// Rekey using HKDF; refresh key and base nonce, reset counters
    pub fn rekey(&mut self) {
        let old_key = self.key.0; // copy
        // 新しい鍵
        let mut new_key = [0u8;32];
        hkdf_expand(&old_key, b"nyx/aead/rekey/v1", &mut new_key);
        // 新しいベースノンス
        let mut new_nonce = [0u8;12];
        hkdf_expand(&old_key, b"nyx/aead/rekey/nonce/v1", &mut new_nonce);
    // 置換
        self.key = AeadKey(new_key);
        self.base_nonce = new_nonce;
        self.seq = 0;
    self.bytes_sent = 0;
    }

    /// Encrypt next packet. Returns (sequence, ciphertext). Enforces limits.
    pub fn seal_next(&mut self, aad: &[u8], plaintext: &[u8]) -> Result<(u64, Vec<u8>)> {
        // seq が max_seq に到達したら以降の送信を拒否（nonce再利用防止）
        if self.seq >= self.max_seq { return Err(Error::Protocol("aead sequence exhausted".into())); }
        if plaintext.len() > Self::MAX_PLAINTEXT_LEN { return Err(Error::Protocol("plaintext too long".into())); }
        if aad.len() > Self::MAX_AAD_LEN { return Err(Error::Protocol("aad too long".into())); }
        // Mix direction id into the first 4 bytes then XOR counter (RFC8439 style)
        let mut base = self.base_nonce;
        let dir = self.dir_id.to_be_bytes();
        for i in 0..4 { base[i] ^= dir[i]; }
        let n = AeadNonce(aead_nonce_xor(&base, self.seq));
    let cipher = AeadCipher::new(self.suite, AeadKey(self.key.0));
    let ct = cipher.seal(n, aad, plaintext)?;
    let used = self.seq;
        self.seq = self.seq.saturating_add(1);
    // タグも含む暗号文長を加算（おおよその上限としてDoS耐性に寄与）
    self.bytes_sent = self.bytes_sent.saturating_add(ct.len() as u64);
        Ok((used, ct))
    }

    /// Decrypt at a given sequence number (reordering/retransmit handled by caller)
    pub fn open_at(&self, seq: u64, aad: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if aad.len() > Self::MAX_AAD_LEN { return Err(Error::Protocol("aad too long".into())); }
    if ciphertext.len() > (Self::MAX_PLAINTEXT_LEN + Self::MAX_TAG_OVERHEAD) { return Err(Error::Protocol("ciphertext too long".into())); }
    if ciphertext.len() < Self::MAX_TAG_OVERHEAD { return Err(Error::Protocol("ciphertext too short".into())); }
        let mut base = self.base_nonce;
        let dir = self.dir_id.to_be_bytes();
        for i in 0..4 { base[i] ^= dir[i]; }
        let n = AeadNonce(aead_nonce_xor(&base, seq));
    let cipher = AeadCipher::new(self.suite, AeadKey(self.key.0));
    cipher.open(n, aad, ciphertext)
    }
}

impl Drop for AeadSession {
    fn drop(&mut self) {
        // Zeroize base nonce and key explicitly (AeadKey also zeroizes)
        self.base_nonce.fill(0);
    // Explicitly zeroize key (safe even if AeadKey Drop also does it)
    self.key.0.fill(0);
        // Reset counters (not strictly sensitive but good hygiene)
        self.seq = 0;
        self.max_seq = 0;
    self.rekey_interval = 0;
        self.bytes_sent = 0;
        self.rekey_bytes_interval = 0;
        self.dir_id = 0;
    }
}

impl core::fmt::Debug for AeadSession {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AeadSession")
            .field("suite", &"ChaCha20Poly1305")
            .field("seq", &self.seq)
            .field("max_seq", &self.max_seq)
            .field("rekey_interval", &self.rekey_interval)
            .field("bytes_sent", &self.bytes_sent)
            .field("rekey_bytes_interval", &self.rekey_bytes_interval)
            .field("dir_id", &self.dir_id)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seq_increments_and_refuses_after_max() {
        let key = AeadKey([9u8;32]);
        let base = [0u8;12];
    let mut sess = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).with_max_seq(2).with_direction_id(1);
        let aad = b"aad";
    let (s0, c0) = sess.seal_next(aad, b"m0").unwrap();
        assert_eq!(s0, 0);
    let (s1, c1) = sess.seal_next(aad, b"m1").unwrap();
        assert_eq!(s1, 1);
    // 次は上限到達で拒否（seq==max_seq でエラー）
        assert!(sess.seal_next(aad, b"m3").is_err());
        // 復号検証
        assert_eq!(sess.open_at(s0, aad, &c0).unwrap(), b"m0");
        assert_eq!(sess.open_at(s1, aad, &c1).unwrap(), b"m1");
    // 2つ送信のみ成功
    }

    #[test]
    fn open_fails_with_wrong_seq() {
        let key = AeadKey([1u8;32]);
        let base = [0u8;12];
    let mut sess = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).with_direction_id(7);
        let aad = b"aad";
        let (s, c) = sess.seal_next(aad, b"m").unwrap();
        assert_eq!(s, 0);
        // 異なる seq での復号は失敗
        assert!(sess.open_at(1, aad, &c).is_err());
    }

    #[test]
    fn refuses_too_long_inputs() {
        let key = AeadKey([2u8;32]);
        let base = [0u8;12];
    let mut sess = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).with_direction_id(3);
        let long_pt = vec![0u8; (AeadSession::MAX_PLAINTEXT_LEN + 1) as usize];
        assert!(sess.seal_next(b"ok", &long_pt).is_err());
        let long_aad = vec![0u8; (AeadSession::MAX_AAD_LEN + 1) as usize];
        assert!(sess.seal_next(&long_aad, b"ok").is_err());
    }

    #[test]
    fn open_rejects_too_short_or_too_long_ciphertext() {
        let key = AeadKey([4u8;32]);
        let base = [9u8;12];
    let sess = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).with_direction_id(0xAABBCCDD);
        // too short (< tag)
        assert!(sess.open_at(0, b"", &[0u8; 15]).is_err());
        // too long (> pt max + tag)
        let big = vec![0u8; AeadSession::MAX_PLAINTEXT_LEN + AeadSession::MAX_TAG_OVERHEAD + 1];
        assert!(sess.open_at(0, b"", &big).is_err());
    }

    #[test]
    fn rekey_resets_sequence_and_keys() {
        let key = AeadKey([7u8;32]);
        let base = [3u8;12];
        let mut tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).with_rekey_interval(2).with_direction_id(1);
    // Receiver before rekey (simulating old state)
    let rx_old = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([7u8;32]), base).with_direction_id(1);
        // 2つ送ってrekey条件に到達
        let (_, c0) = tx.seal_next(b"aad", b"m0").unwrap();
        let (_, c1) = tx.seal_next(b"aad", b"m1").unwrap();
        assert!(tx.needs_rekey());
    // rekey前の受信側で旧CTは復号できる
    assert_eq!(rx_old.open_at(0, b"aad", &c0).unwrap(), b"m0");
    assert_eq!(rx_old.open_at(1, b"aad", &c1).unwrap(), b"m1");
        // rekey実施
        tx.rekey();
        assert_eq!(tx.seq(), 0);
        // 新しい鍵/ノンスで暗号化したものは同一セッションでは復号可能
        let (s2, c2) = tx.seal_next(b"aad", b"m2").unwrap();
        assert_eq!(s2, 0);
        let pt2 = tx.open_at(0, b"aad", &c2).unwrap();
        assert_eq!(pt2, b"m2");
    // 旧受信側では新しいCTは復号できない
    assert!(rx_old.open_at(0, b"aad", &c2).is_err());
    }

    #[test]
    fn rekey_both_sides_compat() {
        let init_key = AeadKey([11u8;32]);
        let base = [5u8;12];
        let mut tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, init_key, base).with_rekey_interval(1).with_direction_id(2);
        let mut rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([11u8;32]), base).with_rekey_interval(1).with_direction_id(2);
        // 1レコードでrekey閾値到達
        let (s0, c0) = tx.seal_next(b"aad", b"hello").unwrap();
        assert_eq!(rx.open_at(s0, b"aad", &c0).unwrap(), b"hello");
    assert!(tx.needs_rekey());
        // 両端rekey
        tx.rekey(); rx.rekey();
        // 次の送信はseq=0で新キー
        let (s1, c1) = tx.seal_next(b"aad", b"world").unwrap();
        assert_eq!(s1, 0);
        assert_eq!(rx.open_at(0, b"aad", &c1).unwrap(), b"world");
    }

    #[test]
    fn different_direction_id_fails_decrypt() {
        // Same key/base/seq but different direction IDs must not decrypt each other
        let key = AeadKey([33u8;32]);
        let base = [1u8;12];
        let mut a = AeadSession::new(AeadSuite::ChaCha20Poly1305, key.clone(), base).with_direction_id(0x11111111);
        let b = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base).with_direction_id(0x22222222);
        let (s, c) = a.seal_next(b"aad", b"msg").unwrap();
        assert!(b.open_at(s, b"aad", &c).is_err());
    }

    #[test]
    fn rekey_by_bytes_threshold() {
        let key = AeadKey([22u8;32]);
        let base = [7u8;12];
        // しきい値を小さく設定して動作確認（タグ16B + 本文5B 程度で超える）
        let mut tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base)
            .with_rekey_interval(u64::MAX) // レコード数では打たない
            .with_rekey_bytes_interval(20);
        assert!(!tx.needs_rekey());
    let (_s0, _c0) = tx.seal_next(b"a", b"hello").unwrap();
        // 暗号文長を加算後、しきい値超えを想定
        assert!(tx.needs_rekey());
        // rekeyでカウンタがリセットされる
        tx.rekey();
        assert_eq!(tx.seq(), 0);
        assert!(!tx.needs_rekey());
        // 送信しても直後はまだ未到達のはず
        let _ = tx.seal_next(b"a", b"x").unwrap();
        assert!(!tx.needs_rekey());
    }
}
