#![no_main]

use libfuzzer_sys::fuzz_target;
use nyx_crypto::vdf::{VdfProof, VdfParameters, VdfChallenge};

fuzz_target!(|data: &[u8]| {
    // Skip inputs that are too small
    if data.len() < 32 {
        return;
    }

    // Parse VDF parameters from input
    let difficulty = ((data[0] as u32) % 1000) + 100; // 100-1099 iterations for fuzzing
    let challenge_size = ((data[1] % 16) + 16) as usize; // 16-31 bytes
    
    if data.len() < challenge_size + 2 {
        return;
    }

    // Create VDF challenge from input data
    let challenge_bytes = &data[2..2 + challenge_size];
    let challenge = match VdfChallenge::from_bytes(challenge_bytes) {
        Ok(ch) => ch,
        Err(_) => return, // Invalid challenge data
    };

    // Create VDF parameters
    let params = VdfParameters {
        difficulty,
        security_level: 128,
        prime_size: 2048,
        enable_optimizations: true,
    };

    // Test VDF proof generation (with timeout for fuzzing)
    if let Ok(proof) = VdfProof::generate(&challenge, &params) {
        // Test proof serialization
        let serialized = proof.to_bytes();
        
        // Test proof deserialization
        if let Ok(deserialized_proof) = VdfProof::from_bytes(&serialized) {
            // Verify the proof
            let verification_result = deserialized_proof.verify(&challenge, &params);
            
            // Test proof properties
            let _ = deserialized_proof.difficulty();
            let _ = deserialized_proof.challenge_hash();
            let _ = deserialized_proof.computation_time();
        }
    }

    // Test batch verification if we have enough data for multiple proofs
    if data.len() > challenge_size + 64 {
        let remaining_data = &data[challenge_size + 2..];
        let mut proofs = Vec::new();
        let mut challenges = Vec::new();

        // Try to create multiple challenges/proofs from remaining data
        for chunk in remaining_data.chunks(32) {
            if chunk.len() >= 16 {
                if let Ok(ch) = VdfChallenge::from_bytes(&chunk[..16]) {
                    challenges.push(ch);
                    
                    // Limit to prevent timeout during fuzzing
                    if challenges.len() >= 3 {
                        break;
                    }
                }
            }
        }

        // Generate proofs for batch testing
        for challenge in &challenges {
            if let Ok(proof) = VdfProof::generate(challenge, &params) {
                proofs.push(proof);
            }
        }

        // Test batch verification
        if !proofs.is_empty() && proofs.len() == challenges.len() {
            let _ = VdfProof::batch_verify(&proofs, &challenges, &params);
        }
    }

    // Test edge cases with malformed data
    if data.len() > challenge_size + 32 {
        let malformed_data = &data[challenge_size + 32..];
        
        // Test with malformed proof data
        let _ = VdfProof::from_bytes(malformed_data);
        
        // Test with malformed challenge
        if malformed_data.len() >= 16 {
            let _ = VdfChallenge::from_bytes(&malformed_data[..16]);
        }
    }
});
