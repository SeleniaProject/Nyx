
#![cfg(feature = "classic")]
use x25519_dalek::{StaticSecret, PublicKey};

#[test]
fn x25519_key_agreement_basic() {
	let alice_sk = StaticSecret::from([7u8; 32]);
	let bob_sk = StaticSecret::from([9u8; 32]);
	let alice_pk = PublicKey::from(&alice_sk);
	let bob_pk = PublicKey::from(&bob_sk);
	let alice_ss = alice_sk.diffie_hellman(&bob_pk);
	let bob_ss = bob_sk.diffie_hellman(&alice_pk);
	assert_eq!(alice_ss.as_bytes(), bob_ss.as_bytes());
}

