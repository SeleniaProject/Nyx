use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct ZcTelemetry {
    pub _buffers_created: AtomicU64,
    pub _buffers_shared: AtomicU64,
}

impl ZcTelemetry {
    pub fn inc_created(&self) {
        self._buffers_created.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_shared(&self) {
        self._buffers_shared.fetch_add(1, Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> (u64, u64) {
        (
            self._buffers_created.load(Ordering::Relaxed),
            self._buffers_shared.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod test_s {
    use super::*;
    #[test]
    fn telemetry_counts() {
        let t = ZcTelemetry::default();
        t.inc_created();
        t.inc_shared();
        t.inc_shared();
        assert_eq!(t.snapshot(), (1, 2));
    }
}
