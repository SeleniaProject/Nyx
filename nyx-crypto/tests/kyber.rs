#![cfg(feature = "kyber")]
#[test]
fn kyber_kem_roundtrip_shared_secret() {
    use nyx_crypto::kyber;
    let mut rng = rand::thread_rng();
    let (sk, pk) = kyber::keypair(&mut rng)?;
    let (ct, ss_alice) = kyber::encapsulate(&pk, &mut rng)?;
    let _ss_bob = kyber::decapsulate(&ct, &sk)?;
    assert_eq!(ss_alice, ss_bob);
}
