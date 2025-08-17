//! VDF calibration helper

use std::time::{Duration, Instant};
use crate::vdf;

/// 目標実行時間(approx)に合わせて必要反復回数を推定
pub fn estimate_iters(target: Duration) -> u32 {
	let seed = b"nyx-calib";
	let mut iters: u32 = 1000;
	// 粗い指数探索 → 近傍微調整
	loop {
		let t0 = Instant::now();
		let _ = vdf::eval(seed, iters);
		let el = t0.elapsed();
		if el >= target || iters >= u32::MAX / 2 { break; }
		let factor = (target.as_secs_f64() / el.as_secs_f64()).clamp(1.2, 8.0);
		iters = (iters as f64 * factor) as u32;
	}
	// 近傍調整
	let mut best = (iters, Duration::MAX);
	for d in [0.5, 0.75, 1.0, 1.25, 1.5] {
		let cand = ((iters as f64) * d) as u32;
		let t0 = Instant::now();
		let _ = vdf::eval(seed, cand);
		let el = t0.elapsed();
		let err = if el > target { el - target } else { target - el };
		if err < best.1 { best = (cand, err); }
	}
	best.0.max(1)
}

#[cfg(test)]
mod tests { use super::*; #[test] fn returns_positive() { assert!(estimate_iters(Duration::from_millis(1)) > 0); } }
