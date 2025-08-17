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

use schemars::JsonSchema;
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
	pub mode: Mode,
	/// カバートラフィック基準レート (pps)
	#[serde(default = "MixConfig::default_lambda")] 
	pub base_cover_lambda: f32,
	/// 低電力時の比率 (0..=1)
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
	fn default_lambda() -> f32 { 5.0 }
	fn default_low_power_ratio() -> f32 { 0.4 }

	/// 軽量バリデーション (値域チェック)。
	pub fn validate_ranges(&self) -> Result<(), String> {
		if !(0.0..=1.0).contains(&self.low_power_ratio) {
			return Err("low_power_ratio must be within [0,1]".into());
		}
		if !(0.0..=50_000.0).contains(&self.base_cover_lambda) {
			return Err("base_cover_lambda out of reasonable range".into());
		}
		Ok(())
	}
}

/// 目標カバートラフィック係数の参考値。
/// ネットワークサイズが大きいほど緩やかに増加させる。
pub fn target_cover_lambda(nodes: usize) -> f32 {
	if nodes == 0 { 0.0 } else { (nodes as f32).sqrt() * 0.1 }
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn non_negative() { assert!(target_cover_lambda(4) > 0.0); }
	#[test]
	fn config_validate_ranges() { MixConfig::default().validate_ranges().unwrap(); }
}

