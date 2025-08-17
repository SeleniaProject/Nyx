//! Automated Compliance Matrix Validation for CI/CD
//!
//! This module provides automated compliance validation that can be integrated
//! into CI/CD pipelines to ensure protocol compliance at build time.

use nyx_core::compliance::*;
use serde_json;
use std::collections::HashMap;

/// CI/CD compliance check that fails if minimum requirements are not met
#[test]
fn ci_compliance_gate() {
    let detector = FeatureDetector::new();
    
    // Determine the highest achievable compliance level
    let achievable_level = determine_compliance_level(&detector);
    
    // Core compliance is mandatory - CI should fail if not achievable
    let core_report = validate_compliance_level(ComplianceLevel::Core, &detector).unwrap();
    assert!(core_report.is_compliant, 
        "CI GATE FAILURE: Core compliance not achieved. Missing: {:?}", 
        core_report.missing_required);
    
    println!("✅ CI GATE PASSED: Core compliance verified");
    println!("🎯 Highest achievable level: {}", achievable_level);
    
    // Generate compliance badge data
    let badge_data = generate_compliance_badge(&detector);
    println!("📊 Compliance badge data: {}", serde_json::to_string_pretty(&badge_data).unwrap());
}

/// Generate data for compliance badges and status indicators
pub fn generate_compliance_badge(detector: &FeatureDetector) -> serde_json::Value {
    let achievable_level = determine_compliance_level(&detector);
    
    let mut status = HashMap::new();
    
    // Check each compliance level
    for &level in &[ComplianceLevel::Core, ComplianceLevel::Plus, ComplianceLevel::Full] {
        let report = validate_compliance_level(level, detector).unwrap();
        
        let level_name = format!("{}", level).to_lowercase();
        status.insert(level_name.clone(), serde_json::json!({
            "compliant": report.is_compliant,
            "missing_required": report.missing_required,
            "missing_recommended": report.missing_recommended,
        }));
    }
    
    serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "highest_level": format!("{}", achievable_level).to_lowercase(),
        "levels": status,
        "available_features": detector.available_features().iter().collect::<Vec<_>>(),
    })
}

#[test]
fn test_compliance_matrix_comprehensive() {
    let detector = FeatureDetector::new();
    
    // Test all compliance levels
    let levels = [ComplianceLevel::Core, ComplianceLevel::Plus, ComplianceLevel::Full];
    
    for &level in &levels {
        let report = validate_compliance_level(level, &detector).unwrap();
        
        println!("\n=== {} Compliance Test ===", level);
        println!("Status: {}", if report.is_compliant { "✅ PASS" } else { "❌ FAIL" });
        
        if !report.missing_required.is_empty() {
            println!("Missing Required:");
            for feature in &report.missing_required {
                println!("  ❌ {}", feature);
            }
        }
        
        if !report.missing_recommended.is_empty() {
            println!("Missing Recommended:");
            for feature in &report.missing_recommended {
                println!("  ⚠️  {}", feature);
            }
        }
        
        // For CI purposes, log feature requirements
        let requirements = match level {
            ComplianceLevel::Core => ComplianceRequirements::core(),
            ComplianceLevel::Plus => ComplianceRequirements::plus(),
            ComplianceLevel::Full => ComplianceRequirements::full(),
        };
        
        println!("Required Features:");
        for feature in &requirements.required_features {
            let status = if detector.has_feature(feature) { "✅" } else { "❌" };
            println!("  {} {}", status, feature);
        }
    }
}

#[test]
fn test_feature_compilation_gates() {
    let detector = FeatureDetector::new();
    
    // Test compile-time feature gates
    println!("=== Compile-time Feature Detection ===");
    
    let expected_features = [
        ("stream", true),           // Always available
        ("frame_codec", true),      // Always available  
        ("flow_control", true),     // Always available
        ("basic_crypto", true),     // Always available
        ("capability_negotiation", true), // Implemented
        ("adaptive_cover_traffic", true), // Implemented
    ];
    
    for (feature_name, should_be_available) in expected_features.iter() {
        let is_available = detector.has_feature(feature_name);
        assert_eq!(is_available, *should_be_available,
            "Feature '{}' availability mismatch: expected {}, got {}",
            feature_name, should_be_available, is_available);
        
        println!("✅ {}: {}", feature_name, if is_available { "Available" } else { "Not Available" });
    }
    
    // Test conditional features
    #[cfg(feature = "multipath")]
    {
        assert!(detector.has_feature("multipath"));
        println!("✅ multipath: Available (feature enabled)");
    }
    #[cfg(not(feature = "multipath"))]
    {
        assert!(!detector.has_feature("multipath"));
        println!("⚠️  multipath: Not Available (feature disabled)");
    }
    
    #[cfg(feature = "hybrid")]
    {
        assert!(detector.has_feature("hybrid_pq"));
        println!("✅ hybrid_pq: Available (feature enabled)");
    }
    #[cfg(not(feature = "hybrid"))]
    {
        assert!(!detector.has_feature("hybrid_pq"));
        println!("⚠️  hybrid_pq: Not Available (feature disabled)");
    }
}

#[test]
fn test_compliance_regression_detection() {
    let detector = FeatureDetector::new();
    
    // This test ensures we don't regress on implemented features
    let implemented_features = [
        "stream",
        "frame_codec", 
        "flow_control",
        "basic_crypto",
        "congestion_control",
        "error_recovery",
        "capability_negotiation",
        "adaptive_cover_traffic",
    ];
    
    for feature in &implemented_features {
        assert!(detector.has_feature(feature),
            "REGRESSION: Previously implemented feature '{}' is no longer available", feature);
    }
    
    // Ensure Core compliance is always maintained
    let core_report = validate_compliance_level(ComplianceLevel::Core, &detector).unwrap();
    assert!(core_report.is_compliant,
        "REGRESSION: Core compliance lost. Missing: {:?}", core_report.missing_required);
    
    println!("✅ No compliance regression detected");
}

#[test]
fn test_compliance_level_progression() {
    let detector = FeatureDetector::new();
    
    // Test that compliance levels form a proper hierarchy
    let core_achievable = validate_compliance_level(ComplianceLevel::Core, &detector).unwrap().is_compliant;
    let plus_achievable = validate_compliance_level(ComplianceLevel::Plus, &detector).unwrap().is_compliant;
    let full_achievable = validate_compliance_level(ComplianceLevel::Full, &detector).unwrap().is_compliant;
    
    // If higher level is achievable, lower levels must also be achievable
    if full_achievable {
        assert!(plus_achievable, "If Full compliance is achievable, Plus must also be achievable");
        assert!(core_achievable, "If Full compliance is achievable, Core must also be achievable");
    }
    
    if plus_achievable {
        assert!(core_achievable, "If Plus compliance is achievable, Core must also be achievable");
    }
    
    println!("Compliance progression: Core={}, Plus={}, Full={}", 
        core_achievable, plus_achievable, full_achievable);
}
