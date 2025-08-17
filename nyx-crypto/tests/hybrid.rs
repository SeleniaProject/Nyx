#![cfg(feature = "hybrid")]

use nyx_crypto::hybrid::{handshake, KyberStaticKeypair, X25519StaticKeypair};
use nyx_crypto::kyber;

#[test]
pub fn hybrid_demo_handshake_roundtrip() {
    // Deterministic X25519 static keys
    let i_x = X25519StaticKeypair::from_seed([1u8; 32]);
    let r_x = X25519StaticKeypair::from_seed([2u8; 32]);
    // Kyber keypair for responder
    let mut rng = rand::thread_rng();
    let (r_sk, r_pk) = kyber::keypair(&mut rng).expect("kyber keypair");
    let r_pq = KyberStaticKeypair { sk: r_sk, pk: r_pk };

    let prologue = b"hybrid-test";
    let mut init = handshake::initiator_handshake(&i_x, &r_x.pk, &r_pq.pk, prologue)
        .expect("initiator handshake");
    let mut resp = handshake::responder_handshake(&r_x, &r_pq, &i_x.pk, &init.msg1, prologue)
        .expect("responder handshake");

    // Verify responder's msg2
    handshake::initiator_verify_msg2(&mut init, &resp.msg2).expect("verify msg2");

    // Session roundtrip both directions
    let aad = b"aad";
    let pt_i2r = b"hello";
    let (seq0, ct0) = init.tx.seal_next(aad, pt_i2r).expect("seal i2r");
    let open0 = resp.rx.open_at(seq0, aad, &ct0).expect("open i2r");
    assert_eq!(open0.as_slice(), pt_i2r);

    let pt_r2i = b"world";
    let (seq1, ct1) = resp.tx.seal_next(aad, pt_r2i).expect("seal r2i");
    let open1 = init.rx.open_at(seq1, aad, &ct1).expect("open r2i");
    assert_eq!(open1.as_slice(), pt_r2i);
}

#[test]
pub fn hybrid_demo_rejects_static_mismatch() {
    let i_x = X25519StaticKeypair::from_seed([3u8; 32]);
    let r_x = X25519StaticKeypair::from_seed([4u8; 32]);
    let mut rng = rand::thread_rng();
    let (r_sk, r_pk) = kyber::keypair(&mut rng).expect("kyber keypair");
    let r_pq = KyberStaticKeypair { sk: r_sk, pk: r_pk };

    let prologue = b"hybrid-test";
    let init = handshake::initiator_handshake(&i_x, &r_x.pk, &r_pq.pk, prologue)
        .expect("initiator handshake");

    // Use wrong expected initiator static pk
    let wrong_pk = X25519StaticKeypair::from_seed([9u8; 32]).pk;
    let err =
        handshake::responder_handshake(&r_x, &r_pq, &wrong_pk, &init.msg1, prologue).unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("initiator static mismatch") || msg.contains("hybrid init"));
}
