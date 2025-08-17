//! Minimal cMix batcher stub

use std::time::{Duration, Instant};

#[derive(Default)]
pub struct BatchStats { pub emitted: usize, pub last_flush: Option<Instant>, pub errors: usize }

pub struct Batcher { size: usize, timeout: Duration, buf: Vec<Vec<u8>>, pub stats: BatchStats }

impl Batcher {
	pub fn new(size: usize, timeout: Duration) -> Self { Self { size, timeout, buf: Vec::with_capacity(size), stats: Default::default() } }
	pub fn push(&mut self, pkt: Vec<u8>) -> Option<Vec<Vec<u8>>> {
		self.buf.push(pkt);
		if self.buf.len() >= self.size { return Some(self.flush()); }
		None
	}
	pub fn tick(&mut self, now: Instant) -> Option<Vec<Vec<u8>>> {
		match self.stats.last_flush {
			None => { self.stats.last_flush = Some(now); None }
			Some(last) if now.duration_since(last) >= self.timeout && !self.buf.is_empty() => Some(self.flush()),
			_ => None,
		}
	}
	fn flush(&mut self) -> Vec<Vec<u8>> {
		let out = std::mem::take(&mut self.buf);
		self.stats.emitted += 1;
		self.stats.last_flush = Some(Instant::now());
		out
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn emits_batch_after_timeout() {
		let mut b = Batcher::new(10, Duration::from_millis(50));
		let t0 = Instant::now();
		assert!(b.tick(t0).is_none());
		b.push(vec![1]);
		let t1 = t0 + Duration::from_millis(60);
		let out = b.tick(t1);
		assert!(out.is_some());
		assert_eq!(out.unwrap().len(), 1);
	}
}
