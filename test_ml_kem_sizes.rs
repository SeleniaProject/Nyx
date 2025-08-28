use ml_kem::{MlKem768, kem::EncapsulationKey, Ciphertext};

fn main() {
    // ML-KEM-768の正しいサイズを確認
    println!("ML-KEM-768 encapsulation key size: {}", MlKem768::ENCAPSULATION_KEY_SIZE);
    println!("ML-KEM-768 decapsulation key size: {}", MlKem768::DECAPSULATION_KEY_SIZE);
    println!("ML-KEM-768 ciphertext size: {}", MlKem768::CIPHERTEXT_SIZE);
    println!("ML-KEM-768 shared secret size: {}", MlKem768::SHARED_SECRET_SIZE);
    
    // 実際にciphertextを生成してサイズを確認
    let mut rng = rand::thread_rng();
    let (decaps_key, encaps_key) = MlKem768::generate(&mut rng);
    let (ciphertext, shared_secret) = encaps_key.encapsulate(&mut rng).unwrap();
    
    println!("Actual ciphertext length: {}", ciphertext.as_array().len());
    println!("Actual shared secret length: {}", shared_secret.as_array().len());
}
