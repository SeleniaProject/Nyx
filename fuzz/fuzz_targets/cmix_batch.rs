#![no_main]

use libfuzzer_sys::fuzz_target;
use nyx_mix::accumulator::{Accumulator, AccumulatorConfig, SecurityLevel};

fuzz_target!(|data: &[u8]| {
    // Skip empty input
    if data.is_empty() {
        return;
    }

    // Create test accumulator with demo security for performance
    let config = AccumulatorConfig {
        modulus_bits: 1024,
        hash_function: "sha256".to_string(),
        max_batch_size: 100,
        crypto_optimizations: true,
        security_level: SecurityLevel::Demo,
    };

    let mut accumulator = Accumulator::with_config(&config);

    // Split input data into chunks to simulate batch elements
    let chunk_size = (data.len() / 10).max(1).min(32);
    let mut elements = Vec::new();

    for chunk in data.chunks(chunk_size) {
        if !chunk.is_empty() {
            elements.push(chunk);
        }
        // Limit to reasonable batch size for fuzzing performance
        if elements.len() >= 20 {
            break;
        }
    }

    // Test adding elements to accumulator
    for element in &elements {
        if let Ok(_) = accumulator.add_element(element) {
            // Element added successfully
        }
    }

    // Test witness generation for first element if available
    if !elements.is_empty() {
        let _ = accumulator.generate_witness(elements[0]);
    }

    // Test batch verification if multiple elements
    if elements.len() > 1 {
        let witnesses: Result<Vec<_>, _> = elements.iter()
            .map(|e| accumulator.generate_witness(e))
            .collect();
        
        if let Ok(witnesses) = witnesses {
            let acc_value = accumulator.get_accumulator_value();
            let _ = nyx_mix::accumulator::batch_verify_membership(&witnesses, &elements, &acc_value);
        }
    }
});
