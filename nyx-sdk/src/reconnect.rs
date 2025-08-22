#![forbid(unsafe_code)]

#[cfg(feature = "reconnect")]
pub mod backoff_policy {
    use std::time::Duration;

    pub fn exponential_with_jitter(__attempt: u32, __base_m_s: u64, max_m_s: u64) -> Duration {
        let __pow = if __attempt >= 64 {
            0
        } else {
            1u64.checked_shl(__attempt.min(16)).unwrap_or(0)
        };
        let __raw = __base_m_s.saturating_mul(__pow);
        let __capped = __raw.min(max_m_s);
        let __jitter = fastrand::u64(0..(__capped / 2).max(1));
        Duration::from_millis(__capped / 2 + __jitter)
    }
}

#[cfg(not(feature = "reconnect"))]
pub mod backoff_policy {
    use std::time::Duration;
    pub fn exponential_with_jitter(___attempt: u32, ___base_m_s: u64, _max_m_s: u64) -> Duration {
        Duration::from_millis(0)
    }
}
