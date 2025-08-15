use criterion::{black_box, criterion_group, criterion_main, Criterion};
use x25519_dalek::{EphemeralSecret, PublicKey};
// Use the 0.6 compatibility crate for OsRng to satisfy EphemeralSecret::random_from_rng bounds
use rand_core_06::OsRng;

fn diffie_hellman_bench(c: &mut Criterion) {
    // Fixed peer public key for DH target
    let peer_secret = EphemeralSecret::random_from_rng(OsRng);
    let peer_public = PublicKey::from(&peer_secret);

    c.bench_function("x25519_dh", |b| {
        b.iter(|| {
            // Generate a fresh ephemeral secret per iteration to avoid moving captured vars
            let my_secret = EphemeralSecret::random_from_rng(OsRng);
            let _ = my_secret.diffie_hellman(black_box(&peer_public));
        })
    });
}

criterion_group!(benches, diffie_hellman_bench);
criterion_main!(benches);
