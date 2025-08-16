#![forbid(unsafe_code)]

use crate::multipath::scheduler::{PathId, WeightedScheduler, PathMetric};

#[derive(Debug, Clone)]
pub struct MprConfig {
	pub enabled: bool,
}

impl Default for MprConfig {
	fn default() -> Self { Self { enabled: false } }
}

#[derive(Debug)]
pub struct MprState {
	pub sched: Option<WeightedScheduler>,
}

impl MprState {
	pub fn disabled() -> Self { Self { sched: None } }
	pub fn new(paths: &[(PathId, PathMetric)]) -> Self { Self { sched: Some(WeightedScheduler::new(paths)) } }
	pub fn pick_path(&mut self) -> PathId { self.sched.as_mut().map(|s| s.next_path()).unwrap_or(PathId(0)) }
}

