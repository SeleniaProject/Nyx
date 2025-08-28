use ml_kem::{MlKem768, Encapsulate, Decapsulate, kem::*};

fn main() {
    // Generate a key pair
    let (decaps_key, encaps_key) = MlKem768::generate();
    
    // Perform encapsulation
    let (ciphertext, shared_secret) = encaps_key.encapsulate();
    
    println!("ML-KEM ciphertext size: {}", ciphertext.as_slice().len());
    println!("ML-KEM shared secret size: {}", shared_secret.as_slice().len());
    println!("Encaps key size: {}", encaps_key.as_bytes().len());
    println!("Decaps key size: {}", decaps_key.as_bytes().len());
}
