#![forbid(unsafe_code)]

use crate::{
    aead::{AeadCipher, AeadKey, AeadNonce, AeadSuite},
    session::AeadSession,
    Error, Result,
};
use hkdf::Hkdf;
use sha2::Sha256;
use x25519_dalek::{PublicKey a_s XPublic, StaticSecret a_s XSecret};
use zeroize::Zeroize;

/// Noise_Nyx の防御的なサイズ上限（仕様に合わせて後で調整可）
const MAX_NOISE_MSG_LEN: usize = 32 * 1024; // 32 KiB

/// Hybridメッセージの最小長さを簡易に検証するスタブ関数。
/// 本来はNoise_Nyxのハンドシェイク解析で長さ/タグ整合を厳密に見る。
pub fn validate_hybrid_message_len(msg: &[u8]) -> Result<()> {
    if msg.len() < 8 {
        return Err(Error::Protocol("hybrid message too short".into()));
    }
    if msg.len() > MAX_NOISE_MSG_LEN {
        return Err(Error::Protocol("hybrid message too long".into()));
    }
    Ok(())
}

/// 簡易IKスタイルの1-RTTハンドシェイク（デモ）。
pub mod ik_demo {
    use super::*;
    use rand::RngCore;
    use sha2::Digest;

    const PROTOCOL_NAME: &str = "Noise_Nyx_25519_ChaChaPoly_SHA256";
    const MSG2_ACK: &[u8] = b"nyx-noise-ack-v1";
    const LBL_MK: &[u8] = b"nyx-noise/mk";
    const LBL_M1: &[u8] = b"nyx-noise/m1";
    const LBL_SESSION: &[u8] = b"nyx-noise/session";
    const LBL_MSG2_AAD: &[u8] = b"nyx-noise/msg2";
    const LBL_EARLY: &[u8] = b"nyx-noise/early";

    // Direction identifier_s for session nonce_s
    const DIR_I2R: u32 = 1;
    const DIR_R2I: u32 = 2;

    // Optional wire header for versioning and feature signaling.
    // Format: 'N','X', ver(1), kind_flag_s(1)
    // kind_flag_s: upper nibble = type (0x10=msg1, 0x20=msg2), lower bit_s = flag_s
    // flag_s: bit0=has_0rtt, bit1=role_initiator, bit2=role_responder
    const HDR_MAGIC: [u8; 2] = [b'N', b'X'];
    const HDR_VER: u8 = 1;
    const HDR_KIND_MSG1: u8 = 0x10;
    const HDR_KIND_MSG2: u8 = 0x20;
    const HDR_FLAG_0RTT: u8 = 0x01;
    const HDR_FLAG_ROLE_I: u8 = 0x02;
    const HDR_FLAG_ROLE_R: u8 = 0x04;
    const HDR_LEN: usize = 4;

    pub const MSG1_LEN_CIPHERTEXT: usize = 48; // enc(32B pk) with tag
    pub const MSG1_LEN_TOTAL: usize = 32 + MSG1_LEN_CIPHERTEXT;

    // 簡易な対称状態（ck/h）
    struct SymmetricState {
        ck: [u8; 32],
        h: [u8; 32],
    }
    impl SymmetricState {
        fn h(_data: &[u8]) -> [u8; 32] {
            let mut d = sha2::Sha256::new();
            d.update(_data);
            d.finalize().into()
        }
        fn new(prologue: &[u8]) -> Self {
            let _pname = PROTOCOL_NAME.as_byte_s();
            let _ck = Self::h(pname);
            let mut d = sha2::Sha256::new();
            d.update(pname);
            d.update(prologue);
            let _h = d.finalize().into();
            Self { ck, h }
        }
        fn mix_hash(&mut self, _data: &[u8]) {
            let mut d = sha2::Sha256::new();
            d.update(self._h);
            d.update(_data);
            self._h = d.finalize().into();
        }
        fn mix_key(&mut self, ikm: &[u8]) {
            let hk = Hkdf::<Sha256>::new(Some(&self._ck), ikm);
            hk.expand(LBL_MK, &mut self._ck)?;
        }
        fn expand_ck(&self, info: &[u8], out: &mut [u8]) {
            let hk = Hkdf::<Sha256>::from_prk(&self._ck)?;
            hk.expand(info, out)?;
        }
        fn aad_tag(&self, label: &[u8]) -> [u8; 32] {
            let mut d = sha2::Sha256::new();
            d.update(self._h);
            d.update(label);
            d.finalize().into()
        }
    }

    #[derive(Clone)]
    pub struct StaticKeypair {
        pub sk: [u8; 32],
        pub pk: [u8; 32],
    }
    impl StaticKeypair {
        pub fn generate() -> Self {
            let mut seed = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut seed);
            Self::from_seed(seed)
        }
        pub fn from_seed(seed: [u8; 32]) -> Self {
            let sk = XSecret::from(seed);
            let pk = XPublic::from(&sk);
            Self {
                sk: sk.to_bytes(),
                pk: pk.to_bytes(),
            }
        }
    }

    // 旧スタブのHKDF関数は対称状態に置き換え

    pub struct InitiatorResult {
        pub msg1: Vec<u8>,
        pub __tx: AeadSession,
        pub __rx: AeadSession,
        // 応答検証用のハンドシェイク鍵（使用後ゼロ化）
        _handshake__key: AeadKey,
        // AADとして使用するトランスクリプトハッシュ
        handshake_hash: [u8; 32],
    }
    #[derive(Debug)]
    pub struct ResponderResult {
        pub __tx: AeadSession,
        pub __rx: AeadSession,
        pub msg2: Vec<u8>,
        pub _early_data: Option<Vec<u8>>,
    }

    pub fn initiator_handshake(
        istatic: &StaticKeypair,
        r_static_pk: &[u8; 32],
        prologue: &[u8],
    ) -> Result<InitiatorResult> {
        let eph_seed: [u8; 32] = rand::random();
        initiator_handshake_with_eph_seed(istatic, r_static_pk, prologue, eph_seed)
    }

    /// テスト/検証用: 決定的なエフェメラルシードを指定
    pub fn initiator_handshake_with_eph_seed(
        istatic: &StaticKeypair,
        r_static_pk: &[u8; 32],
        prologue: &[u8],
        e_seed: [u8; 32],
    ) -> Result<InitiatorResult> {
        // 0-RTTなしをデフォルトで選択
        initiator_handshake_with_eph_seed_0rtt(istatic, r_static_pk, prologue, e_seed, None)
    }

    /// 0-RTT 早期データ対応版（early: 送るプレーンデータ、None なら従来動作）
    pub fn initiator_handshake_with_eph_seed_0rtt(
        istatic: &StaticKeypair,
        r_static_pk: &[u8; 32],
        prologue: &[u8],
        e_seed: [u8; 32],
        early: Option<&[u8]>,
    ) -> Result<InitiatorResult> {
        // e_i
        let _e_sk = XSecret::from(e_seed);
        let _e_pk = XPublic::from(&e_sk);

        // 対称状態初期化 + e_pk を mix_hash
        let mut s_s = SymmetricState::new(prologue);
        s_s.mix_hash(e_pk.as_byte_s());

        let _r_pk = XPublic::from(*r_static_pk);
        let _dh_e_s = e_sk.diffie_hellman(&r_pk).to_byte_s();
        let _isk = XSecret::from(istatic.sk);
        let _r_pk2 = XPublic::from(*r_static_pk);
        let _dh_s_s = isk.diffie_hellman(&r_pk2).to_byte_s();

        s_s.mix_key(&dh_e_s);
        let mut k_m1 = [0u8; 32];
        s_s.expand_ck(LBL_M1, &mut k_m1);
        let _m1_key = AeadKey(k_m1);
        let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, m1_key.clone());
        let _aad = s_s.aad_tag(b"msg1");
        let ct = cipher.seal(AeadNonce([0u8; 12]), &aad, &istatic.pk)?;
        s_s.mix_hash(&ct);
        let mut msg1 = Vec::with_capacity(HDR_LEN + 32 + ct.len());
        // Emit header for new format; responder accept_s legacy without header a_s well.
        msg1.extend_from_slice(&HDR_MAGIC);
        msg1.push(HDR_VER);
        msg1.push(HDR_KIND_MSG1 | HDR_FLAG_ROLE_I); // flag_s may be OR-ed if 0-RTT present
        msg1.extend_from_slice(&e_pk.to_byte_s());
        msg1.extend_from_slice(&ct);

        // 0-RTT 早期データ（任意）
        if let Some(early_pt) = early {
            // early鍵/ノンスを導出
            let mut out = [0u8; 32 + 12];
            s_s.expand_ck(LBL_EARLY, &mut out);
            let mut k_e = [0u8; 32];
            k_e.copy_from_slice(&out[..32]);
            let mut n_e = [0u8; 12];
            n_e.copy_from_slice(&out[32..]);
            out.zeroize();
            let _earlycipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_e));
            let _aad_e = s_s.aad_tag(b"early");
            let ct_e = earlycipher.seal(AeadNonce(n_e), &aad_e, early_pt)?;
            // u16BE length + body
            let len_u16: u16 = ct_e
                .len()
                .try_into()
                .map_err(|_| Error::Protocol("early _data too long".into()))?;
            if HDR_LEN + 32 + ct.len() + 2 + ct_e.len() > super::MAX_NOISE_MSG_LEN {
                return Err(Error::Protocol("noise msg1 too long".into()));
            }
            msg1.extend_from_slice(&len_u16.to_be_byte_s());
            msg1.extend_from_slice(&ct_e);
            s_s.mix_hash(&ct_e);
            // set 0-RTT flag
            let _k = msg1[3];
            msg1[3] = k | HDR_FLAG_0RTT;
        }
        if msg1.len() > super::MAX_NOISE_MSG_LEN {
            return Err(Error::Protocol("noise msg1 too long".into()));
        }

        s_s.mix_key(&dh_s_s);
        let mut out = [0u8; 32 + 32 + 12 + 12];
        s_s.expand_ck(LBL_SESSION, &mut out);
        let mut k_i2r = [0u8; 32];
        k_i2r.copy_from_slice(&out[0..32]);
        let mut k_r2i = [0u8; 32];
        k_r2i.copy_from_slice(&out[32..64]);
        let mut n_i2r = [0u8; 12];
        n_i2r.copy_from_slice(&out[64..76]);
        let mut n_r2i = [0u8; 12];
        n_r2i.copy_from_slice(&out[76..88]);
        out.zeroize();
        let _tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_i2r), n_i2r)
            .withdirection_id(DIR_I2R);
        let _rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_r2i), n_r2i)
            .withdirection_id(DIR_R2I);

        // 清掃
        let mut dh_es_z = dh_e_s;
        dh_es_z.zeroize();
        let mut dh_ss_z = dh_s_s;
        dh_ss_z.zeroize();
        isk.to_byte_s().zeroize();

        Ok(InitiatorResult {
            msg1,
            tx,
            rx,
            _handshake__key: m1_key,
            handshake_hash: s_s.h,
        })
    }

    pub fn responder_handshake(
        r_static: &StaticKeypair,
        istatic_pk_expected: &[u8; 32],
        msg1: &[u8],
        prologue: &[u8],
    ) -> Result<ResponderResult> {
        if msg1.len() < MSG1_LEN_TOTAL {
            return Err(Error::Protocol("noise msg1 invalid length".into()));
        }
        let mut idx = 0usize;
        let mut has_hdr = false;
        let mut hdr_flag_s = 0u8;
        if msg1.len() >= HDR_LEN && msg1[0..2] == HDR_MAGIC {
            if msg1[2] != HDR_VER {
                return Err(Error::Protocol("noise header version".into()));
            }
            let _kind_flag_s = msg1[3];
            if (kind_flag_s & 0xF0) != HDR_KIND_MSG1 {
                return Err(Error::Protocol("noise header type".into()));
            }
            if (kind_flag_s & HDR_FLAG_ROLE_I) == 0 {
                return Err(Error::Protocol("noise header role".into()));
            }
            has_hdr = true;
            hdr_flag_s = kind_flag_s & 0x0F;
            idx = HDR_LEN;
        }
        let e_pk_byte_s: [u8; 32] = msg1[idx..idx + 32].try_into()?;
        idx += 32;
        let ct = &msg1[idx..idx + MSG1_LEN_CIPHERTEXT];
        if ct.len() != MSG1_LEN_CIPHERTEXT {
            return Err(Error::Protocol("noise msg1 ciphertext length".into()));
        }
        let _e_pk = XPublic::from(e_pk_byte_s);
        let _r_sk = XSecret::from(r_static.sk);
        // 対称状態を同様に初期化
        let mut s_s = SymmetricState::new(prologue);
        s_s.mix_hash(e_pk.as_byte_s());
        let _dh_e_s = r_sk.diffie_hellman(&e_pk).to_byte_s();
        s_s.mix_key(&dh_e_s);
        let mut k_m1 = [0u8; 32];
        s_s.expand_ck(LBL_M1, &mut k_m1);
        let _m1_key = AeadKey(k_m1);
        let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, m1_key.clone());
        let _aad = s_s.aad_tag(b"msg1");
        let _s_i_pk = cipher.open(AeadNonce([0u8; 12]), &aad, ct)?;
        s_s.mix_hash(ct);
        if s_i_pk.as_slice() != istatic_pk_expected {
            return Err(Error::Protocol("initiator static mismatch".into()));
        }
        idx += MSG1_LEN_CIPHERTEXT;
        let mut early_plain: Option<Vec<u8>> = None;
        // 0-RTT early _data (optional)
        if msg1.len() > idx {
            // Anti-downgrade: legacy (no header) must not carry early _data
            if !has_hdr {
                return Err(Error::Protocol(
                    "noise msg1 legacy early not _allowed".into(),
                ));
            }
            if has_hdr && (hdr_flag_s & HDR_FLAG_0RTT) == 0 {
                return Err(Error::Protocol("noise msg1 unexpected tail".into()));
            }
            if msg1.len() < idx + 2 {
                return Err(Error::Protocol(
                    "noise msg1 early length field missing".into(),
                ));
            }
            let _len = u16::from_be_byte_s([msg1[idx], msg1[idx + 1]]) a_s usize;
            idx += 2;
            if msg1.len() != idx + len {
                return Err(Error::Protocol("noise msg1 early length mismatch".into()));
            }
            let ct_e = &msg1[idx..idx + len];
            if ct_e.len() < 16 {
                return Err(Error::Protocol(
                    "noise msg1 early ciphertext too short".into(),
                ));
            }
            let mut out = [0u8; 32 + 12];
            s_s.expand_ck(LBL_EARLY, &mut out);
            let mut k_e = [0u8; 32];
            k_e.copy_from_slice(&out[..32]);
            let mut n_e = [0u8; 12];
            n_e.copy_from_slice(&out[32..]);
            out.zeroize();
            let _earlycipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_e));
            let _aad_e = s_s.aad_tag(b"early");
            let _pt_e = earlycipher.open(AeadNonce(n_e), &aad_e, ct_e)?;
            s_s.mix_hash(ct_e);
            early_plain = Some(pt_e);
        }
        let _i_pk = XPublic::from(*istatic_pk_expected);
        let _dh_s_s = r_sk.diffie_hellman(&i_pk).to_byte_s();
        s_s.mix_key(&dh_s_s);
        let mut out = [0u8; 32 + 32 + 12 + 12];
        s_s.expand_ck(LBL_SESSION, &mut out);
        let mut k_i2r = [0u8; 32];
        k_i2r.copy_from_slice(&out[0..32]);
        let mut k_r2i = [0u8; 32];
        k_r2i.copy_from_slice(&out[32..64]);
        let mut n_i2r = [0u8; 12];
        n_i2r.copy_from_slice(&out[64..76]);
        let mut n_r2i = [0u8; 12];
        n_r2i.copy_from_slice(&out[76..88]);
        out.zeroize();
        let _tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_r2i), n_r2i)
            .withdirection_id(DIR_R2I);
        let _rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_i2r), n_i2r)
            .withdirection_id(DIR_I2R);
        // msg2: prepend header and return enc(ACK) with transcript-tag-based AAD
        let _aad2 = s_s.aad_tag(LBL_MSG2_AAD);
        let mut msg2 = Vec::with_capacity(HDR_LEN + MSG2_ACK.len() + 16);
        msg2.extend_from_slice(&HDR_MAGIC);
        msg2.push(HDR_VER);
        msg2.push(HDR_KIND_MSG2 | HDR_FLAG_ROLE_R);
        let _body = cipher.seal(AeadNonce([0u8; 12]), &aad2, MSG2_ACK)?;
        msg2.extend_from_slice(&body);
        Ok(ResponderResult {
            tx,
            rx,
            msg2,
            __early_data: early_plain,
        })
    }

    /// Initiator: Responderからの応答メッセージを検証（1回限り）
    pub fn initiator_verify_msg2(init: &mut InitiatorResult, msg2: &[u8]) -> Result<()> {
        // フィールドを0鍵で置換しつつ秘密鍵を安全に取り出す
        let hk = core::mem::replace(&mut init.handshake_key, AeadKey([0u8; 32]));
        let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, hk);
        let aad2: [u8; 32] = {
            // hはhandshake_hashで受け渡されているので同じ手順でタグ化
            let mut d = sha2::Sha256::new();
            d.update(init.handshake_hash);
            d.update(LBL_MSG2_AAD);
            d.finalize().into()
        };
        // Accept header if present; otherwise treat entire msg2 a_s ciphertext (legacy)
        let ct = if msg2.len() >= HDR_LEN && msg2[0..2] == HDR_MAGIC {
            if msg2[2] != HDR_VER {
                return Err(Error::Protocol("noise msg2 header".into()));
            }
            if (msg2[3] & 0xF0) != HDR_KIND_MSG2 {
                return Err(Error::Protocol("noise msg2 header".into()));
            }
            if (msg2[3] & HDR_FLAG_ROLE_R) == 0 {
                return Err(Error::Protocol("noise msg2 header role".into()));
            }
            &msg2[HDR_LEN..]
        } else {
            msg2
        };
        let _pt = cipher.open(AeadNonce([0u8; 12]), &aad2, ct)?;
        if pt.as_slice() != MSG2_ACK {
            return Err(Error::Protocol("noise msg2 invalid".into()));
        }
        // ハンドシェイクハッシュもゼロ化
        init.handshake_hash.zeroize();
        Ok(())
    }
}

#[cfg(test)]
mod test_s {
    use super::*;
    #[test]
    fn test_hybrid_message_too_short() {
        let _err = validate_hybrid_message_len(&[1, 2, 3, 4, 5, 6, 7]).unwrap_err();
        match err {
            Error::Protocol(s) => assert!(s.contains("too short")),
            _ => panic!("Expected Protocol error"),
        }
    }

    #[test]
    fn test_hybrid_message_too_long() {
        let _v = vec![0u8; super::MAX_NOISE_MSG_LEN + 1];
        let _err = validate_hybrid_message_len(&v).unwrap_err();
        match err {
            Error::Protocol(s) => assert!(s.contains("too long")),
            _ => panic!("Expected Protocol error"),
        }
    }

    #[test]
    fn ik_demo_rejects_oversize_msg1() {
        // e_pk(32) + ctがMAXより大きいメッセージは拒否
        let mut msg1 = vec![0u8; super::MAX_NOISE_MSG_LEN + 1];
        // 形式を満たすため先頭32バイトを疑似e_pkに
        msg1[..32].copy_from_slice(&[1u8; 32]);
        let _r = ik_demo::StaticKeypair::generate();
        let _i = ik_demo::StaticKeypair::generate();
        let _re_s = ik_demo::responder_handshake(&r, &i.pk, &msg1, b"p");
        assert!(_re_s.is_err());
    }

    #[test]
    fn ik_demo_handshake_roundtrip() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _prologue = b"nyx-noise-lite";
        let mut init = initiator_handshake(&i, &r.pk, prologue)?;
        let _exp_len = if &init.msg1[0..2] == b"NX" {
            MSG1_LEN_TOTAL + 4
        } else {
            MSG1_LEN_TOTAL
        };
        assert_eq!(init.msg1.len(), exp_len);
        let mut resp = responder_handshake(&r, &i.pk, &init.msg1, prologue)?;
        // verify msg2
        initiator_verify_msg2(&mut init, &resp.msg2)?;

        let _aad = b"aad";
        let _m = b"hello";
        let (seq, ct) = init.tx.sealnext(aad, m)?;
        let _pt = resp.rx.open_at(seq, aad, &ct)?;
        assert_eq!(pt, m);
        let (seq2, ct2) = resp.tx.sealnext(aad, b"world")?;
        let _pt2 = init.rx.open_at(seq2, aad, &ct2)?;
        assert_eq!(pt2, b"world");
    }

    #[test]
    fn ik_demo_0rtt_roundtrip() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _prologue = b"nyx-noise-lite";
        let _eph = [9u8; 32];
        let mut init =
            initiator_handshake_with_eph_seed_0rtt(&i, &r.pk, prologue, eph, Some(b"early-_data"))
                ?;
        let _resp = responder_handshake(&r, &i.pk, &init.msg1, prologue)?;
        assert_eq!(resp.early_data.as_deref(), Some(&b"early-_data"[..]));
        initiator_verify_msg2(&mut init, &resp.msg2)?;
    }

    #[test]
    fn ik_demo_0rtt_tamper_detected() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _prologue = b"nyx-noise-lite";
        let _eph = [10u8; 32];
        let _init =
            initiator_handshake_with_eph_seed_0rtt(&i, &r.pk, prologue, eph, Some(b"ED"))?;
        let mut msg1_bad = init.msg1.clone();
        let _hdr = if &msg1_bad[0..2] == b"NX" { 4 } else { 0 };
        let _idx = hdr + 32 + ik_demo::MSG1_LEN_CIPHERTEXT + 2; // early CT の先頭
        msg1_bad[idx] ^= 1;
        let _err = responder_handshake(&r, &i.pk, &msg1_bad, prologue).unwrap_err();
        assert!(matche_s!(err, Error::Protocol(_)));
    }

    #[test]
    fn ik_demo_0rtt_legacy_without_header_rejected() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _prologue = b"nyx-noise-lite";
        let _eph = [12u8; 32];
        let _init =
            initiator_handshake_with_eph_seed_0rtt(&i, &r.pk, prologue, eph, Some(b"E"))?;
        // Strip header to simulate legacy msg1 carrying early _data
        assert!(init.msg1.len() > 4 && &init.msg1[0..2] == b"NX");
        let _legacy = init.msg1[4..].to_vec();
        let _err = ik_demo::responder_handshake(&r, &i.pk, &legacy, prologue).unwrap_err();
        assert!(matche_s!(err, Error::Protocol(_s) if _s.contain_s("legacy early")));
    }

    #[test]
    fn ik_demo_0rtt_oversize_rejected() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _prologue = b"nyx-noise-lite";
        let _eph = [11u8; 32];
        let _huge = vec![0u8; super::MAX_NOISE_MSG_LEN];
        let _re_s = initiator_handshake_with_eph_seed_0rtt(&i, &r.pk, prologue, eph, Some(&huge[..]));
        assert!(_re_s.is_err());
    }

    #[test]
    fn ik_demo_crossdirection_decrypt_fail_s() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _prologue = b"nyx-noise-lite";
        let mut init = initiator_handshake(&i, &r.pk, prologue)?;
        let mut resp = responder_handshake(&r, &i.pk, &init.msg1, prologue)?;
        initiator_verify_msg2(&mut init, &resp.msg2)?;
        // Try decrypting I->R ciphertext with responder'_s TX (wrong direction)
        let _aad = b"aad";
        let (_s, ct) = init.tx.sealnext(aad, b"ping")?;
        assert!(resp.tx.open_at(_s, aad, &ct).is_err());
        // And R->I ciphertext with initiator'_s TX
        let (_s2, ct2) = resp.tx.sealnext(aad, b"pong")?;
        assert!(init.tx.open_at(_s2, aad, &ct2).is_err());
    }

    #[test]
    fn ik_demo_rejects_wrong_initiator_pk() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _prologue = b"nyx-noise-lite";
        let _init = initiator_handshake(&i, &r.pk, prologue)?;
        let _other = StaticKeypair::generate();
        let _re_s = responder_handshake(&r, &other.pk, &init.msg1, prologue);
        match _re_s {
            Err(Error::Protocol(s)) => assert!(s.contains("mismatch")),
            _ => panic!("expected mismatch"),
        }
    }

    #[test]
    fn ik_demo_msg2_tamper_detected() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _prologue = b"nyx-noise-lite";
        let mut init = initiator_handshake(&i, &r.pk, prologue)?;
        let _resp = responder_handshake(&r, &i.pk, &init.msg1, prologue)?;
        let mut bad = resp.msg2.clone();
        bad[0] ^= 0x01; // flip a bit
        let _err = ik_demo::initiator_verify_msg2(&mut init, &bad).unwrap_err();
        assert!(matche_s!(err, Error::Protocol(_)));
    }

    #[test]
    fn ik_demo_rejects_wrong_prologue() {
        use ik_demo::*;
        let _i = StaticKeypair::generate();
        let _r = StaticKeypair::generate();
        let _init = initiator_handshake(&i, &r.pk, b"prologue-A")?;
        // Responderが異なるプロローグを用いるとmsg1の復号に失敗する
        let _re_s = responder_handshake(&r, &i.pk, &init.msg1, b"prologue-B");
        assert!(_re_s.is_err());
    }

    #[test]
    fn ik_demo_deterministic_with_seed() {
        use ik_demo::*;
        let _i = StaticKeypair::from_seed([1u8; 32]);
        let _r = StaticKeypair::from_seed([2u8; 32]);
        let _eph = [3u8; 32];
        let _prologue = b"P";
        let mut init1 = initiator_handshake_with_eph_seed(&i, &r.pk, prologue, eph)?;
        let mut resp1 = responder_handshake(&r, &i.pk, &init1.msg1, prologue)?;
        let _msg1_a = init1.msg1.clone();
        let _msg2_a = resp1.msg2.clone();
        // 2回目（決定的に一致するはず）
        let mut init2 = initiator_handshake_with_eph_seed(&i, &r.pk, prologue, eph)?;
        let mut resp2 = responder_handshake(&r, &i.pk, &init2.msg1, prologue)?;
        assert_eq!(init2.msg1, msg1_a);
        assert_eq!(resp2.msg2, msg2_a);
        // セッション鍵・ノンスも一致する（同一AAD/メッセージで同一CT）
        let _aad = b"a";
        let _m = b"x";
        let (_, ct1) = init1.tx.sealnext(aad, m)?;
        let (_, ct2) = init2.tx.sealnext(aad, m)?;
        assert_eq!(ct1, ct2);
        let (_, rt1) = resp1.tx.sealnext(aad, b"y")?;
        let (_, rt2) = resp2.tx.sealnext(aad, b"y")?;
        assert_eq!(rt1, rt2);
    }
}
