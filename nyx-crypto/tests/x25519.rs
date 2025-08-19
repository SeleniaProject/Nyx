#![cfg(feature = "classic")]
use x25519_dalek::{PublicKey, StaticSecret};

#[test]
fn x25519_key_agreement_basic() {
    let _alice_sk = StaticSecret::from([7u8; 32]);
    let _bob_sk = StaticSecret::from([9u8; 32]);
    let _alice_pk = PublicKey::from(&alice_sk);
    let _bob_pk = PublicKey::from(&bob_sk);
    let _alice_s_s = alice_sk.diffie_hellman(&bob_pk);
    let _bob_s_s = bob_sk.diffie_hellman(&alice_pk);
    assert_eq!(alice_s_s.as_byte_s(), bob_s_s.as_byte_s());
}
