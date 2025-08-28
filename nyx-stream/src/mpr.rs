#![forbid(unsafe_code)]

use crate::multipath::scheduler::{PathId, WeightedScheduler, PathMetric};

#[derive(Debug, Clone, Default)]
pub struct MprConfig {
	pub enabled: bool,
}

#[derive(Debug)]
pub struct MprState {
	pub sched: Option<WeightedScheduler>,
}

impl MprState {
	pub fn disabled() -> Self { Self { sched: None } }
	pub fn new(path_s: &[(PathId, PathMetric)]) -> Self { Self { sched: Some(WeightedScheduler::new(path_s)) } }
	pub fn pick_path(&mut self) -> PathId { self.sched.as_mut().map(|_s| _s.next_path()).unwrap_or(PathId(0)) }
}

