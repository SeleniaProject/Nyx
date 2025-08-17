//! Adaptive cover traffic controller

use crate::MixConfig;

/// 観測利用率[0,1]から、適用するカバーレートを返す。
/// 非減少性: 利用率が上がるとレートは下がらない（ここでは単調増加）。
pub fn apply_utilization(config: &MixConfig, utilization: f32, low_power: bool) -> f32 {
	let u = utilization.clamp(0.0, 1.0);
	let base = if low_power {
		config.base_cover_lambda * config.low_power_ratio
	} else { config.base_cover_lambda };
	// 線形増加: 最低 base, 最大 2x base
	base * (1.0 + u)
}

#[cfg(test)]
mod tests { use super::*; #[test] fn monotonic() { let c = MixConfig::default(); assert!(apply_utilization(&c, 0.8, false) >= apply_utilization(&c, 0.2, false)); } }
