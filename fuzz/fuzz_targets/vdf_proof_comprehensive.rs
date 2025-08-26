#![no_main]

use libfuzzer_sys::fuzz_target;
use nyx_crypto::vdf::{VdfProof, VdfParameters, VdfChallenge, VdfBatch, VdfOptimizer};

fuzz_target!(|data: &[u8]| {
    // Skip inputs that are too small for comprehensive testing
    if data.len() < 64 {
        return;
    }

    // Parse comprehensive VDF parameters from input
    let difficulty = ((data[0] as u32) % 500) + 50; // 50-549 iterations for comprehensive testing
    let security_level = match data[1] % 3 {
        0 => 128,
        1 => 192,
        2 => 256,
        _ => 128,
    };
    let prime_size = match data[2] % 3 {
        0 => 1024,
        1 => 2048,
        2 => 4096,
        _ => 2048,
    };
    let enable_optimizations = data[3] % 2 == 1;
    let batch_size = ((data[4] % 8) + 1) as usize; // 1-8 batch size

    // Create comprehensive VDF parameters
    let params = VdfParameters {
        difficulty,
        security_level,
        prime_size,
        enable_optimizations,
    };

    // Test VDF optimizer configuration
    let optimizer_config = VdfOptimizer::new()
        .with_parallel_workers(batch_size.min(4))
        .with_memory_optimization(enable_optimizations)
        .with_cache_size(1024);

    // Create multiple challenges for batch testing
    let mut challenges = Vec::new();
    let challenge_start = 5;
    let challenge_size = 32;

    for i in 0..batch_size {
        let offset = challenge_start + (i * challenge_size);
        if offset + challenge_size <= data.len() {
            let challenge_bytes = &data[offset..offset + challenge_size];
            if let Ok(challenge) = VdfChallenge::from_bytes(challenge_bytes) {
                challenges.push(challenge);
            }
        }
    }

    if challenges.is_empty() {
        return;
    }

    // Test individual VDF proof generation with comprehensive validation
    let first_challenge = &challenges[0];
    if let Ok(proof) = VdfProof::generate_with_optimizer(first_challenge, &params, &optimizer_config) {
        // Test comprehensive proof properties
        let _ = proof.difficulty();
        let _ = proof.challenge_hash();
        let _ = proof.computation_time();
        let _ = proof.memory_usage();
        let _ = proof.verification_complexity();

        // Test proof serialization with different formats
        let serialized_compact = proof.to_bytes_compact();
        let serialized_full = proof.to_bytes_full();
        let serialized_json = proof.to_json();

        // Test deserialization from different formats
        if let Ok(compact_proof) = VdfProof::from_bytes_compact(&serialized_compact) {
            let _ = compact_proof.verify_fast(first_challenge, &params);
        }
        if let Ok(full_proof) = VdfProof::from_bytes_full(&serialized_full) {
            let _ = full_proof.verify_comprehensive(first_challenge, &params);
        }
        if let Ok(json_proof) = VdfProof::from_json(&serialized_json) {
            let _ = json_proof.verify(first_challenge, &params);
        }

        // Test proof optimization
        if let Ok(optimized_proof) = proof.optimize(&params) {
            let _ = optimized_proof.verify(first_challenge, &params);
        }
    }

    // Test batch VDF operations
    if challenges.len() > 1 {
        let mut batch = VdfBatch::new(&params);
        
        // Add challenges to batch
        for challenge in &challenges {
            let _ = batch.add_challenge(challenge.clone());
        }

        // Test batch proof generation
        if let Ok(batch_proofs) = batch.generate_proofs_parallel(&optimizer_config) {
            // Test batch verification
            let verification_results = VdfProof::batch_verify_comprehensive(
                &batch_proofs,
                &challenges,
                &params
            );

            // Test batch proof aggregation
            if let Ok(aggregated_proof) = VdfProof::aggregate_batch(&batch_proofs, &challenges) {
                let _ = aggregated_proof.verify_aggregated(&challenges, &params);
            }

            // Test partial batch verification
            if batch_proofs.len() > 2 {
                let partial_proofs = &batch_proofs[..2];
                let partial_challenges = &challenges[..2];
                let _ = VdfProof::batch_verify_partial(
                    partial_proofs,
                    partial_challenges,
                    &params
                );
            }
        }

        // Test batch optimization strategies
        let optimization_strategies = [
            optimizer_config.clone(),
            optimizer_config.with_parallel_workers(1),
            optimizer_config.with_memory_optimization(false),
            optimizer_config.with_cache_size(512),
        ];

        for strategy in &optimization_strategies {
            if let Ok(strategy_proofs) = batch.generate_proofs_with_strategy(strategy) {
                // Compare proof quality and performance
                let _ = VdfProof::compare_batch_quality(&strategy_proofs, &challenges, &params);
            }
        }
    }

    // Test VDF security properties
    if data.len() > challenge_start + (batch_size * challenge_size) + 32 {
        let security_test_data = &data[challenge_start + (batch_size * challenge_size)..];
        
        // Test against malicious inputs
        if let Ok(malicious_challenge) = VdfChallenge::from_bytes(&security_test_data[..32.min(security_test_data.len())]) {
            // Test proof generation with potentially malicious challenge
            if let Ok(proof) = VdfProof::generate(&malicious_challenge, &params) {
                // Verify proof security properties
                let _ = proof.verify_security_properties(&malicious_challenge, &params);
                let _ = proof.check_for_backdoors(&params);
                let _ = proof.validate_entropy(&malicious_challenge);
            }
        }

        // Test timing attack resistance
        let timing_test_challenges: Vec<_> = security_test_data
            .chunks(32)
            .take(3)
            .filter_map(|chunk| VdfChallenge::from_bytes(chunk).ok())
            .collect();

        if timing_test_challenges.len() >= 2 {
            let _ = VdfProof::test_timing_attack_resistance(
                &timing_test_challenges,
                &params,
                &optimizer_config
            );
        }
    }

    // Test error handling and edge cases
    let edge_case_params = [
        VdfParameters { difficulty: 1, ..params },
        VdfParameters { difficulty: 10000, ..params },
        VdfParameters { security_level: 64, ..params },
        VdfParameters { prime_size: 512, ..params },
    ];

    for edge_params in &edge_case_params {
        if let Ok(edge_proof) = VdfProof::generate(first_challenge, edge_params) {
            let _ = edge_proof.verify(first_challenge, edge_params);
        }
    }

    // Test VDF proof chaining
    if challenges.len() >= 3 {
        let mut chained_proofs = Vec::new();
        let mut current_challenge = challenges[0].clone();

        for i in 1..challenges.len().min(4) {
            if let Ok(proof) = VdfProof::generate(&current_challenge, &params) {
                chained_proofs.push(proof.clone());
                
                // Create next challenge from previous proof
                current_challenge = VdfChallenge::from_proof_hash(&proof);
            }
        }

        // Verify the proof chain
        if !chained_proofs.is_empty() {
            let _ = VdfProof::verify_chain(&chained_proofs, &challenges[0], &params);
        }
    }
});
