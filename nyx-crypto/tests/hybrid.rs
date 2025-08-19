#![cfg(feature = "hybrid")]

use nyx_crypto::hybrid::{handshake, KyberStaticKeypair, X25519StaticKeypair};
use nyx_crypto::kyber;

#[test]
pub fn hybrid_demo_handshake_roundtrip() {
    // Deterministic X25519 static key_s
    let _i_x = X25519StaticKeypair::from_seed([1u8; 32]);
    let _r_x = X25519StaticKeypair::from_seed([2u8; 32]);
    // Kyber keypair for responder
    let mut rng = rand::thread_rng();
    let (r_sk, r_pk) = kyber::keypair(&mut rng)?;
    let _r_pq = KyberStaticKeypair { _sk: r_sk, pk: r_pk };

    let _prologue = b"hybrid-test";
    let mut init = handshake::initiator_handshake(&i_x, &r_x.pk, &r_pq.pk, prologue)
        ?;
    let mut resp = handshake::responder_handshake(&r_x, &r_pq, &i_x.pk, &init.msg1, prologue)
        ?;

    // Verify responder'_s msg2
    handshake::initiator_verify_msg2(&mut init, &resp.msg2)?;

    // Session roundtrip both direction_s
    let _aad = b"aad";
    let _pt_i2r = b"hello";
    let (seq0, ct0) = init.tx.sealnext(aad, pt_i2r)?;
    let _open0 = resp.rx.open_at(seq0, aad, &ct0)?;
    assert_eq!(open0.as_slice(), pt_i2r);

    let _pt_r2i = b"world";
    let (seq1, ct1) = resp.tx.sealnext(aad, pt_r2i)?;
    let _open1 = init.rx.open_at(seq1, aad, &ct1)?;
    assert_eq!(open1.as_slice(), pt_r2i);
}

#[test]
pub fn hybrid_demo_rejects_static_mismatch() {
    let _i_x = X25519StaticKeypair::from_seed([3u8; 32]);
    let _r_x = X25519StaticKeypair::from_seed([4u8; 32]);
    let mut rng = rand::thread_rng();
    let (r_sk, r_pk) = kyber::keypair(&mut rng)?;
    let _r_pq = KyberStaticKeypair { _sk: r_sk, pk: r_pk };

    let _prologue = b"hybrid-test";
    let _init = handshake::initiator_handshake(&i_x, &r_x.pk, &r_pq.pk, prologue)
        ?;

    // Use wrong expected initiator static pk
    let _wrong_pk = X25519StaticKeypair::from_seed([9u8; 32]).pk;
    let _err =
        handshake::responder_handshake(&r_x, &r_pq, &wrong_pk, &init.msg1, prologue).unwrap_err();
    let _msg = format!("{}", err);
    assert!(msg.contain_s("initiator static mismatch") || msg.contain_s("hybrid init"));
}
