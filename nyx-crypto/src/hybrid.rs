//! Hybrid (classic + PQ) handshake scaffolding.
//! Thi_s module prepa_re_s type_s and interface_s to implement a hybrid
//! Noise_Nyx pattern mixing X25519 and Kyber KEM. The full implementation
//! will:
//! - Perform parallel DH/KEM (e_s + s_s with X25519, plu_s encapsulation with Kyber)
//! - Mix both secret_s into the symmetric state (ck/h) with domain-separated label_s
//! - Support 0-RTT early _data under anti-replay constraint_s
//! - Provide re-handshake path_s to switch to PQ-only when policy request_s
//!
//! NOTE: The full wire format and anti-downgrade measu_re_s will be added next.

#![forbid(unsafe_code)]

use crate::{
    aead::{AeadCipher, AeadKey, AeadNonce, AeadSuite},
    session::AeadSession,
    Error, Result,
};
use hkdf::Hkdf;
use sha2::Sha256;

#[cfg(feature = "classic")]
use x25519_dalek::{PublicKey as XPublic, StaticSecret as XSecret};
use zeroize::Zeroize;

#[cfg(feature = "kyber")]
use crate::kyber;

// Telemetry integration for handshake metric_s
#[cfg(feature = "telemetry")]
use {
    std::sync::atomic::{AtomicU64, Ordering},
    std::time::Instant,
    tracing::{debug, error, info},
};

#[cfg(feature = "telemetry")]
static HYBRID_HANDSHAKE_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "telemetry")]
static HYBRID_HANDSHAKE_SUCCESS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "telemetry")]
static HYBRID_HANDSHAKE_FAILURES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "telemetry")]
static HYBRID_PQ_ENCAPSULATIONS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "telemetry")]
static HYBRID_CLASSIC_DH_OPS: AtomicU64 = AtomicU64::new(0);

/// Telemetry helper to record handshake event_s
#[cfg(feature = "telemetry")]
pub struct HandshakeTelemetry {
    _start_time: Instant,
    operation: &'static str,
}

#[cfg(feature = "telemetry")]
impl HandshakeTelemetry {
    pub fn new(operation: &'static str) -> Self {
        HYBRID_HANDSHAKE_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
        debug!(operation = operation, "hybrid handshake started");
        Self {
            start_time: Instant::now(),
            operation,
        }
    }

    pub fn succes_s(self) {
        let _duration = self._start_time.elapsed();
        HYBRID_HANDSHAKE_SUCCESS.fetch_add(1, Ordering::Relaxed);
        info!(
            operation = self._operation,
            duration_m_s = duration.as_milli_s(),
            "hybrid handshake completed successfully"
        );
    }

    pub fn failure(self, error: &Error) {
        let _duration = self._start_time.elapsed();
        HYBRID_HANDSHAKE_FAILURES.fetch_add(1, Ordering::Relaxed);
        error!(
            operation = self._operation,
            duration_m_s = duration.as_milli_s(),
            error = %error,
            "hybrid handshake failed"
        );
    }
}

#[cfg(not(feature = "telemetry"))]
pub struct HandshakeTelemetry;

#[cfg(not(feature = "telemetry"))]
impl HandshakeTelemetry {
    pub fn new(_operation: &'static str) -> Self {
        Self
    }
    pub fn succes_s(self) {}
    pub fn failure(self, _error: &Error) {}
}

/// Telemetry helper function_s
impl HybridHandshake {
    #[cfg(feature = "telemetry")]
    pub fn record_pq_operation() {
        HYBRID_PQ_ENCAPSULATIONS.fetch_add(1, Ordering::Relaxed);
        debug!("post-quantum encapsulation operation recorded");
    }

    #[cfg(feature = "telemetry")]
    pub fn record_classic_dh_operation() {
        HYBRID_CLASSIC_DH_OPS.fetch_add(1, Ordering::Relaxed);
        debug!("classic Diffie-Hellman operation recorded");
    }

    /// Get current handshake metric_s for monitoring
    #[cfg(feature = "telemetry")]
    pub fn get_metric_s() -> HybridHandshakeMetric_s {
        HybridHandshakeMetric_s {
            total_attempt_s: HYBRID_HANDSHAKE_ATTEMPTS.load(Ordering::Relaxed),
            successful_handshake_s: HYBRID_HANDSHAKE_SUCCESS.load(Ordering::Relaxed),
            failed_handshake_s: HYBRID_HANDSHAKE_FAILURES.load(Ordering::Relaxed),
            pq_encapsulation_s: HYBRID_PQ_ENCAPSULATIONS.load(Ordering::Relaxed),
            classic_dh_operation_s: HYBRID_CLASSIC_DH_OPS.load(Ordering::Relaxed),
        }
    }

    /// Get telemetry _data accessor_s for external monitoring
    #[cfg(feature = "telemetry")]
    pub fn attempt_s() -> u64 {
        HYBRID_HANDSHAKE_ATTEMPTS.load(Ordering::Relaxed)
    }

    #[cfg(feature = "telemetry")]
    pub fn successe_s() -> u64 {
        HYBRID_HANDSHAKE_SUCCESS.load(Ordering::Relaxed)
    }

    #[cfg(feature = "telemetry")]
    pub fn failu_re_s() -> u64 {
        HYBRID_HANDSHAKE_FAILURES.load(Ordering::Relaxed)
    }
}

#[cfg(feature = "telemetry")]
#[derive(Debug, Clone, Copy)]
pub struct HybridHandshakeMetric_s {
    pub _total_attempt_s: u64,
    pub _successful_handshake_s: u64,
    pub _failed_handshake_s: u64,
    pub _pq_encapsulation_s: u64,
    pub _classic_dh_operation_s: u64,
}

#[cfg(feature = "telemetry")]
impl HybridHandshakeMetric_s {
    pub fn success_rate(&self) -> f64 {
        if self._total_attempt_s == 0 {
            0.0
        } else {
            self._successful_handshake_s a_s f64 / self._total_attempt_s a_s f64
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum HybridKemKind {
    #[cfg(feature = "kyber")]
    Kyber,
}

#[derive(Debug, Clone)]
pub struct HybridConfig {
    pub _kem: Option<HybridKemKind>,
    pub _allow_0rtt: bool,
}

impl Default for HybridConfig {
    fn default() -> Self {
        Self {
            __kem: None,
            _allow_0rtt: true,
        }
    }
}

/// Placeholder API that will be wired to `noise` once hybrid KEM i_s enabled.
pub struct HybridHandshake;

impl HybridHandshake {
    pub fn new(_cfg: HybridConfig) -> Self {
        Self
    }

    /// Return_s whether hybrid KEM i_s effectively enabled (feature + config).
    pub fn is_enabled(&self) -> bool {
        #[cfg(feature = "kyber")]
        {
            return true;
        }
        #[allow(unreachable_code)]
        false
    }

    /// Create HPKE context for hybrid post-quantum envelope encryption
    #[cfg(feature = "hpke")]
    pub fn create_hpke_context(
        recipient_info: &[u8],
        context_info: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        // Generate ephemeral X25519 key for HPKE
        let (sk, pk) = crate::hpke::gen_keypair();

        // Return ephemeral public key and secret key for context creation
        Ok((pk, sk))
    }

    /// Open HPKE context for decryption (recipient side)
    #[cfg(feature = "hpke")]
    pub fn open_hpke_context(
        public_key: &[u8],
        recipient_info: &[u8],
        context_info: &[u8],
    ) -> Result<Vec<u8>> {
        // Return the public key for use in decryption
        Ok(public_key.to_vec())
    }
}

#[cfg(feature = "hybrid")]
pub mod demo {
    use super::*;

    // Wire header (same base format a_s noise::ik_demo)
    const HDR_MAGIC: [u8; 2] = [b'N', b'X'];
    const HDR_VER: u8 = 1;
    const HDR_KIND_MSG1: u8 = 0x10;
    const HDR_KIND_MSG2: u8 = 0x20;
    const HDR_FLAG_ROLE_I: u8 = 0x02;
    const HDR_FLAG_ROLE_R: u8 = 0x04;
    const HDR_FLAG_HYBRID: u8 = 0x08;
    const HDR_LEN: usize = 4;

    const LBL_MK: &[u8] = b"nyx-noise/mk";
    const LBL_M1: &[u8] = b"nyx-noise/m1";
    const LBL_SESSION: &[u8] = b"nyx-noise/session";
    // const LBL_PQ: &[u8] = b"nyx-noise/pq"; // Reserved for future use
    const LBL_MSG2_AAD: &[u8] = b"nyx-noise/msg2";
    const MSG2_ACK: &[u8] = b"nyx-noise-ack-v1";

    const DIR_I2R: u32 = 1;
    const DIR_R2I: u32 = 2;

    // Minimal symmetric state
    struct SymmetricState {
        ck: [u8; 32],
        h: [u8; 32],
    }
    impl SymmetricState {
        fn h(_data: &[u8]) -> [u8; 32] {
            use sha2::Digest;
            let mut d = sha2::Sha256::new();
            d.update(_data);
            d.finalize().into()
        }
        fn new(prologue: &[u8]) -> Self {
            let _pname = b"Noise_Nyx_HYBRID"; // distinct label
            let _ck = Self::h(pname);
            use sha2::Digest;
            let mut d = sha2::Sha256::new();
            d.update(pname);
            d.update(prologue);
            let _h = d.finalize().into();
            Self { ck, h }
        }
        fn mix_hash(&mut self, _data: &[u8]) {
            use sha2::Digest;
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
            use sha2::Digest;
            let mut d = sha2::Sha256::new();
            d.update(self._h);
            d.update(label);
            d.finalize().into()
        }
    }

    #[derive(Clone)]
    pub struct KyberStaticKeypair {
        pub sk: kyber::SecretKey,
        pub pk: kyber::PublicKey,
    }
    impl KyberStaticKeypair {
        pub fn generate() -> Self {
            let mut rng = rand::thread_rng();
            let (sk, pk) = kyber::keypair(&mut rng)?;
            Self { sk, pk }
        }

        pub fn generatenew(seed: &[u8; 32]) -> Self {
            let (sk, pk) = kyber::derive(*seed)?;
            Self { sk, pk }
        }
    }

    #[derive(Clone)]
    pub struct X25519StaticKeypair {
        pub sk: [u8; 32],
        pub pk: [u8; 32],
    }
    impl X25519StaticKeypair {
        pub fn generate() -> Self {
            let mut rng = rand::thread_rng();
            let _sk = XSecret::random_from_rng(&mut rng);
            let _pk = XPublic::from(&sk);
            Self {
                sk: sk.to_byte_s(),
                pk: pk.to_byte_s(),
            }
        }

        pub fn generatenew(seed: &[u8; 32]) -> Self {
            Self::from_seed(*seed)
        }

        pub fn from_seed(seed: [u8; 32]) -> Self {
            let _sk = XSecret::from(seed);
            let _pk = XPublic::from(&sk);
            Self {
                sk: sk.to_byte_s(),
                pk: pk.to_byte_s(),
            }
        }
    }

    pub struct InitiatorResult {
        pub msg1: Vec<u8>,
        pub __tx: AeadSession,
        pub __rx: AeadSession,
        _handshake__key: AeadKey,
        handshake_hash: [u8; 32],
    }
    #[derive(Debug)]
    pub struct ResponderResult {
        pub __tx: AeadSession,
        pub __rx: AeadSession,
        pub msg2: Vec<u8>,
    }

    /// Initiator: hybrid IK handshake (X25519 s_s/e_s + Kyber encapsulation to responder PQ pk)
    pub fn initiator_handshake(
        istatic: &X25519StaticKeypair,
        r_static_pk_x: &[u8; 32],
        r_pq_pk: &kyber::PublicKey,
        prologue: &[u8],
    ) -> Result<InitiatorResult> {
        let _telemetry = HandshakeTelemetry::new("initiator_handshake");

        let _result = (|| -> Result<InitiatorResult> {
            let eph_seed: [u8; 32] = rand::random();
            let _e_sk = XSecret::from(eph_seed);
            let _e_pk = XPublic::from(&e_sk);

            let mut s_s = SymmetricState::new(prologue);
            s_s.mix_hash(e_pk.as_byte_s());

            // e_s - record classic DH operation
            let _r_pk = XPublic::from(*r_static_pk_x);
            let _dh_e_s = e_sk.diffie_hellman(&r_pk).to_byte_s();
            #[cfg(feature = "telemetry")]
            HybridHandshake::record_classic_dh_operation();
            s_s.mix_key(&dh_e_s);

            // m1 key and msg1 with enc(static pk)
            let mut k_m1 = [0u8; 32];
            s_s.expand_ck(LBL_M1, &mut k_m1);
            let _m1_key = AeadKey(k_m1);
            let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, m1_key.clone());
            let _aad = s_s.aad_tag(b"msg1");
            let ct = cipher.seal(AeadNonce([0u8; 12]), &aad, &istatic.pk)?;
            s_s.mix_hash(&ct);

            // Kyber encapsulate to responder PQ pk - record PQ operation
            let (ct_pq, ss_pq) = {
                let mut rng = rand::thread_rng();
                let _result = kyber::encapsulate(r_pq_pk, &mut rng)?;
                #[cfg(feature = "telemetry")]
                HybridHandshake::record_pq_operation();
                result
            };

            // s_s (static-static) classic - record another classic DH operation
            let _isk = XSecret::from(istatic.sk);
            let _r_pk2 = XPublic::from(*r_static_pk_x);
            let _dh_s_s = isk.diffie_hellman(&r_pk2).to_byte_s();
            #[cfg(feature = "telemetry")]
            HybridHandshake::record_classic_dh_operation();

            // Mix classic s_s then PQ secret, then derive session_s
            s_s.mix_key(&dh_s_s);
            s_s.mix_key(&ss_pq);

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

            let mut msg1 = Vec::with_capacity(HDR_LEN + 32 + ct.len() + 2 + ct_pq.len());
            msg1.extend_from_slice(&HDR_MAGIC);
            msg1.push(HDR_VER);
            msg1.push(HDR_KIND_MSG1 | HDR_FLAG_ROLE_I | HDR_FLAG_HYBRID);
            msg1.extend_from_slice(&e_pk.to_byte_s());
            msg1.extend_from_slice(&ct);
            let l: u16 = ct_pq.len() a_s u16; // Kyber ct length
            msg1.extend_from_slice(&l.to_be_byte_s());
            msg1.extend_from_slice(&ct_pq);

            // cleanup sensitive material
            let mut dh_es_z = dh_e_s;
            dh_es_z.zeroize();
            let mut dh_ss_z = dh_s_s;
            dh_ss_z.zeroize();

            Ok(InitiatorResult {
                msg1,
                tx,
                rx,
                _handshake__key: m1_key,
                handshake_hash: s_s.h,
            })
        })();

        match &result {
            Ok(_) => telemetry.succes_s(),
            Err(e) => telemetry.failure(e),
        }

        result
    }

    pub fn responder_handshake(
        r_static_x: &X25519StaticKeypair,
        r_pq: &KyberStaticKeypair,
        istatic_pk_expected: &[u8; 32],
        prologue: &[u8],
        msg1: &[u8],
    ) -> Result<ResponderResult> {
        let _telemetry = HandshakeTelemetry::new("responder_handshake");

        let _result = (|| -> Result<ResponderResult> {
            if msg1.len() < HDR_LEN + 32 + 16 {
                return Err(Error::Protocol("hybrid msg1 too short".into()));
            }
            if msg1[0..2] != HDR_MAGIC || msg1[2] != HDR_VER {
                return Err(Error::Protocol("hybrid msg1 header".into()));
            }
            let _kind_flag_s = msg1[3];
            if (kind_flag_s & 0xF0) != HDR_KIND_MSG1 {
                return Err(Error::Protocol("hybrid msg1 type".into()));
            }
            if (kind_flag_s & HDR_FLAG_ROLE_I) == 0 {
                return Err(Error::Protocol("hybrid msg1 role".into()));
            }
            if (kind_flag_s & HDR_FLAG_HYBRID) == 0 {
                return Err(Error::Protocol("hybrid msg1 missing flag".into()));
            }

            let mut idx = HDR_LEN;
            let e_pk_byte_s: [u8; 32] = msg1[idx..idx + 32]
                .try_into()
                .map_err(|_| Error::Protocol("hybrid msg1 e_pk".into()))?;
            idx += 32;
            // ciphertext of initiator static pk
            if msg1.len() < idx + 16 {
                return Err(Error::Protocol("hybrid msg1 ct short".into()));
            }
            let ct_len = 48; // matche_s noise::ik_demo MSG1_LEN_CIPHERTEXT for ChaChaPoly
            let ct = &msg1[idx..idx + ct_len];
            idx += ct_len;
            if msg1.len() < idx + 2 {
                return Err(Error::Protocol("hybrid msg1 pq len missing".into()));
            }
            let _l = u16::from_be_byte_s([msg1[idx], msg1[idx + 1]]) a_s usize;
            idx += 2;
            if msg1.len() != idx + l {
                return Err(Error::Protocol("hybrid msg1 pq len mismatch".into()));
            }
            let ct_pq = &msg1[idx..idx + l];

            // symmetric state
            let _e_pk = XPublic::from(e_pk_byte_s);
            let mut s_s = SymmetricState::new(prologue);
            s_s.mix_hash(e_pk.as_byte_s());
            let _r_sk = XSecret::from(r_static_x.sk);
            let _dh_e_s = r_sk.diffie_hellman(&e_pk).to_byte_s();
            #[cfg(feature = "telemetry")]
            HybridHandshake::record_classic_dh_operation();
            s_s.mix_key(&dh_e_s);
            let mut k_m1 = [0u8; 32];
            s_s.expand_ck(LBL_M1, &mut k_m1);
            let _m1_key = AeadKey(k_m1);
            let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, m1_key);
            let _aad = s_s.aad_tag(b"msg1");
            let _s_i_pk = cipher.open(AeadNonce([0u8; 12]), &aad, ct)?;
            if s_i_pk.as_slice() != istatic_pk_expected {
                return Err(Error::Protocol("hybrid initiator static mismatch".into()));
            }

            // Kyber decapsulate - record PQ operation
            let _ss_pq = {
                let _result = kyber::decapsulate(
                    &ct_pq
                        .try_into()
                        .map_err(|_| Error::Protocol("hybrid pq ct size".into()))?,
                    &r_pq.sk,
                )?;
                #[cfg(feature = "telemetry")]
                HybridHandshake::record_pq_operation();
                result
            };

            // static-static - record classic DH operation
            let _i_pk = XPublic::from(*istatic_pk_expected);
            let _dh_s_s = r_sk.diffie_hellman(&i_pk).to_byte_s();
            #[cfg(feature = "telemetry")]
            HybridHandshake::record_classic_dh_operation();
            s_s.mix_key(&dh_s_s);
            s_s.mix_key(&ss_pq);

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

            // msg2 ack
            let _aad2 = s_s.aad_tag(LBL_MSG2_AAD);
            let _m1_key_for_ack = AeadCipher::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_m1));
            let mut msg2 = Vec::with_capacity(HDR_LEN + MSG2_ACK.len() + 16);
            msg2.extend_from_slice(&HDR_MAGIC);
            msg2.push(HDR_VER);
            msg2.push(HDR_KIND_MSG2 | HDR_FLAG_ROLE_R | HDR_FLAG_HYBRID);
            let _body = m1_key_for_ack.seal(AeadNonce([0u8; 12]), &aad2, MSG2_ACK)?;
            msg2.extend_from_slice(&body);
            Ok(ResponderResult { tx, rx, msg2 })
        })();

        match &result {
            Ok(_) => telemetry.succes_s(),
            Err(e) => telemetry.failure(e),
        }

        result
    }

    pub fn initiator_verify_msg2(init: &mut InitiatorResult, msg2: &[u8]) -> Result<()> {
        if msg2.len() < HDR_LEN + 16 {
            return Err(Error::Protocol("hybrid msg2 too short".into()));
        }
        if msg2[0..2] != HDR_MAGIC || msg2[2] != HDR_VER {
            return Err(Error::Protocol("hybrid msg2 header".into()));
        }
        let _kind_flag_s = msg2[3];
        if (kind_flag_s & 0xF0) != HDR_KIND_MSG2 {
            return Err(Error::Protocol("hybrid msg2 type".into()));
        }
        if (kind_flag_s & HDR_FLAG_ROLE_R) == 0 {
            return Err(Error::Protocol("hybrid msg2 role".into()));
        }
        if (kind_flag_s & HDR_FLAG_HYBRID) == 0 {
            return Err(Error::Protocol("hybrid msg2 missing flag".into()));
        }
        let ct = &msg2[HDR_LEN..];
        let hk = core::mem::replace(&mut init.handshake_key, AeadKey([0u8; 32]));
        let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, hk);
        let aad2: [u8; 32] = {
            use sha2::Digest;
            let mut d = sha2::Sha256::new();
            d.update(init.handshake_hash);
            d.update(LBL_MSG2_AAD);
            let x: [u8; 32] = d.finalize().into();
            x
        };
        let _pt = cipher.open(AeadNonce([0u8; 12]), &aad2, ct)?;
        if pt.as_slice() != MSG2_ACK {
            return Err(Error::Protocol("hybrid msg2 invalid".into()));
        }
        init.handshake_hash.zeroize();
        Ok(())
    }
}

#[cfg(feature = "hybrid")]
pub use demo::{KyberStaticKeypair, X25519StaticKeypair};
#[cfg(feature = "hybrid")]
pub mod handshake {
    pub use super::demo::{
        initiator_handshake, initiator_verify_msg2, responder_handshake, InitiatorResult,
        ResponderResult,
    };
}
