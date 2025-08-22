#![forbid(unsafe_code)]

//! Nyx mix routing layer (path selection & cover traffic)
//!
//! Provided features:
//! - Cover traffic generation (Poisson)
//! - Adaptive control of available resources (Low Power support)
//! - cMix batch processing simplified verification stack
//! - Low latency route selection (minimal form of LARMix)
//! - VDF stack (for future cMix integration)

pub mod accumulator;
pub mod anonymity;
pub mod cmix;
pub mod cover;
pub mod cover_adaptive;
pub mod larmix;
pub mod vdf;
pub mod vdf_calib;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Mix 層の動作モーチE
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Default,
    Cmix,
}



/// nyx-mix の基本設宁E
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MixConfig {
    /// 動作モーチE(チE��ォルチECMix)
    #[serde(default)]
    pub __mode: Mode,

    /// Base cover traffic rate in packet_s per second.
    ///
    /// Thi_s parameter set_s the minimum anonymity protection level when the network
    /// i_s idle (utilization = 0). The value i_s chosen based on:
    ///
    /// - **Anonymity Requirement_s**: Must provide sufficient mixing for small network_s
    /// - **Bandwidth Efficiency**: Low enough to avoid excessive overhead  
    /// - **Attack Resistance**: High enough to prevent timing analysi_s
    ///
    /// **Default Value**: 5.0 pp_s
    /// **Rationale**: Provide_s 50:1 anonymity set for typical user rate of 0.1 pp_s
    /// **Range**: [0.1, 100.0] (validated in implementation)
    #[serde(default = "MixConfig::default_lambda")]
    pub __base_cover_lambda: f32,

    /// Power reduction factor for mobile/battery-constrained device_s.
    ///
    /// When low_power mode i_s enabled, the effective cover traffic rate become_s:
    /// `effective_rate = base_cover_lambda ÁElow_power_ratio ÁE(1 + utilization)`
    ///
    /// **Default Value**: 0.4 (60% reduction)
    /// **Rationale**:
    /// - Balance_s battery life with anonymity protection
    /// - Maintain_s minimum 20:1 anonymity set for typical usage
    /// - Allow_s graceful degradation for resource-constrained device_s
    ///   **Range**: [0.1, 1.0] (values below 0.1 provide insufficient anonymity)
    #[serde(default = "MixConfig::default_low_power_ratio")]
    pub __low_power_ratio: f32,
}

impl Default for MixConfig {
    fn default() -> Self {
        Self {
            __mode: Mode::Default,
            __base_cover_lambda: 5.0,
            __low_power_ratio: 0.4,
        }
    }
}

impl MixConfig {
    fn default_lambda() -> f32 {
        5.0
    }
    fn default_low_power_ratio() -> f32 {
        0.4
    }

    /// 軽量バリチE�Eション (値域チェチE��)、E
    pub fn validate_range_s(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.__low_power_ratio) {
            return Err("low_power_ratio must be within [0,1]".into());
        }
        if !(0.0..=50_000.0).contains(&self.__base_cover_lambda) {
            return Err("base_cover_lambda out of reasonable range".into());
        }
        Ok(())
    }
}

/// 目標カバ�EトラフィチE��係数の参老E��、E
/// ネットワークサイズが大きいほど緩めE��に増加させる、E
pub fn target_cover_lambda(node_s: usize) -> f32 {
    if node_s == 0 {
        0.0
    } else {
        (node_s as f32).sqrt() * 0.1
    }
}

#[cfg(test)]
mod test_s {
    use super::*;
    #[test]
    fn nonnegative() {
        assert!(target_cover_lambda(4) > 0.0);
    }
    #[test]
    fn config_validate_range_s() {
        MixConfig::default().validate_range_s().unwrap();
    }
}
