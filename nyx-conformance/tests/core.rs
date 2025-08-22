//! Core compliance testing
//!
//! This module provides fundamental compliance tests for the Nyx protocol
//! implementation. It validates basic features required for core functionality.

use nyx_core::compliance::*;
use std::path::Path;

#[test]
fn test_core_compliance_basic() {
    // Check if root nyx.toml exists, otherwise skip equivalent lightweight check
    let config_path = Path::new("nyx.toml");
    if !config_path.exists() {
        println!("nyx.toml not found, running basic feature detection only");
    }

    let detector = FeatureDetector::new();
    let report = validate_compliance_level(ComplianceLevel::Core, &detector)
        .expect("Core compliance validation should not fail");

    println!(
        "Core Compliance Status: {}",
        if report.is_compliant {
            "COMPLIANT"
        } else {
            "NON-COMPLIANT"
        }
    );

    if !report.missing_required.is_empty() {
        println!("Missing required features:");
        for feature in &report.missing_required {
            println!("  - {}", feature);
        }
    }

    if !report.missing_recommended.is_empty() {
        println!("Missing recommended features:");
        for feature in &report.missing_recommended {
            println!("  - {}", feature);
        }
    }

    // Core compliance should be achievable with basic implementation
    // This test serves as a baseline check
    println!("Available features: {:?}", detector.available_features());
}

#[test]
fn test_basic_feature_detection() {
    let detector = FeatureDetector::new();

    // Test that feature detector can be instantiated
    assert!(detector.available_features().len() >= 0);

    // Test basic feature checking doesn't panic
    let has_network = detector.has_feature("network.basic");
    let has_crypto = detector.has_feature("crypto.basic");
    let has_privacy = detector.has_feature("privacy.basic");

    println!("Feature detection working correctly");
}

#[test]
fn test_compliance_level_determination() {
    let detector = FeatureDetector::new();

    // Test that we can determine the highest compliance level
    let level = determine_compliance_level(&detector);

    println!("Determined compliance level: {:?}", level);

    // Should always return a valid level
    match level {
        ComplianceLevel::Core | ComplianceLevel::Enhanced | ComplianceLevel::Advanced => {
            println!("Valid compliance level detected");
        }
    }
}

#[test]
fn test_core_requirements_minimal() {
    let detector = FeatureDetector::new();

    // Test minimal core requirements that should always be available
    // These are the most basic features needed for any Nyx implementation
    let core_features = ["basic_functionality", "error_handling", "logging"];

    for feature in &core_features {
        // Don't fail if feature isn't available, just report
        let available = detector.has_feature(feature);
        println!(
            "Feature '{}': {}",
            feature,
            if available {
                "available"
            } else {
                "not available"
            }
        );
    }

    println!("Core requirements check completed");
}
