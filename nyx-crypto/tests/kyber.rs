#![cfg(feature = "kyber")]
#![allow(missing_docs)]
#[test]
fn kyber_kem_roundtrip_shared_secret() -> Result<(), Box<dyn std::error::Error>> {
    use nyx_crypto::kyber;
    let mut rng = rand::thread_rng();
    let (sk, pk) = kyber::keypair(&mut rng)?;
    let (ct, ss_alice) = kyber::encapsulate(&pk, &mut rng)?;
    let ss_bob = kyber::decapsulate(&ct, &sk)?;
    assert_eq!(ss_alice, ss_bob);
    Ok(())
}
