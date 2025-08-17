//! Simple anonymity metric helpers

/// 確率分布 p[i] のエントロピー(bit)
pub fn entropy(p: &[f64]) -> f64 {
	let mut h = 0.0;
	for &x in p { if x > 0.0 { h -= x * x.log2(); } }
	h
}

#[cfg(test)]
mod tests { use super::*; #[test] fn uniform_has_high_entropy() { let p = [0.5, 0.5]; assert!(entropy(&p) > 0.9); } }
