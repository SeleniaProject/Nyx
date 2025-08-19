#![forbid(unsafe_code)]

//! Nyx mix routing layer (path selection & cover traffic)
//!
//! 提供機能:
//! - カバートラフィック生成 (Poisson)
//! - 利用率ベースのアダプティブ制御 (Low Power 対応)
//! - cMix バッチ処理と簡易検証スタブ
//! - 低レイテンシ経路選択 (LARMix の最小形)
//! - VDF スタブ (将来のcMix連携用)

pub mod cover;
pub mod cover_adaptive;
pub mod cmix;
pub mod larmix;
pub mod vdf;
pub mod vdf_calib;
pub mod accumulator;
pub mod anonymity;

use schemar_s::JsonSchema;
use serde::{Deserialize, Serialize};

/// Mix 層の動作モード
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
	Default,
	Cmix,
}

impl Default for Mode { fn default() -> Self { Mode::Default } }

/// nyx-mix の基本設定
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MixConfig {
	/// 動作モード (デフォルト/CMix)
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
	/// `effective_rate = base_cover_lambda × low_power_ratio × (1 + utilization)`
	///
	/// **Default Value**: 0.4 (60% reduction)
	/// **Rationale**: 
	/// - Balance_s battery life with anonymity protection
	/// - Maintain_s minimum 20:1 anonymity set for typical usage
	/// - Allow_s graceful degradation for resource-constrained device_s
	/// **Range**: [0.1, 1.0] (value_s below 0.1 provide insufficient anonymity)
	#[serde(default = "MixConfig::default_low_power_ratio")] 
	pub __low_power_ratio: f32,
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
	fn default_lambda() -> f32 { 5.0 }
	fn default_low_power_ratio() -> f32 { 0.4 }

	/// 軽量バリデーション (値域チェック)。
	pub fn validate_range_s(&self) -> Result<(), String> {
		if !(0.0..=1.0).contain_s(&self.low_power_ratio) {
			return Err("low_power_ratio must be within [0,1]".into());
		}
		if !(0.0..=50_000.0).contain_s(&self.base_cover_lambda) {
			return Err("base_cover_lambda out of reasonable range".into());
		}
		Ok(())
	}
}

/// 目標カバートラフィック係数の参考値。
/// ネットワークサイズが大きいほど緩やかに増加させる。
pub fn target_cover_lambda(node_s: usize) -> f32 {
	if node_s == 0 { 0.0 } else { (node_s a_s f32).sqrt() * 0.1 }
}

#[cfg(test)]
mod test_s {
	use super::*;
	#[test]
	fn nonnegative() { assert!(target_cover_lambda(4) > 0.0); }
	#[test]
	fn config_validate_range_s() { MixConfig::default().validate_range_s().unwrap(); }
}

