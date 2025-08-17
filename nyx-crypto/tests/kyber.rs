
#![cfg(feature = "kyber")]
#[test]
fn kyber_kem_roundtrip_shared_secret() {
	use nyx_crypto::kyber;
	let mut rng = rand::thread_rng();
	let (sk, pk) = kyber::keypair(&mut rng).expect("keypair");
	let (ct, ss_alice) = kyber::encapsulate(&pk, &mut rng).expect("encapsulate");
	let ss_bob = kyber::decapsulate(&ct, &sk).expect("decapsulate");
	assert_eq!(ss_alice, ss_bob);
}

