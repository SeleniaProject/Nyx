//! CI/CD Compliance Gates - Automated Compliance Verification
//!
//! This module implements automated compliance verification that can be
//! integrated into CI/CD pipelines. It provides comprehensive testing,
//! badge generation, and failure reporting for continuous compliance monitoring.

use nyx_core::compliance::*;
use serde_json::{self, Value};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

/// Environment variable for required compliance level in CI
const CI_REQUIRED_COMPLIANCE_LEVEL: &str = "NYX_REQUIRED_COMPLIANCE_LEVEL";

/// Environment variable for output directory
const CI_OUTPUT_DIR: &str = "NYX_CI_OUTPUT_DIR";

/// Main CI/CD compliance gate that enforces minimum compliance requirements
#[test]
fn ci_compliance_gate_main() {
    let detector = FeatureDetector::new();
    
    // Get required compliance level from environment (defaults to Core)
    let required_level = get_required_compliance_level();
    
    println!("🔍 Checking compliance against required level: {}", required_level);
    
    // Validate against required level
    let report = validate_compliance_level(required_level, &detector)
        .expect("Failed to validate compliance");
    
    if !report.is_compliant {
        let failure_msg = format!(
            "❌ CI COMPLIANCE GATE FAILURE: {} compliance not achieved.\n\
             Missing Required Features: {:?}\n\
             Available Features: {:?}",
            required_level,
            report.missing_required,
            detector.available_features().iter().collect::<Vec<_>>()
        );
        
        // Output failure details for CI logs
        eprintln!("{}", failure_msg);
        
        // Generate failure report if output directory is specified
        if let Some(output_dir) = get_output_directory() {
            let _ = generate_failure_report(&output_dir, required_level, &report, &detector);
        }
        
        panic!("{}", failure_msg);
    }
    
    println!("✅ CI COMPLIANCE GATE PASSED: {} compliance verified", required_level);
    
    // Generate success artifacts
    if let Some(output_dir) = get_output_directory() {
        let _ = generate_compliance_artifacts(&output_dir, &detector);
    }
}

/// Test matrix for all compliance levels with detailed reporting
#[test]
fn ci_compliance_matrix_full() {
    let detector = FeatureDetector::new();
    
    println!("\n🧪 COMPREHENSIVE COMPLIANCE MATRIX TEST");
    println!("========================================");
    
    let mut matrix_results = HashMap::new();
    
    for &level in &[ComplianceLevel::Core, ComplianceLevel::Plus, ComplianceLevel::Full] {
        let report = validate_compliance_level(level, &detector)
            .expect("Failed to validate compliance");
        
        let status = if report.is_compliant { "✅ PASS" } else { "❌ FAIL" };
        let level_name = format!("{}", level);
        
        println!("\n--- {} Compliance ---", level_name);
        println!("Status: {}", status);
        
        if !report.missing_required.is_empty() {
            println!("Missing Required Features:");
            for feature in &report.missing_required {
                println!("  ❌ {}", feature);
            }
        } else {
            println!("✅ All required features present");
        }
        
        if !report.missing_recommended.is_empty() {
            println!("Missing Recommended Features:");
            for feature in &report.missing_recommended {
                println!("  ⚠️  {}", feature);
            }
        }
        
        matrix_results.insert(level_name.to_lowercase(), serde_json::json!({
            "compliant": report.is_compliant,
            "missing_required": report.missing_required,
            "missing_recommended": report.missing_recommended,
        }));
    }
    
    // Generate matrix summary
    let highest_level = determine_compliance_level(&detector);
    println!("\n🎯 MATRIX SUMMARY");
    println!("Highest Achievable Level: {}", highest_level);
    
    // Output matrix results if output directory specified
    if let Some(output_dir) = get_output_directory() {
        let matrix_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "highest_level": format!("{}", highest_level).to_lowercase(),
            "matrix": matrix_results,
            "available_features": detector.available_features().iter().collect::<Vec<_>>(),
        });
        
        let _ = save_json_report(&output_dir, "compliance_matrix.json", &matrix_data);
    }
}

/// Test for feature availability and compilation gates
#[test]
fn ci_feature_compilation_verification() {
    let detector = FeatureDetector::new();
    
    println!("\n🔧 FEATURE COMPILATION VERIFICATION");
    println!("===================================");
    
    // Test core features that should always be available
    let core_features = [
        "stream",
        "frame_codec", 
        "flow_control",
        "basic_crypto",
        "congestion_control",
        "error_recovery",
        "capability_negotiation",
        "adaptive_cover_traffic",
    ];
    
    let mut feature_status = HashMap::new();
    
    for feature in &core_features {
        let available = detector.has_feature(feature);
        let status = if available { "✅ Available" } else { "❌ Missing" };
        println!("{}: {}", feature, status);
        
        feature_status.insert(feature.to_string(), available);
        
        // Core features should always be available
        assert!(available, "Core feature '{}' is not available", feature);
    }
    
    // Test conditional features based on Cargo features
    let conditional_features = [
        ("multipath", cfg!(feature = "multipath")),
        ("hybrid_pq", cfg!(feature = "hybrid")),
        ("telemetry", cfg!(feature = "telemetry")),
        ("fec", cfg!(feature = "fec")),
        ("cmix", cfg!(all(feature = "cmix", feature = "vdf"))),
        ("vdf", cfg!(feature = "vdf")),
        ("plugin_framework", cfg!(feature = "plugin")),
        ("low_power_mode", cfg!(feature = "mobile")),
        ("quic_transport", cfg!(feature = "quic")),
        ("advanced_telemetry", cfg!(all(feature = "telemetry", feature = "otlp"))),
    ];
    
    for (feature, expected) in &conditional_features {
        let available = detector.has_feature(feature);
        let status_icon = if available { "✅" } else { "⚠️ " };
        let expected_icon = if *expected { "Expected" } else { "Optional" };
        
        println!("{} {}: {} ({})", status_icon, feature, 
                if available { "Available" } else { "Not Available" }, expected_icon);
        
        feature_status.insert(feature.to_string(), available);
        
        // Verify feature detection matches compilation
        assert_eq!(available, *expected, 
            "Feature '{}' detection mismatch: expected {}, got {}", 
            feature, expected, available);
    }
    
    // Output feature status if output directory specified
    if let Some(output_dir) = get_output_directory() {
        let feature_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "features": feature_status,
            "total_features": detector.available_features().len(),
        });
        
        let _ = save_json_report(&output_dir, "feature_status.json", &feature_data);
    }
}

/// Test for compliance level progression and hierarchy validation
#[test]
fn ci_compliance_hierarchy_validation() {
    let detector = FeatureDetector::new();
    
    println!("\n📊 COMPLIANCE HIERARCHY VALIDATION");
    println!("==================================");
    
    let core_report = validate_compliance_level(ComplianceLevel::Core, &detector).unwrap();
    let plus_report = validate_compliance_level(ComplianceLevel::Plus, &detector).unwrap();
    let full_report = validate_compliance_level(ComplianceLevel::Full, &detector).unwrap();
    
    let core_compliant = core_report.is_compliant;
    let plus_compliant = plus_report.is_compliant;
    let full_compliant = full_report.is_compliant;
    
    println!("Compliance Status:");
    println!("  Core: {}", if core_compliant { "✅ COMPLIANT" } else { "❌ NON-COMPLIANT" });
    println!("  Plus: {}", if plus_compliant { "✅ COMPLIANT" } else { "❌ NON-COMPLIANT" });
    println!("  Full: {}", if full_compliant { "✅ COMPLIANT" } else { "❌ NON-COMPLIANT" });
    
    // Validate hierarchy: if higher level is compliant, lower levels must be too
    if full_compliant {
        assert!(plus_compliant, "Hierarchy violation: Full compliant but Plus not compliant");
        assert!(core_compliant, "Hierarchy violation: Full compliant but Core not compliant");
    }
    
    if plus_compliant {
        assert!(core_compliant, "Hierarchy violation: Plus compliant but Core not compliant");
    }
    
    let hierarchy_valid = (!full_compliant || plus_compliant) && 
                         (!plus_compliant || core_compliant);
    
    assert!(hierarchy_valid, "Compliance hierarchy validation failed");
    
    println!("✅ Compliance hierarchy is valid");
    
    // Output hierarchy validation if output directory specified
    if let Some(output_dir) = get_output_directory() {
        let hierarchy_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "hierarchy_valid": hierarchy_valid,
            "levels": {
                "core": core_compliant,
                "plus": plus_compliant,
                "full": full_compliant,
            },
        });
        
        let _ = save_json_report(&output_dir, "hierarchy_validation.json", &hierarchy_data);
    }
}

/// Helper function to get required compliance level from environment
fn get_required_compliance_level() -> ComplianceLevel {
    match env::var(CI_REQUIRED_COMPLIANCE_LEVEL).as_deref() {
        Ok("core") | Ok("Core") => ComplianceLevel::Core,
        Ok("plus") | Ok("Plus") => ComplianceLevel::Plus,
        Ok("full") | Ok("Full") => ComplianceLevel::Full,
        _ => ComplianceLevel::Core, // Default to Core
    }
}

/// Helper function to get output directory from environment
fn get_output_directory() -> Option<String> {
    env::var(CI_OUTPUT_DIR).ok()
}

/// Generate comprehensive compliance artifacts for CI
fn generate_compliance_artifacts(output_dir: &str, detector: &FeatureDetector) -> std::io::Result<()> {
    fs::create_dir_all(output_dir)?;
    
    // Generate compliance badge data
    let badge_data = generate_compliance_badge_data(detector);
    save_json_report(output_dir, "compliance_badge.json", &badge_data)?;
    
    // Generate README badge markdown
    let badge_markdown = generate_compliance_badge_markdown(detector);
    fs::write(
        Path::new(output_dir).join("compliance_badges.md"),
        badge_markdown
    )?;
    
    // Generate detailed compliance report
    let detailed_report = generate_detailed_compliance_report(detector);
    save_json_report(output_dir, "detailed_compliance_report.json", &detailed_report)?;
    
    println!("📁 Compliance artifacts generated in: {}", output_dir);
    
    Ok(())
}

/// Generate compliance badge data in Shields.io compatible format
fn generate_compliance_badge_data(detector: &FeatureDetector) -> Value {
    let highest_level = determine_compliance_level(detector);
    
    let (color, message) = match highest_level {
        ComplianceLevel::Core => ("orange", "Core"),
        ComplianceLevel::Plus => ("blue", "Plus"),
        ComplianceLevel::Full => ("green", "Full"),
    };
    
    serde_json::json!({
        "schemaVersion": 1,
        "label": "Nyx Compliance",
        "message": message,
        "color": color,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })
}

/// Generate markdown for compliance badges
fn generate_compliance_badge_markdown(detector: &FeatureDetector) -> String {
    let highest_level = determine_compliance_level(detector);
    
    let core_report = validate_compliance_level(ComplianceLevel::Core, detector).unwrap();
    let plus_report = validate_compliance_level(ComplianceLevel::Plus, detector).unwrap();
    let full_report = validate_compliance_level(ComplianceLevel::Full, detector).unwrap();
    
    let mut markdown = String::new();
    markdown.push_str("# Nyx Protocol Compliance Status\n\n");
    
    // Main compliance badge
    let (badge_color, badge_text) = match highest_level {
        ComplianceLevel::Core => ("orange", "Core"),
        ComplianceLevel::Plus => ("blue", "Plus"), 
        ComplianceLevel::Full => ("green", "Full"),
    };
    
    markdown.push_str(&format!(
        "![Compliance Level](https://img.shields.io/badge/Compliance-{}-{})\n\n",
        badge_text, badge_color
    ));
    
    // Individual level badges
    markdown.push_str("## Compliance Levels\n\n");
    
    let core_badge = if core_report.is_compliant { "passing-green" } else { "failing-red" };
    let plus_badge = if plus_report.is_compliant { "passing-green" } else { "failing-red" };
    let full_badge = if full_report.is_compliant { "passing-green" } else { "failing-red" };
    
    markdown.push_str(&format!(
        "- ![Core](https://img.shields.io/badge/Core-{}) Core Compliance\n", core_badge
    ));
    markdown.push_str(&format!(
        "- ![Plus](https://img.shields.io/badge/Plus-{}) Plus Compliance\n", plus_badge
    ));
    markdown.push_str(&format!(
        "- ![Full](https://img.shields.io/badge/Full-{}) Full Compliance\n", full_badge
    ));
    
    markdown.push_str(&format!(
        "\n*Last updated: {}*\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    
    markdown
}

/// Generate detailed compliance report
fn generate_detailed_compliance_report(detector: &FeatureDetector) -> Value {
    let mut reports = HashMap::new();
    
    for &level in &[ComplianceLevel::Core, ComplianceLevel::Plus, ComplianceLevel::Full] {
        let report = validate_compliance_level(level, detector).unwrap();
        let level_name = format!("{}", level).to_lowercase();
        
        reports.insert(level_name, serde_json::json!({
            "compliant": report.is_compliant,
            "missing_required": report.missing_required,
            "missing_recommended": report.missing_recommended,
            "completion_percentage": calculate_completion_percentage(&report, level),
        }));
    }
    
    serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "highest_achievable": format!("{}", determine_compliance_level(detector)).to_lowercase(),
        "available_features": detector.available_features().iter().collect::<Vec<_>>(),
        "total_features": detector.available_features().len(),
        "reports": reports,
    })
}

/// Calculate completion percentage for a compliance level
fn calculate_completion_percentage(report: &ComplianceReport, level: ComplianceLevel) -> f64 {
    let requirements = match level {
        ComplianceLevel::Core => ComplianceRequirements::core(),
        ComplianceLevel::Plus => ComplianceRequirements::plus(),
        ComplianceLevel::Full => ComplianceRequirements::full(),
    };
    
    let total_required = requirements.required_features.len();
    let missing_required = report.missing_required.len();
    
    if total_required == 0 {
        100.0
    } else {
        ((total_required - missing_required) as f64 / total_required as f64) * 100.0
    }
}

/// Generate failure report for CI debugging
fn generate_failure_report(
    output_dir: &str,
    required_level: ComplianceLevel,
    report: &ComplianceReport,
    detector: &FeatureDetector,
) -> std::io::Result<()> {
    fs::create_dir_all(output_dir)?;
    
    let failure_report = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "required_level": format!("{}", required_level).to_lowercase(),
        "compliance_status": "FAILED",
        "missing_required": report.missing_required,
        "missing_recommended": report.missing_recommended,
        "available_features": detector.available_features().iter().collect::<Vec<_>>(),
        "completion_percentage": calculate_completion_percentage(report, required_level),
        "debugging_info": {
            "total_features": detector.available_features().len(),
            "required_features_count": match required_level {
                ComplianceLevel::Core => ComplianceRequirements::core().required_features.len(),
                ComplianceLevel::Plus => ComplianceRequirements::plus().required_features.len(),
                ComplianceLevel::Full => ComplianceRequirements::full().required_features.len(),
            },
        }
    });
    
    save_json_report(output_dir, "compliance_failure.json", &failure_report)?;
    
    eprintln!("🚨 Compliance failure report saved to: {}/compliance_failure.json", output_dir);
    
    Ok(())
}

/// Helper function to save JSON reports
fn save_json_report(output_dir: &str, filename: &str, data: &Value) -> std::io::Result<()> {
    let file_path = Path::new(output_dir).join(filename);
    let json_content = serde_json::to_string_pretty(data)?;
    fs::write(&file_path, json_content)?;
    println!("📄 Report saved: {}", file_path.display());
    Ok(())
}
