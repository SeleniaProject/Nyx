#![forbid(unsafe_code)]

/// カバートラフィックの目標係数を返すスタブ。
pub fn target_cover_lambda(nodes: usize) -> f32 {
	if nodes == 0 { 0.0 } else { (nodes as f32).sqrt() * 0.1 }
}

#[cfg(test)]
mod tests { use super::*; #[test] fn non_negative() { assert!(target_cover_lambda(4) > 0.0); } }

