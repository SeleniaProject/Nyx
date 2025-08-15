#![cfg(feature = "pq")]
/// @spec 3. Hybrid Post-Quantum Handshake
use nyx_crypto::noise::pq::{initiator_encapsulate, responder_decapsulate, responder_keypair};

#[test]
fn kyber_kem_session_key_matches() {
    // Responder generates keypair
    let (pk, sk) = responder_keypair();

    // Initiator encapsulates
    let (ct, key_i) = initiator_encapsulate(&pk);

    // Responder decapsulates
    let key_r = responder_decapsulate(&ct, &sk);

    // Session keys must match
    assert_eq!(key_i.0, key_r.0);
}
