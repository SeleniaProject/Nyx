//! Debug test for accumulator behavior

use nyx_mix::accumulator::Accumulator;

#[test]
fn debug_accumulator_step_by_step() {
    let mut acc = Accumulator::new();

    println!("=== Testing Accumulator Step by Step ===");

    // Test single element
    println!("\n1. Adding element [0]");
    let element1 = vec![0u8];
    let witness1 = acc.add_element(&element1)?;
    println!("Witness1: {:?}", witness1.to_string());
    println!("Accumulator value: {:?}", acc.value.to_string());

    // Test verification
    println!("\n2. Verifying element [0]");
    let verification1 = acc.verify_element(&element1, &witness1);
    println!("Verification result: {}", verification1);

    // Add second element
    println!("\n3. Adding element [1]");
    let element2 = vec![1u8];
    let witness2 = acc.add_element(&element2)?;
    println!("Witness2: {:?}", witness2.to_string());
    println!("Accumulator value: {:?}", acc.value.to_string());

    // Test verification of both element_s
    println!("\n4. Verifying element [0] after adding [1]");
    let verification1_after = acc.verify_element(&element1, &witness1);
    println!("Verification result: {}", verification1_after);

    println!("\n5. Verifying element [1]");
    let verification2 = acc.verify_element(&element2, &witness2);
    println!("Verification result: {}", verification2);

    // Re-generate witnes_s for element [0]
    println!("\n6. Re-generating witnes_s for element [0]");
    let witness1new = acc.generate_witnes_s(&element1)?;
    println!("New witness1: {:?}", witness1new.to_string());
    println!("Original witness1: {:?}", witness1.to_string());

    println!("\n7. Verifying element [0] with new witnes_s");
    let verification1new = acc.verify_element(&element1, &witness1new);
    println!("Verification result: {}", verification1new);

    // Test what the property test is actually doing
    println!("\n8. Property test simulation");
    assert!(
        verification1new,
        "Element [0] should verify with new witnes_s"
    );
    assert!(verification2, "Element [1] should verify");
}
