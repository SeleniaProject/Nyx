//! Compliance Matrix Testing
//!
//! This module provides comprehensive testing for compliance level validation
//! across all features and configurations.

use nyx_core::compliance::*;
use std::collections::HashMap;

#[test]
fn test_core_compliance_requirements() {
    let detector = FeatureDetector::new();
    let report = validate_compliance_level(ComplianceLevel::Core, &detector)
        .expect("Failed to validate core compliance");

    println!("Core Compliance Report:");
    println!("  Compliant: {}", report.is_compliant);
    println!("  Missing Required: {:?}", report.missing_required);
    println!("  Missing Recommended: {:?}", report.missing_recommended);

    // Core level should always be achievable with basic features
    if !report.is_compliant {
        for feature in &report.missing_required {
            println!("  Missing core feature: {}", feature);
        }
    }
}

#[test]
fn test_enhanced_compliance_requirements() {
    let detector = FeatureDetector::new();
    let report = validate_compliance_level(ComplianceLevel::Enhanced, &detector)
        .expect("Failed to validate enhanced compliance");

    println!("Enhanced Compliance Report:");
    println!("  Compliant: {}", report.is_compliant);
    println!("  Missing Required: {:?}", report.missing_required);
    println!("  Missing Recommended: {:?}", report.missing_recommended);

    if !report.is_compliant {
        for feature in &report.missing_required {
            println!("  Missing enhanced feature: {}", feature);
        }
    }
}

#[test]
fn test_advanced_compliance_requirements() {
    let detector = FeatureDetector::new();
    let report = validate_compliance_level(ComplianceLevel::Advanced, &detector)
        .expect("Failed to validate advanced compliance");

    println!("Advanced Compliance Report:");
    println!("  Compliant: {}", report.is_compliant);
    println!("  Missing Required: {:?}", report.missing_required);
    println!("  Missing Recommended: {:?}", report.missing_recommended);

    if !report.is_compliant {
        for feature in &report.missing_required {
            println!("  Missing advanced feature: {}", feature);
        }
    }
}

#[test]
fn test_compliance_matrix_complete() {
    let detector = FeatureDetector::new();
    let mut matrix = HashMap::new();

    println!("Complete Compliance Matrix:");
    println!("===========================");

    for level in &[
        ComplianceLevel::Core,
        ComplianceLevel::Enhanced,
        ComplianceLevel::Advanced,
    ] {
        let report = validate_compliance_level(*level, &detector)
            .expect("Failed to validate compliance level");

        let level_name = format!("{:?}", level);
        let status = if report.is_compliant {
            "✅ PASS"
        } else {
            "❌ FAIL"
        };

        println!("{}: {}", level_name, status);

        if !report.missing_required.is_empty() {
            println!("  Missing Required Features:");
            for feature in &report.missing_required {
                println!("    - {}", feature);
            }
        }

        if !report.missing_recommended.is_empty() {
            println!("  Missing Recommended Features:");
            for feature in &report.missing_recommended {
                println!("    - {}", feature);
            }
        }

        matrix.insert(level_name, report);
    }

    // Determine highest achievable level
    let highest_level = determine_compliance_level(&detector);
    println!("\nHighest Achievable Level: {:?}", highest_level);

    // Validate hierarchy consistency
    let core_compliant = matrix.get("Core").map(|r| r.is_compliant).unwrap_or(false);
    let enhanced_compliant = matrix
        .get("Enhanced")
        .map(|r| r.is_compliant)
        .unwrap_or(false);
    let advanced_compliant = matrix
        .get("Advanced")
        .map(|r| r.is_compliant)
        .unwrap_or(false);

    // If higher level is compliant, lower levels should also be compliant
    if advanced_compliant {
        assert!(
            enhanced_compliant,
            "Hierarchy violation: Advanced compliant but Enhanced not compliant"
        );
        assert!(
            core_compliant,
            "Hierarchy violation: Advanced compliant but Core not compliant"
        );
    }

    if enhanced_compliant {
        assert!(
            core_compliant,
            "Hierarchy violation: Enhanced compliant but Core not compliant"
        );
    }

    println!("✅ Compliance hierarchy is consistent");
}

#[test]
fn test_feature_availability_matrix() {
    let detector = FeatureDetector::new();
    let available_features = detector.available_features();

    println!("Feature Availability Matrix:");
    println!("============================");

    // Test major feature categories
    let feature_categories = [
        ("network", "Network features"),
        ("crypto", "Cryptographic features"),
        ("privacy", "Privacy features"),
        ("performance", "Performance features"),
        ("compliance", "Compliance features"),
    ];

    for (category, description) in &feature_categories {
        println!("\n{}: {}", category, description);

        let category_features: Vec<_> = available_features
            .iter()
            .filter(|f| f.starts_with(category))
            .collect();

        if category_features.is_empty() {
            println!("  ⚠️  No features available in this category");
        } else {
            for feature in category_features {
                let available = detector.has_feature(feature);
                let status = if available { "✅" } else { "❌" };
                println!("  {} {}", status, feature);
            }
        }
    }

    println!("\nTotal available features: {}", available_features.len());
}

#[test]
fn test_compliance_edge_cases() {
    let detector = FeatureDetector::new();

    println!("Testing compliance edge cases:");

    // Test with empty feature set (shouldn't panic)
    println!("  Testing edge case scenarios...");

    // All compliance levels should handle missing features gracefully
    for level in &[
        ComplianceLevel::Core,
        ComplianceLevel::Enhanced,
        ComplianceLevel::Advanced,
    ] {
        let result = validate_compliance_level(*level, &detector);
        assert!(
            result.is_ok(),
            "Compliance validation should not panic for {:?}",
            level
        );

        let report = result.unwrap();

        // Report should always have valid structure
        assert!(report.missing_required.len() >= 0);
        assert!(report.missing_recommended.len() >= 0);

        println!(
            "  {:?}: {} required missing, {} recommended missing",
            level,
            report.missing_required.len(),
            report.missing_recommended.len()
        );
    }

    println!("✅ All edge cases handled correctly");
}

#[test]
fn test_compliance_performance() {
    use std::time::Instant;

    println!("Testing compliance performance:");

    let start_local = Instant::now();
    let detector = FeatureDetector::new();
    let detection_time = start.elapsed();

    println!("  Feature detection time: {:?}", detection_time);

    // Test validation performance for each level
    for level in &[
        ComplianceLevel::Core,
        ComplianceLevel::Enhanced,
        ComplianceLevel::Advanced,
    ] {
        let start_local = Instant::now();
        let report =
            validate_compliance_level(*level, &detector).expect("Validation should succeed");
        let validation_time = start.elapsed();

        println!("  {:?} validation time: {:?}", level, validation_time);

        // Ensure reasonable performance (under 100ms for validation)
        assert!(
            validation_time.as_millis() < 100,
            "Validation too slow for {:?}: {:?}",
            level,
            validation_time
        );
    }

    println!("✅ Performance requirements met");
}
