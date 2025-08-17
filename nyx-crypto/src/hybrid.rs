//! Hybrid (classic + PQ) handshake scaffolding.
//! This module prepares types and interfaces to implement a hybrid
//! Noise_Nyx pattern mixing X25519 and Kyber KEM. The full implementation
//! will:
//! - Perform parallel DH/KEM (es + ss with X25519, plus encapsulation with Kyber)
//! - Mix both secrets into the symmetric state (ck/h) with domain-separated labels
//! - Support 0-RTT early data under anti-replay constraints
//! - Provide re-handshake paths to switch to PQ-only when policy requests
//!
//! NOTE: The full wire format and anti-downgrade measures will be added next.

#![forbid(unsafe_code)]

use crate::{aead::{AeadCipher, AeadKey, AeadNonce, AeadSuite}, session::AeadSession, Error, Result};
use hkdf::Hkdf;
use sha2::Sha256;

#[cfg(feature = "classic")]
use x25519_dalek::{PublicKey as XPublic, StaticSecret as XSecret};
use zeroize::Zeroize;

#[cfg(feature = "kyber")]
use crate::kyber;

#[derive(Debug, Clone, Copy)]
pub enum HybridKemKind {
	#[cfg(feature = "kyber")]
	Kyber,
}

#[derive(Debug, Clone)]
pub struct HybridConfig {
	pub kem: Option<HybridKemKind>,
	pub allow_0rtt: bool,
}

impl Default for HybridConfig {
	fn default() -> Self { Self { kem: None, allow_0rtt: true } }
}

/// Placeholder API that will be wired to `noise` once hybrid KEM is enabled.
pub struct HybridHandshake;

impl HybridHandshake {
	pub fn new(_cfg: HybridConfig) -> Self { Self }

	/// Returns whether hybrid KEM is effectively enabled (feature + config).
	pub fn is_enabled(&self) -> bool {
		#[cfg(feature = "kyber")]
		{ return true; }
		#[allow(unreachable_code)]
		false
	}
}

#[cfg(feature = "hybrid")]
mod demo {
	use super::*;

	// Wire header (same base format as noise::ik_demo)
	const HDR_MAGIC: [u8;2] = [b'N', b'X'];
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
	const LBL_PQ: &[u8] = b"nyx-noise/pq";
	const LBL_MSG2_AAD: &[u8] = b"nyx-noise/msg2";
	const MSG2_ACK: &[u8] = b"nyx-noise-ack-v1";

	const DIR_I2R: u32 = 1;
	const DIR_R2I: u32 = 2;

	// Minimal symmetric state
	struct SymmetricState { ck: [u8;32], h: [u8;32] }
	impl SymmetricState {
		fn h(data: &[u8]) -> [u8;32] { use sha2::Digest; let mut d = sha2::Sha256::new(); d.update(data); d.finalize().into() }
		fn new(prologue: &[u8]) -> Self {
			let pname = b"Noise_Nyx_HYBRID"; // distinct label
			let ck = Self::h(pname);
			use sha2::Digest;
			let mut d = sha2::Sha256::new(); d.update(pname); d.update(prologue); let h = d.finalize().into();
			Self { ck, h }
		}
		fn mix_hash(&mut self, data: &[u8]) { use sha2::Digest; let mut d = sha2::Sha256::new(); d.update(self.h); d.update(data); self.h = d.finalize().into(); }
		fn mix_key(&mut self, ikm: &[u8]) { let hk = Hkdf::<Sha256>::new(Some(&self.ck), ikm); hk.expand(LBL_MK, &mut self.ck).expect("hkdf"); }
		fn expand_ck(&self, info: &[u8], out: &mut [u8]) { let hk = Hkdf::<Sha256>::from_prk(&self.ck).expect("prk"); hk.expand(info, out).expect("hkdf exp"); }
		fn aad_tag(&self, label: &[u8]) -> [u8;32] { use sha2::Digest; let mut d = sha2::Sha256::new(); d.update(self.h); d.update(label); d.finalize().into() }
	}

	#[derive(Clone)]
	pub struct KyberStaticKeypair { pub sk: kyber::SecretKey, pub pk: kyber::PublicKey }

	#[derive(Clone)]
	pub struct X25519StaticKeypair { pub sk: [u8;32], pub pk: [u8;32] }
	impl X25519StaticKeypair {
		pub fn from_seed(seed: [u8;32]) -> Self {
			let sk = XSecret::from(seed);
			let pk = XPublic::from(&sk);
			Self { sk: sk.to_bytes(), pk: pk.to_bytes() }
		}
	}

	pub struct InitiatorResult {
		pub msg1: Vec<u8>,
		pub tx: AeadSession,
		pub rx: AeadSession,
		handshake_key: AeadKey,
		handshake_hash: [u8;32],
	}
	pub struct ResponderResult { pub tx: AeadSession, pub rx: AeadSession, pub msg2: Vec<u8> }

	/// Initiator: hybrid IK handshake (X25519 ss/es + Kyber encapsulation to responder PQ pk)
	pub fn initiator_handshake(
		i_static: &X25519StaticKeypair,
		r_static_pk_x: &[u8;32],
		r_pq_pk: &kyber::PublicKey,
		prologue: &[u8],
	) -> Result<InitiatorResult> {
		let eph_seed: [u8;32] = rand::random();
		let e_sk = XSecret::from(eph_seed);
		let e_pk = XPublic::from(&e_sk);

		let mut ss = SymmetricState::new(prologue);
		ss.mix_hash(e_pk.as_bytes());

		// es
		let r_pk = XPublic::from(*r_static_pk_x);
		let dh_es = e_sk.diffie_hellman(&r_pk).to_bytes();
		ss.mix_key(&dh_es);

		// m1 key and msg1 with enc(static pk)
		let mut k_m1 = [0u8;32]; ss.expand_ck(LBL_M1, &mut k_m1);
		let m1_key = AeadKey(k_m1);
		let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, m1_key.clone());
		let aad = ss.aad_tag(b"msg1");
		let ct = cipher.seal(AeadNonce([0u8;12]), &aad, &i_static.pk)?;
		ss.mix_hash(&ct);

		// Kyber encapsulate to responder PQ pk
		let (ct_pq, ss_pq) = {
			let mut rng = rand::thread_rng();
			kyber::encapsulate(r_pq_pk, &mut rng)?
		};

		// ss (static-static) classic
		let i_sk = XSecret::from(i_static.sk);
		let r_pk2 = XPublic::from(*r_static_pk_x);
		let dh_ss = i_sk.diffie_hellman(&r_pk2).to_bytes();

		// Mix classic ss then PQ secret, then derive sessions
		ss.mix_key(&dh_ss);
		ss.mix_key(&ss_pq);

		let mut out = [0u8; 32+32+12+12];
		ss.expand_ck(LBL_SESSION, &mut out);
		let mut k_i2r = [0u8;32]; k_i2r.copy_from_slice(&out[0..32]);
		let mut k_r2i = [0u8;32]; k_r2i.copy_from_slice(&out[32..64]);
		let mut n_i2r = [0u8;12]; n_i2r.copy_from_slice(&out[64..76]);
		let mut n_r2i = [0u8;12]; n_r2i.copy_from_slice(&out[76..88]);
		out.zeroize();
		let tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_i2r), n_i2r).with_direction_id(DIR_I2R);
		let rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_r2i), n_r2i).with_direction_id(DIR_R2I);

		let mut msg1 = Vec::with_capacity(HDR_LEN + 32 + ct.len() + 2 + ct_pq.len());
		msg1.extend_from_slice(&HDR_MAGIC);
		msg1.push(HDR_VER);
		msg1.push(HDR_KIND_MSG1 | HDR_FLAG_ROLE_I | HDR_FLAG_HYBRID);
		msg1.extend_from_slice(&e_pk.to_bytes());
		msg1.extend_from_slice(&ct);
		let l: u16 = ct_pq.len() as u16; // Kyber ct length
		msg1.extend_from_slice(&l.to_be_bytes());
		msg1.extend_from_slice(&ct_pq);

		// cleanup sensitive material
		let mut dh_es_z = dh_es; dh_es_z.zeroize();
		let mut dh_ss_z = dh_ss; dh_ss_z.zeroize();

		Ok(InitiatorResult { msg1, tx, rx, handshake_key: m1_key, handshake_hash: ss.h })
	}

	pub fn responder_handshake(
		r_static_x: &X25519StaticKeypair,
		r_pq: &KyberStaticKeypair,
		i_static_pk_expected: &[u8;32],
		msg1: &[u8],
		prologue: &[u8],
	) -> Result<ResponderResult> {
		if msg1.len() < HDR_LEN + 32 + 16 { return Err(Error::Protocol("hybrid msg1 too short".into())); }
		if msg1[0..2] != HDR_MAGIC || msg1[2] != HDR_VER { return Err(Error::Protocol("hybrid msg1 header".into())); }
		let kind_flags = msg1[3];
		if (kind_flags & 0xF0) != HDR_KIND_MSG1 { return Err(Error::Protocol("hybrid msg1 type".into())); }
		if (kind_flags & HDR_FLAG_ROLE_I) == 0 { return Err(Error::Protocol("hybrid msg1 role".into())); }
		if (kind_flags & HDR_FLAG_HYBRID) == 0 { return Err(Error::Protocol("hybrid msg1 missing flag".into())); }

		let mut idx = HDR_LEN;
		let e_pk_bytes: [u8;32] = msg1[idx..idx+32].try_into().map_err(|_| Error::Protocol("hybrid msg1 e_pk".into()))?;
		idx += 32;
		// ciphertext of initiator static pk
		if msg1.len() < idx + 16 { return Err(Error::Protocol("hybrid msg1 ct short".into())); }
		let ct_len = 48; // matches noise::ik_demo MSG1_LEN_CIPHERTEXT for ChaChaPoly
		let ct = &msg1[idx..idx+ct_len];
		idx += ct_len;
		if msg1.len() < idx + 2 { return Err(Error::Protocol("hybrid msg1 pq len missing".into())); }
		let l = u16::from_be_bytes([msg1[idx], msg1[idx+1]]) as usize;
		idx += 2;
		if msg1.len() != idx + l { return Err(Error::Protocol("hybrid msg1 pq len mismatch".into())); }
		let ct_pq = &msg1[idx..idx+l];

		// symmetric state
		let e_pk = XPublic::from(e_pk_bytes);
		let mut ss = SymmetricState::new(prologue);
		ss.mix_hash(e_pk.as_bytes());
		let r_sk = XSecret::from(r_static_x.sk);
		let dh_es = r_sk.diffie_hellman(&e_pk).to_bytes();
		ss.mix_key(&dh_es);
		let mut k_m1 = [0u8;32]; ss.expand_ck(LBL_M1, &mut k_m1);
		let m1_key = AeadKey(k_m1);
		let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, m1_key);
		let aad = ss.aad_tag(b"msg1");
		let s_i_pk = cipher.open(AeadNonce([0u8;12]), &aad, ct)?;
		if s_i_pk.as_slice() != i_static_pk_expected { return Err(Error::Protocol("hybrid initiator static mismatch".into())); }

		// Kyber decapsulate
		let ss_pq = kyber::decapsulate(&ct_pq.try_into().map_err(|_| Error::Protocol("hybrid pq ct size".into()))?, &r_pq.sk)?;

		// static-static
		let i_pk = XPublic::from(*i_static_pk_expected);
		let dh_ss = r_sk.diffie_hellman(&i_pk).to_bytes();
		ss.mix_key(&dh_ss);
		ss.mix_key(&ss_pq);

		let mut out = [0u8; 32+32+12+12];
		ss.expand_ck(LBL_SESSION, &mut out);
		let mut k_i2r = [0u8;32]; k_i2r.copy_from_slice(&out[0..32]);
		let mut k_r2i = [0u8;32]; k_r2i.copy_from_slice(&out[32..64]);
		let mut n_i2r = [0u8;12]; n_i2r.copy_from_slice(&out[64..76]);
		let mut n_r2i = [0u8;12]; n_r2i.copy_from_slice(&out[76..88]);
		out.zeroize();
		let tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_r2i), n_r2i).with_direction_id(DIR_R2I);
		let rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_i2r), n_i2r).with_direction_id(DIR_I2R);

		// msg2 ack
		let aad2 = ss.aad_tag(LBL_MSG2_AAD);
		let m1_key_for_ack = AeadCipher::new(AeadSuite::ChaCha20Poly1305, AeadKey(k_m1));
		let mut msg2 = Vec::with_capacity(HDR_LEN + MSG2_ACK.len() + 16);
		msg2.extend_from_slice(&HDR_MAGIC);
		msg2.push(HDR_VER);
		msg2.push(HDR_KIND_MSG2 | HDR_FLAG_ROLE_R | HDR_FLAG_HYBRID);
		let body = m1_key_for_ack.seal(AeadNonce([0u8;12]), &aad2, MSG2_ACK)?;
		msg2.extend_from_slice(&body);
		Ok(ResponderResult { tx, rx, msg2 })
	}

	pub fn initiator_verify_msg2(init: &mut InitiatorResult, msg2: &[u8]) -> Result<()> {
		if msg2.len() < HDR_LEN + 16 { return Err(Error::Protocol("hybrid msg2 too short".into())); }
		if msg2[0..2] != HDR_MAGIC || msg2[2] != HDR_VER { return Err(Error::Protocol("hybrid msg2 header".into())); }
		let kind_flags = msg2[3];
		if (kind_flags & 0xF0) != HDR_KIND_MSG2 { return Err(Error::Protocol("hybrid msg2 type".into())); }
		if (kind_flags & HDR_FLAG_ROLE_R) == 0 { return Err(Error::Protocol("hybrid msg2 role".into())); }
		if (kind_flags & HDR_FLAG_HYBRID) == 0 { return Err(Error::Protocol("hybrid msg2 missing flag".into())); }
		let ct = &msg2[HDR_LEN..];
		let hk = core::mem::replace(&mut init.handshake_key, AeadKey([0u8;32]));
		let cipher = AeadCipher::new(AeadSuite::ChaCha20Poly1305, hk);
		let aad2: [u8;32] = {
			use sha2::Digest;
			let mut d = sha2::Sha256::new(); d.update(init.handshake_hash); d.update(LBL_MSG2_AAD); let x: [u8;32] = d.finalize().into(); x
		};
		let pt = cipher.open(AeadNonce([0u8;12]), &aad2, ct)?;
		if pt.as_slice() != MSG2_ACK { return Err(Error::Protocol("hybrid msg2 invalid".into())); }
		init.handshake_hash.zeroize();
		Ok(())
	}
}

#[cfg(feature = "hybrid")]
pub use demo::{KyberStaticKeypair, X25519StaticKeypair};
#[cfg(feature = "hybrid")]
pub mod handshake {
	pub use super::demo::{initiator_handshake, initiator_verify_msg2, responder_handshake, InitiatorResult, ResponderResult};
}
