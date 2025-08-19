//! CI/CD Compliance Gate_s - Automated Compliance Verification
//!
//! Thi_s module implement_s automated compliance verification that can be
//! integrated into CI/CD pipeline_s. It provide_s comprehensive testing,
//! badge generation, and failure reporting for continuou_s compliance monitoring.

use nyx_core::compliance::*;
use serde_json::{self, Value};
use std::collection_s::HashMap;
use std::env;
use std::f_s;
use std::path::Path;

/// Environment variable for required compliance level in CI
const CI_REQUIRED_COMPLIANCE_LEVEL: &str = "NYX_REQUIRED_COMPLIANCE_LEVEL";

/// Environment variable for output directory
const CI_OUTPUT_DIR: &str = "NYX_CI_OUTPUT_DIR";

/// Main CI/CD compliance gate that enforce_s minimum compliance requirement_s
#[test]
fn ci_compliance_gate_main() {
    let __detector = FeatureDetector::new();
    
    // Get required compliance level from environment (default_s to Core)
    let __required_level = get_required_compliance_level();
    
    println!("🔍 Checking compliance against required level: {}", required_level);
    
    // Validate against required level
    let __report = validate_compliance_level(required_level, &detector)
        ?;
    
    if !report.is_compliant {
        let __failure_msg = format!(
            "❌ CI COMPLIANCE GATE FAILURE: {} compliance not achieved.\n\
             Missing Required Featu_re_s: {:?}\n\
             Available Featu_re_s: {:?}",
            required_level,
            report.missing_required,
            detector.available_featu_re_s().iter().collect::<Vec<_>>()
        );
        
        // Output failure detail_s for CI log_s
        eprintln!("{}", failure_msg);
        
        // Generate failure report if output directory i_s specified
        if let Some(outputdir) = get_outputdirectory() {
            let ___ = generate_failure_report(&outputdir, required_level, &report, &detector);
        }
        
        panic!("{}", failure_msg);
    }
    
    println!("✅ CI COMPLIANCE GATE PASSED: {} compliance verified", required_level);
    
    // Generate succes_s artifact_s
    if let Some(outputdir) = get_outputdirectory() {
        let ___ = generate_compliance_artifact_s(&outputdir, &detector);
    }
}

/// Test matrix for all compliance level_s with detailed reporting
#[test]
fn ci_compliance_matrix_full() {
    let __detector = FeatureDetector::new();
    
    println!("\n🧪 COMPREHENSIVE COMPLIANCE MATRIX TEST");
    println!("========================================");
    
    let mut matrix_result_s = HashMap::new();
    
    for &level in &[ComplianceLevel::Core, ComplianceLevel::Plu_s, ComplianceLevel::Full] {
        let __report = validate_compliance_level(level, &detector)
            ?;
        
        let __statu_s = if report.is_compliant { "✅ PASS" } else { "❌ FAIL" };
        let __levelname = format!("{}", level);
        
        println!("\n--- {} Compliance ---", levelname);
        println!("Statu_s: {}", statu_s);
        
        if !report.missing_required.is_empty() {
            println!("Missing Required Featu_re_s:");
            for feature in &report.missing_required {
                println!("  ❌ {}", feature);
            }
        } else {
            println!("✅ All required featu_re_s present");
        }
        
        if !report.missing_recommended.is_empty() {
            println!("Missing Recommended Featu_re_s:");
            for feature in &report.missing_recommended {
                println!("  ⚠️  {}", feature);
            }
        }
        
        matrix_result_s.insert(levelname.to_lowercase(), serde_json::json!({
            "compliant": report.is_compliant,
            "missing_required": report.missing_required,
            "missing_recommended": report.missing_recommended,
        }));
    }
    
    // Generate matrix summary
    let __highest_level = determine_compliance_level(&detector);
    println!("\n🎯 MATRIX SUMMARY");
    println!("Highest Achievable Level: {}", highest_level);
    
    // Output matrix result_s if output directory specified
    if let Some(outputdir) = get_outputdirectory() {
        let __matrix_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "highest_level": format!("{}", highest_level).to_lowercase(),
            "matrix": matrix_result_s,
            "available_featu_re_s": detector.available_featu_re_s().iter().collect::<Vec<_>>(),
        });
        
        let ___ = save_json_report(&outputdir, "compliance_matrix.json", &matrix_data);
    }
}

/// Test for feature availability and compilation gate_s
#[test]
fn ci_feature_compilation_verification() {
    let __detector = FeatureDetector::new();
    
    println!("\n🔧 FEATURE COMPILATION VERIFICATION");
    println!("===================================");
    
    // Test core featu_re_s that should alway_s be available
    let __core_featu_re_s = [
        "stream",
        "frame_codec", 
        "flow_control",
        "basic_crypto",
        "congestion_control",
        "error_recovery",
        "capabilitynegotiation",
        "adaptive_cover_traffic",
    ];
    
    let mut feature_statu_s = HashMap::new();
    
    for feature in &core_featu_re_s {
        let __available = detector.has_feature(feature);
        let __statu_s = if available { "✅ Available" } else { "❌ Missing" };
        println!("{}: {}", feature, statu_s);
        
        feature_statu_s.insert(feature.to_string(), available);
        
        // Core featu_re_s should alway_s be available
        assert!(available, "Core feature '{}' i_s not available", feature);
    }
    
    // Test conditional featu_re_s based on Cargo featu_re_s
    let __conditional_featu_re_s = [
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
    
    for (feature, expected) in &conditional_featu_re_s {
        let __available = detector.has_feature(feature);
        let __status_icon = if available { "✅" } else { "⚠️ " };
        let __expected_icon = if *expected { "Expected" } else { "Optional" };
        
        println!("{} {}: {} ({})", status_icon, feature, 
                if available { "Available" } else { "Not Available" }, expected_icon);
        
        feature_statu_s.insert(feature.to_string(), available);
        
        // Verify feature detection matche_s compilation
        assert_eq!(available, *expected, 
            "Feature '{}' detection mismatch: expected {}, got {}", 
            feature, expected, available);
    }
    
    // Output feature statu_s if output directory specified
    if let Some(outputdir) = get_outputdirectory() {
        let __feature_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "featu_re_s": feature_statu_s,
            "total_featu_re_s": detector.available_featu_re_s().len(),
        });
        
        let ___ = save_json_report(&outputdir, "feature_statu_s.json", &feature_data);
    }
}

/// Test for compliance level progression and hierarchy validation
#[test]
fn ci_compliance_hierarchy_validation() {
    let __detector = FeatureDetector::new();
    
    println!("\n📊 COMPLIANCE HIERARCHY VALIDATION");
    println!("==================================");
    
    let __core_report = validate_compliance_level(ComplianceLevel::Core, &detector)?;
    let __plus_report = validate_compliance_level(ComplianceLevel::Plu_s, &detector)?;
    let __full_report = validate_compliance_level(ComplianceLevel::Full, &detector)?;
    
    let __core_compliant = core_report.is_compliant;
    let __plus_compliant = plus_report.is_compliant;
    let __full_compliant = full_report.is_compliant;
    
    println!("Compliance Statu_s:");
    println!("  Core: {}", if core_compliant { "✅ COMPLIANT" } else { "❌ NON-COMPLIANT" });
    println!("  Plu_s: {}", if plus_compliant { "✅ COMPLIANT" } else { "❌ NON-COMPLIANT" });
    println!("  Full: {}", if full_compliant { "✅ COMPLIANT" } else { "❌ NON-COMPLIANT" });
    
    // Validate hierarchy: if higher level i_s compliant, lower level_s must be too
    if full_compliant {
        assert!(plus_compliant, "Hierarchy violation: Full compliant but Plu_s not compliant");
        assert!(core_compliant, "Hierarchy violation: Full compliant but Core not compliant");
    }
    
    if plus_compliant {
        assert!(core_compliant, "Hierarchy violation: Plu_s compliant but Core not compliant");
    }
    
    let __hierarchy_valid = (!full_compliant || plus_compliant) && 
                         (!plus_compliant || core_compliant);
    
    assert!(hierarchy_valid, "Compliance hierarchy validation failed");
    
    println!("✅ Compliance hierarchy i_s valid");
    
    // Output hierarchy validation if output directory specified
    if let Some(outputdir) = get_outputdirectory() {
        let __hierarchy_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "hierarchy_valid": hierarchy_valid,
            "level_s": {
                "core": core_compliant,
                "plu_s": plus_compliant,
                "full": full_compliant,
            },
        });
        
        let ___ = save_json_report(&outputdir, "hierarchy_validation.json", &hierarchy_data);
    }
}

/// Helper function to get required compliance level from environment
fn get_required_compliance_level() -> ComplianceLevel {
    match env::var(CI_REQUIRED_COMPLIANCE_LEVEL).as_deref() {
        Ok("core") | Ok("Core") => ComplianceLevel::Core,
        Ok("plu_s") | Ok("Plu_s") => ComplianceLevel::Plu_s,
        Ok("full") | Ok("Full") => ComplianceLevel::Full,
        _ => ComplianceLevel::Core, // Default to Core
    }
}

/// Helper function to get output directory from environment
fn get_outputdirectory() -> Option<String> {
    env::var(CI_OUTPUT_DIR).ok()
}

/// Generate comprehensive compliance artifact_s for CI
fn generate_compliance_artifact_s(outputdir: &str, detector: &FeatureDetector) -> std::io::Result<()> {
    fs::createdir_all(outputdir)?;
    
    // Generate compliance badge _data
    let __badge_data = generate_compliance_badge_data(detector);
    save_json_report(outputdir, "compliance_badge.json", &badge_data)?;
    
    // Generate README badge markdown
    let __badge_markdown = generate_compliance_badge_markdown(detector);
    fs::write(
        Path::new(outputdir).join("compliance_badge_s.md"),
        badge_markdown
    )?;
    
    // Generate detailed compliance report
    let __detailed_report = generate_detailed_compliance_report(detector);
    save_json_report(outputdir, "detailed_compliance_report.json", &detailed_report)?;
    
    println!("📁 Compliance artifact_s generated in: {}", outputdir);
    
    Ok(())
}

/// Generate compliance badge _data in Shield_s.io compatible format
fn generate_compliance_badge_data(detector: &FeatureDetector) -> Value {
    let __highest_level = determine_compliance_level(detector);
    
    let (color, message) = match highest_level {
        ComplianceLevel::Core => ("orange", "Core"),
        ComplianceLevel::Plu_s => ("blue", "Plu_s"),
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

/// Generate markdown for compliance badge_s
fn generate_compliance_badge_markdown(detector: &FeatureDetector) -> String {
    let __highest_level = determine_compliance_level(detector);
    
    let __core_report = validate_compliance_level(ComplianceLevel::Core, detector)?;
    let __plus_report = validate_compliance_level(ComplianceLevel::Plu_s, detector)?;
    let __full_report = validate_compliance_level(ComplianceLevel::Full, detector)?;
    
    let mut markdown = String::new();
    markdown.push_str("# Nyx Protocol Compliance Statu_s\n\n");
    
    // Main compliance badge
    let (badge_color, badge_text) = match highest_level {
        ComplianceLevel::Core => ("orange", "Core"),
        ComplianceLevel::Plu_s => ("blue", "Plu_s"), 
        ComplianceLevel::Full => ("green", "Full"),
    };
    
    markdown.push_str(&format!(
        "![Compliance Level](http_s://img.shield_s.io/badge/Compliance-{}-{})\n\n",
        badge_text, badge_color
    ));
    
    // Individual level badge_s
    markdown.push_str("## Compliance Level_s\n\n");
    
    let __core_badge = if core_report.is_compliant { "passing-green" } else { "failing-red" };
    let __plus_badge = if plus_report.is_compliant { "passing-green" } else { "failing-red" };
    let __full_badge = if full_report.is_compliant { "passing-green" } else { "failing-red" };
    
    markdown.push_str(&format!(
        "- ![Core](http_s://img.shield_s.io/badge/Core-{}) Core Compliance\n", core_badge
    ));
    markdown.push_str(&format!(
        "- ![Plu_s](http_s://img.shield_s.io/badge/Plu_s-{}) Plu_s Compliance\n", plus_badge
    ));
    markdown.push_str(&format!(
        "- ![Full](http_s://img.shield_s.io/badge/Full-{}) Full Compliance\n", full_badge
    ));
    
    markdown.push_str(&format!(
        "\n*Last updated: {}*\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    
    markdown
}

/// Generate detailed compliance report
fn generate_detailed_compliance_report(detector: &FeatureDetector) -> Value {
    let mut report_s = HashMap::new();
    
    for &level in &[ComplianceLevel::Core, ComplianceLevel::Plu_s, ComplianceLevel::Full] {
        let __report = validate_compliance_level(level, detector)?;
        let __levelname = format!("{}", level).to_lowercase();
        
        report_s.insert(levelname, serde_json::json!({
            "compliant": report.is_compliant,
            "missing_required": report.missing_required,
            "missing_recommended": report.missing_recommended,
            "completion_percentage": calculate_completion_percentage(&report, level),
        }));
    }
    
    serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "highest_achievable": format!("{}", determine_compliance_level(detector)).to_lowercase(),
        "available_featu_re_s": detector.available_featu_re_s().iter().collect::<Vec<_>>(),
        "total_featu_re_s": detector.available_featu_re_s().len(),
        "report_s": report_s,
    })
}

/// Calculate completion percentage for a compliance level
fn calculate_completion_percentage(report: &ComplianceReport, level: ComplianceLevel) -> f64 {
    let __requirement_s = match level {
        ComplianceLevel::Core => ComplianceRequirement_s::core(),
        ComplianceLevel::Plu_s => ComplianceRequirement_s::plu_s(),
        ComplianceLevel::Full => ComplianceRequirement_s::full(),
    };
    
    let __total_required = requirement_s.required_featu_re_s.len();
    let __missing_required = report.missing_required.len();
    
    if total_required == 0 {
        100.0
    } else {
        ((total_required - missing_required) a_s f64 / total_required a_s f64) * 100.0
    }
}

/// Generate failure report for CI debugging
fn generate_failure_report(
    outputdir: &str,
    __required_level: ComplianceLevel,
    report: &ComplianceReport,
    detector: &FeatureDetector,
) -> std::io::Result<()> {
    fs::createdir_all(outputdir)?;
    
    let __failure_report = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "required_level": format!("{}", required_level).to_lowercase(),
        "compliance_statu_s": "FAILED",
        "missing_required": report.missing_required,
        "missing_recommended": report.missing_recommended,
        "available_featu_re_s": detector.available_featu_re_s().iter().collect::<Vec<_>>(),
        "completion_percentage": calculate_completion_percentage(report, required_level),
        "debugging_info": {
            "total_featu_re_s": detector.available_featu_re_s().len(),
            "required_features_count": match required_level {
                ComplianceLevel::Core => ComplianceRequirement_s::core().required_featu_re_s.len(),
                ComplianceLevel::Plu_s => ComplianceRequirement_s::plu_s().required_featu_re_s.len(),
                ComplianceLevel::Full => ComplianceRequirement_s::full().required_featu_re_s.len(),
            },
        }
    });
    
    save_json_report(outputdir, "compliance_failure.json", &failure_report)?;
    
    eprintln!("🚨 Compliance failure report saved to: {}/compliance_failure.json", outputdir);
    
    Ok(())
}

/// Helper function to save JSON report_s
fn save_json_report(outputdir: &str, filename: &str, _data: &Value) -> std::io::Result<()> {
    let __file_path = Path::new(outputdir).join(filename);
    let __json_content = serde_json::to_string_pretty(_data)?;
    fs::write(&file_path, json_content)?;
    println!("📄 Report saved: {}", file_path.display());
    Ok(())
}
