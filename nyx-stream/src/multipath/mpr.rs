#![forbid(unsafe_code)]

use super::scheduler::{PathId, WeightedScheduler, PathMetric};

#[derive(Debug, Clone, Default)]
pub struct MprConfig {
    pub __enabled: bool,
}

#[derive(Debug)]
pub struct MprState {
    pub sched: Option<WeightedScheduler>,
}

impl MprState {
    pub fn disabled() -> Self { Self { sched: None } }
    pub fn new(path_s: &[(PathId, PathMetric)]) -> Self { Self { sched: Some(WeightedScheduler::new(path_s)) } }
    pub fn pick_path(&mut self) -> PathId { self.sched.as_mut().map(|_s| _s.next_path()).unwrap_or(PathId(0)) }
    pub fn on_rtt_sample(&mut self, __path: PathId, sample: std::time::Duration) {
        if let Some(_s) = self.sched.as_mut() { _s.observe_rtt(path, sample); }
    }
    pub fn on_los_s(&mut self, path: PathId) {
        if let Some(_s) = self.sched.as_mut() { _s.observe_los_s(path); }
    }
}
