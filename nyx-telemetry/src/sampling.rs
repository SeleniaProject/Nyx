use std::sync::atomic::{AtomicU64, Ordering};
use once_cell::sync::Lazy;
use std::sync::RwLock as StdRwLock;

// -------------------------------------------------------------------------------------------------
// Global sampling kept/dropped Prometheus counter hooks (registered by TelemetryCollector).
// We avoid direct prometheus dependency here (already a crate dependency) to keep this module pure;
// the counters are injected at runtime and are optional (no-ops if not yet registered).
// -------------------------------------------------------------------------------------------------
use prometheus::IntCounter;

struct SamplingCounters { kept: IntCounter, dropped: IntCounter }
static SAMPLING_COUNTERS: Lazy<StdRwLock<Option<SamplingCounters>>> = Lazy::new(|| StdRwLock::new(None));

pub(crate) fn register_sampling_counters(kept: IntCounter, dropped: IntCounter) {
    *SAMPLING_COUNTERS.write().unwrap() = Some(SamplingCounters { kept, dropped });
}

#[inline]
fn inc_kept() { if let Some(c) = SAMPLING_COUNTERS.read().unwrap().as_ref() { c.kept.inc(); } }
#[inline]
fn inc_dropped() { if let Some(c) = SAMPLING_COUNTERS.read().unwrap().as_ref() { c.dropped.inc(); } }

/// Deterministic ratio sampling without randomness.
/// 0.0 => drop all, 1.0 => keep all, else keep every Nth span (N ≈ 1/ratio).
#[inline]
pub fn deterministic_accept(counter: &AtomicU64, ratio: f64) -> bool {
    if ratio <= 0.0 { inc_dropped(); return false; }
    if ratio >= 1.0 { inc_kept(); return true; }
    let step = (1.0 / ratio).round().max(1.0) as u64; // ratio=0.1 -> 10
    let c = counter.fetch_add(1, Ordering::Relaxed);
    let accepted = c % step == 0;
    if accepted { inc_kept(); } else { inc_dropped(); }
    accepted
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ratio_behavior() {
        let c = AtomicU64::new(0);
        let mut kept = 0;
        for _ in 0..100 { if deterministic_accept(&c, 0.2) { kept+=1; } }
        assert!(kept > 0 && kept < 100, "kept={}", kept);
    }
}
