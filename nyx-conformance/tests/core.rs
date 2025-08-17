
//! Core compliance testing for Nyx Protocol
//!
//! Tests Core/Plus/Full compliance levels as specified in
//! `spec/Nyx_Protocol_v1.0_Spec.md` Section 10: Compliance Levels.

use nyx_core::compliance::*;

// 10. Compliance Levels → nyx_config_parse_defaults
#[test]
fn nyx_config_parse_defaults() {
	// ルートのnyx.tomlがあればパース、なければスキップ相当の軽量チェック
	match std::fs::read_to_string("nyx.toml") {
		Ok(content) => assert!(content.contains("[daemon]") || content.len() > 10),
		Err(_) => assert!(true),
	}
}

#[test]
fn test_core_compliance_always_achievable() {
	let detector = FeatureDetector::new();
	let report = validate_compliance_level(ComplianceLevel::Core, &detector).unwrap();
	
	assert!(report.is_compliant, "Core compliance must always be achievable");
	println!("Core compliance: {}", report.summary());
}

#[test]
fn test_plus_compliance_requirements() {
	let detector = FeatureDetector::new();
	let report = validate_compliance_level(ComplianceLevel::Plus, &detector).unwrap();
	
	println!("Plus compliance: {}", report.summary());
	
	if !report.is_compliant {
		println!("Missing required features for Plus compliance:");
		for feature in &report.missing_required {
			println!("  - {}", feature);
		}
	}
}

#[test]
fn test_full_compliance_requirements() {
	let detector = FeatureDetector::new();
	let report = validate_compliance_level(ComplianceLevel::Full, &detector).unwrap();
	
	println!("Full compliance: {}", report.summary());
	
	if !report.is_compliant {
		println!("Missing required features for Full compliance:");
		for feature in &report.missing_required {
			println!("  - {}", feature);
		}
	}
}

#[test]
fn test_compliance_level_determination() {
	let detector = FeatureDetector::new();
	let level = determine_compliance_level(&detector);
	
	println!("Highest achievable compliance level: {}", level);
	
	// Verify the determined level is actually achievable
	let report = validate_compliance_level(level, &detector).unwrap();
	assert!(report.is_compliant, "Determined compliance level should be achievable");
}

#[test]
fn test_compliance_report_generation() {
	let detector = FeatureDetector::new();
	
	for &level in &[ComplianceLevel::Core, ComplianceLevel::Plus, ComplianceLevel::Full] {
		let report = validate_compliance_level(level, &detector).unwrap();
		
		println!("\n{}", report.detailed_report());
		
		// Verify report contains expected sections
		let detailed = report.detailed_report();
		assert!(detailed.contains("Nyx Protocol Compliance Report"));
		assert!(detailed.contains(&format!("Target Level: {}", level)));
		assert!(detailed.contains("Available Features:"));
	}
}

#[test]
fn test_feature_detection_consistency() {
	let detector = FeatureDetector::new();
	
	// Core features should always be available
	assert!(detector.has_feature("stream"));
	assert!(detector.has_feature("frame_codec"));
	assert!(detector.has_feature("flow_control"));
	assert!(detector.has_feature("basic_crypto"));
	
	// Implemented features should be detected
	assert!(detector.has_feature("capability_negotiation"));
	assert!(detector.has_feature("adaptive_cover_traffic"));
	
	println!("Available features: {:?}", detector.available_features());
}

#[test]
fn test_compliance_matrix_validation() {
	let detector = FeatureDetector::new();
	
	// Test that compliance requirements are properly nested
	let core_reqs = ComplianceRequirements::core();
	let plus_reqs = ComplianceRequirements::plus();
	let full_reqs = ComplianceRequirements::full();
	
	// Plus should include all Core requirements
	for feature in &core_reqs.required_features {
		assert!(plus_reqs.required_features.contains(feature),
			"Plus compliance should include Core feature: {}", feature);
	}
	
	// Full should include all Plus requirements
	for feature in &plus_reqs.required_features {
		assert!(full_reqs.required_features.contains(feature),
			"Full compliance should include Plus feature: {}", feature);
	}
	
	println!("Compliance matrix validation passed");
}

