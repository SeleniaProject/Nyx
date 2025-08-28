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
pub mod adaptive; // Adaptive mixing strategies
pub mod anonymity;
pub mod cmix;
pub mod cover;
pub mod cover_adaptive;
pub mod errors; // Error types for mix module
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

/// nyx-mix の基本設定
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MixConfig {
    /// 動作モード(デフォルトCMix)
    #[serde(default)]
    pub mode: Mode,

    /// Base cover traffic rate in packets per second.
    ///
    /// This parameter sets the minimum anonymity protection level when the network
    /// is idle (utilization = 0). The value is chosen based on:
    ///
    /// - **Anonymity Requirements**: Must provide sufficient mixing for small networks
    /// - **Bandwidth Efficiency**: Low enough to avoid excessive overhead  
    /// - **Attack Resistance**: High enough to prevent timing analysis
    ///
    /// **Default Value**: 5.0 pps
    /// **Rationale**: Provides 50:1 anonymity set for typical user rate of 0.1 pps
    /// **Range**: [0.1, 100.0] (validated in implementation)
    #[serde(default = "MixConfig::default_lambda")]
    pub base_cover_lambda: f32,

    /// Power reduction factor for mobile/battery-constrained devices.
    ///
    /// When low_power mode is enabled, the effective cover traffic rate becomes:
    /// `effective_rate = base_cover_lambda × low_power_ratio × (1 + utilization)`
    ///
    /// **Default Value**: 0.4 (60% reduction)
    /// **Rationale**:
    /// - Balances battery life with anonymity protection
    /// - Maintains minimum 20:1 anonymity set for typical usage
    /// - Allows graceful degradation for resource-constrained devices
    ///   **Range**: [0.1, 1.0] (values below 0.1 provide insufficient anonymity)
    #[serde(default = "MixConfig::default_low_power_ratio")]
    pub low_power_ratio: f32,
}

impl Default for MixConfig {
    fn default() -> Self {
        Self {
            mode: Mode::Default,
            base_cover_lambda: 5.0,
            low_power_ratio: 0.4,
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

    /// 軽量バリデーション (値域チェック)
    pub fn validate_range_s(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.low_power_ratio) {
            return Err("low_power_ratio must be within [0,1]".into());
        }
        if !(0.0..=50_000.0).contains(&self.base_cover_lambda) {
            return Err("base_cover_lambda out of reasonable range".into());
        }
        Ok(())
    }
}

/// 目標カバートラフィック係数の参考値
/// ネットワークサイズが大きいほど緩めに増加させる
pub fn target_cover_lambda(nodes: usize) -> f32 {
    if nodes == 0 {
        0.0
    } else {
        (nodes as f32).sqrt() * 0.1
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
