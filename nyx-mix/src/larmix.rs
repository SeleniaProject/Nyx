//! Latency-aware route selection (minimal)

use rand::seq::SliceRandom;
use rand::Rng;

#[derive(Clone, Debug)]
pub struct Candidate { pub id: String, pub rtt_ms: u32 }

/// 小さいRTTほど選ばれやすいルーレット選択
pub fn choose_path(mut cands: Vec<Candidate>, rng: &mut impl Rng, hops: usize) -> Vec<Candidate> {
	if cands.is_empty() || hops == 0 { return Vec::new(); }
	// Weight = 1 / (rtt + 1)
	let weights: Vec<f64> = cands.iter().map(|c| 1.0 / (c.rtt_ms as f64 + 1.0)).collect();
	let mut out = Vec::with_capacity(hops);
	for _ in 0..hops {
		if let Some(idx) = weighted(&weights, rng) { out.push(cands[idx].clone()); }
	}
	if out.is_empty() { cands.shuffle(rng); out.extend(cands.into_iter().take(hops)); }
	out
}

fn weighted(weights: &[f64], rng: &mut impl Rng) -> Option<usize> {
	let sum: f64 = weights.iter().copied().sum();
	if sum <= f64::EPSILON { return None; }
	let mut t = rng.gen::<f64>() * sum;
	for (i, w) in weights.iter().enumerate() { t -= *w; if t <= 0.0 { return Some(i); } }
	Some(weights.len() - 1)
}

#[cfg(test)]
mod tests { use super::*; use rand::thread_rng; #[test] fn returns_no_more_than_hops() { let mut rng = thread_rng(); let c = vec![Candidate{ id:"a".into(), rtt_ms:10}, Candidate{ id:"b".into(), rtt_ms:50}]; let out = choose_path(c, &mut rng, 3); assert!(out.len() <= 3); } }
