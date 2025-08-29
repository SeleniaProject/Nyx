//! Nyx Protocol Compliance Level Detection and Validation
//!
//! This module implements the compliance level detection system as specified in
//! `spec/Nyx_Protocol_v1.0_Spec.md` Section 10: Compliance Levels.
//!
//! ## Compliance Levels
//!
//! - **Core**: Minimum compatibility (v0.1 feature set)
//! - **Plu_s**: Multipath, Hybrid Post-Quantum (recommended)
#![cfg_attr(test, allow(clippy::unwrap_used))]
//! - **Full**: cMix, Plugin Framework, Low Power Mode (all Features)

use crate::config::CoreConfig;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Compliance levels as defined in the Nyx protocol specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ComplianceLevel {
    /// Core compliance: minimum feature set for basic interoperability
    Core,
    /// Plus compliance: includes multipath and hybrid post-quantum features  
    Plus,
    /// Full compliance: all protocol features including cMix and plugins
    Full,
}

impl std::fmt::Display for ComplianceLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComplianceLevel::Core => write!(f, "Core"),
            ComplianceLevel::Plus => write!(f, "Plus"),
            ComplianceLevel::Full => write!(f, "Full"),
        }
    }
}

/// Required Features for each compliance level
#[derive(Debug, Clone)]
pub struct ComplianceRequirements {
    /// Features that must be present for this compliance level
    pub required_features: HashSet<String>,
    /// Features that are recommended but not required
    pub recommended_features: HashSet<String>,
}

impl ComplianceRequirements {
    /// Get requirements for Core compliance level
    pub fn core() -> Self {
        let mut required = HashSet::new();
        required.insert("stream".to_string());
        required.insert("frame_codec".to_string());
        required.insert("flow_control".to_string());
        required.insert("basic_crypto".to_string());

        let mut recommended = HashSet::new();
        recommended.insert("congestion_control".to_string());
        recommended.insert("error_recovery".to_string());

        Self {
            required_features: required,
            recommended_features: recommended,
        }
    }

    /// Get requirements for Plus compliance level
    pub fn plus() -> Self {
        let mut required = ComplianceRequirements::core().required_features;
        required.insert("multipath".to_string());
        required.insert("hybrid_pq".to_string());
        required.insert("capabilitynegotiation".to_string());

        let mut recommended = HashSet::new();
        recommended.insert("adaptive_cover_traffic".to_string());
        recommended.insert("telemetry".to_string());
        recommended.insert("fec".to_string());

        Self {
            required_features: required,
            recommended_features: recommended,
        }
    }

    /// Get requirements for Full compliance level
    pub fn full() -> Self {
        let mut required = ComplianceRequirements::plus().required_features;
        required.insert("cmix".to_string());
        required.insert("plugin_framework".to_string());
        required.insert("low_power_mode".to_string());
        required.insert("vdf".to_string());

        let mut recommended = HashSet::new();
        recommended.insert("quic_transport".to_string());
        recommended.insert("nat_traversal".to_string());
        recommended.insert("advanced_telemetry".to_string());

        Self {
            required_features: required,
            recommended_features: recommended,
        }
    }
}

/// Feature detection based on compile-time Cargo features
pub struct FeatureDetector {
    available_features: HashSet<String>,
}

impl FeatureDetector {
    /// Create a new feature detector with compile-time feature detection
    pub fn new() -> Self {
        let mut features = HashSet::new();

        // Core features - always available
        features.insert("stream".to_string());
        features.insert("frame_codec".to_string());
        features.insert("flow_control".to_string());
        features.insert("basic_crypto".to_string());
        features.insert("congestion_control".to_string());
        features.insert("error_recovery".to_string());

        // Multipath feature
        #[cfg(feature = "multipath")]
        features.insert("multipath".to_string());

        // Hybrid Post-Quantum feature
        #[cfg(feature = "hybrid")]
        features.insert("hybrid_pq".to_string());

        // Capability negotiation - implemented
        features.insert("capabilitynegotiation".to_string());

        // Adaptive cover traffic - implemented
        features.insert("adaptive_cover_traffic".to_string());

        // Telemetry feature
        #[cfg(feature = "telemetry")]
        features.insert("telemetry".to_string());

        // FEC feature
        #[cfg(feature = "fec")]
        features.insert("fec".to_string());

        // cMix feature - check if VDF is implemented
        #[cfg(all(feature = "cmix", feature = "vdf"))]
        features.insert("cmix".to_string());

        // VDF feature
        #[cfg(feature = "vdf")]
        features.insert("vdf".to_string());

        // Plugin framework feature
        #[cfg(feature = "plugin")]
        features.insert("plugin_framework".to_string());

        // Low power mode feature
        #[cfg(feature = "mobile")]
        features.insert("low_power_mode".to_string());

        // QUIC transport feature
        #[cfg(feature = "quic")]
        features.insert("quic_transport".to_string());

        // NAT traversal feature
        #[cfg(feature = "nat_traversal")]
        features.insert("nat_traversal".to_string());

        // Advanced telemetry feature
        #[cfg(all(feature = "telemetry", feature = "otlp"))]
        features.insert("advanced_telemetry".to_string());

        Self {
            available_features: features,
        }
    }

    /// Get all available features
    pub fn available_features(&self) -> &HashSet<String> {
        &self.available_features
    }

    /// Check if a specific feature i_s available
    pub fn has_feature(&self, feature: &str) -> bool {
        self.available_features.contains(feature)
    }
}

impl Default for FeatureDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Determine the highest compliance level achievable with current features
pub fn determine_compliance_level(detector: &FeatureDetector) -> ComplianceLevel {
    let full_reqs = ComplianceRequirements::full();
    if full_reqs
        .required_features
        .iter()
        .all(|f| detector.has_feature(f))
    {
        return ComplianceLevel::Full;
    }

    let plus_reqs = ComplianceRequirements::plus();
    if plus_reqs
        .required_features
        .iter()
        .all(|f| detector.has_feature(f))
    {
        return ComplianceLevel::Plus;
    }

    // Core compliance should always be achievable
    ComplianceLevel::Core
}

/// Validate compliance level against requirements
pub fn validate_compliance_level(
    level: ComplianceLevel,
    detector: &FeatureDetector,
) -> Result<ComplianceReport> {
    let requirements = match level {
        ComplianceLevel::Core => ComplianceRequirements::core(),
        ComplianceLevel::Plus => ComplianceRequirements::plus(),
        ComplianceLevel::Full => ComplianceRequirements::full(),
    };

    let mut missing_required = Vec::new();
    let mut missing_recommended = Vec::new();

    // Check required Features
    for feature in &requirements.required_features {
        if !detector.has_feature(feature) {
            missing_required.push(feature.to_string());
        }
    }

    // Check recommended Features
    for feature in &requirements.recommended_features {
        if !detector.has_feature(feature) {
            missing_recommended.push(feature.to_string());
        }
    }

    let is_compliant = missing_required.is_empty();

    Ok(ComplianceReport {
        level,
        is_compliant,
        missing_required,
        missing_recommended,
        available_features: detector.available_features().clone(),
    })
}

/// Compliance validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    /// Target compliance level
    pub level: ComplianceLevel,
    /// Whether the target level is achieved
    pub is_compliant: bool,
    /// Required features that are missing
    pub missing_required: Vec<String>,
    /// Recommended features that are missing
    pub missing_recommended: Vec<String>,
    /// All available features
    pub available_features: HashSet<String>,
}

impl ComplianceReport {
    /// Get a human-readable summary of the compliance status
    pub fn summary(&self) -> String {
        if self.is_compliant {
            format!("✓ {} compliance achieved", self.level)
        } else {
            format!(
                "✗ {} compliance failed - missing required features: {}",
                self.level,
                self.missing_required.join(", ")
            )
        }
    }

    /// Generate detailed compliance report
    pub fn detailed_report(&self) -> String {
        let mut report = String::new();

        report.push_str("Nyx Protocol Compliance Report\n");
        report.push_str("=============================\n\n");
        report.push_str(&format!("Target Level: {}\n", self.level));
        report.push_str(&format!(
            "Status: {}\n\n",
            if self.is_compliant {
                "COMPLIANT"
            } else {
                "NON-COMPLIANT"
            }
        ));

        if !self.missing_required.is_empty() {
            report.push_str("Missing Required Features:\n");
            for feature in &self.missing_required {
                report.push_str(&format!("  - {feature}\n"));
            }
            report.push('\n');
        }

        if !self.missing_recommended.is_empty() {
            report.push_str("Missing Recommended Features:\n");
            for feature in &self.missing_recommended {
                report.push_str(&format!("  - {feature}\n"));
            }
            report.push('\n');
        }

        report.push_str("Available Features:\n");
        let mut sorted_features: Vec<_> = self.available_features.iter().collect();
        sorted_features.sort();
        for feature in sorted_features {
            report.push_str(&format!("  + {feature}\n"));
        }

        report
    }
}

/// Simple policy describing allowed configuration combinations.
#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub allow_trace_logs: bool,
    pub allow_multipath: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            allow_trace_logs: false,
            allow_multipath: true,
        }
    }
}

/// Validate a configuration against a policy.
pub fn validate_against(cfg: &CoreConfig, pol: Policy) -> Result<()> {
    if !pol.allow_trace_logs && cfg.log_level == "trace" {
        return Err(Error::config("trace logs are disallowed by policy"));
    }
    if cfg.enable_multipath && !pol.allow_multipath {
        return Err(Error::config("multipath is disallowed by policy"));
    }
    Ok(())
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn test_feature_detector() {
        let detector = FeatureDetector::new();

        // Core features should always be available
        assert!(detector.has_feature("stream"));
        assert!(detector.has_feature("frame_codec"));
        assert!(detector.has_feature("flow_control"));
        assert!(detector.has_feature("basic_crypto"));
    }

    #[test]
    fn test_compliance_level_determination() {
        let detector = FeatureDetector::new();
        let level = determine_compliance_level(&detector);

        // Should at least achieve Core compliance
        assert!(level >= ComplianceLevel::Core);
    }

    #[test]
    fn test_core_compliance_validation() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FeatureDetector::new();
        let report = validate_compliance_level(ComplianceLevel::Core, &detector)?;

        // Core compliance should always be achievable
        assert!(
            report.is_compliant,
            "Core compliance should always be achievable"
        );
        assert!(report.missing_required.is_empty());
        Ok(())
    }

    #[test]
    fn test_compliance_report_summary() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FeatureDetector::new();
        let report = validate_compliance_level(ComplianceLevel::Core, &detector)?;

        let summary = report.summary();
        assert!(summary.contains("Core"));
        assert!(
            summary.contains("✓")
                || summary.contains("✗")
                || summary.contains("available")
                || summary.contains("missing")
        );
        Ok(())
    }

    #[test]
    fn test_compliance_requirements() {
        let core_reqs = ComplianceRequirements::core();
        assert!(core_reqs.required_features.contains("stream"));
        assert!(core_reqs.required_features.contains("basic_crypto"));

        let plus_reqs = ComplianceRequirements::plus();
        assert!(plus_reqs.required_features.contains("multipath"));
        assert!(plus_reqs.required_features.contains("hybrid_pq"));

        let full_reqs = ComplianceRequirements::full();
        assert!(full_reqs.required_features.contains("cmix"));
        assert!(full_reqs.required_features.contains("plugin_framework"));
    }

    #[test]
    fn test_policy_blocks_trace() {
        let cfg = CoreConfig {
            log_level: "trace".into(),
            ..CoreConfig::default()
        };
        let e = validate_against(
            &cfg,
            Policy {
                allow_trace_logs: false,
                allow_multipath: true,
            },
        )
        .unwrap_err();
        assert!(format!("{e}").contains("disallowed"));
    }

    #[test]
    fn test_compliance_level_ordering() {
        assert!(ComplianceLevel::Core < ComplianceLevel::Plus);
        assert!(ComplianceLevel::Plus < ComplianceLevel::Full);
    }

    #[test]
    fn test_detailed_compliance_report() -> Result<(), Box<dyn std::error::Error>> {
        let detector = FeatureDetector::new();
        let report = validate_compliance_level(ComplianceLevel::Full, &detector)?;

        let detailed = report.detailed_report();
        assert!(detailed.contains("Nyx Protocol Compliance Report"));
        assert!(detailed.contains("Target Level: Full"));
        assert!(detailed.contains("Available Features:"));
        Ok(())
    }
}
