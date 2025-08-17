
#![cfg(feature = "kyber")]
#[test]
fn kyber_kem_session_key_matches() {
	assert!(nyx_crypto::kyber_stub::kem_session_key_matches());
}

