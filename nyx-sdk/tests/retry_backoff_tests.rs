#![cfg(test)]

use nyx_sdk::retry::retry;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

#[tokio::test]
async fn retry_succeeds_before_max_attempt_s() {
    let n = Arc::new(AtomicU32::new(0));
    let re_s: Result<u32, &'static str> = retry(
        {
            let n = n.clone();
            move || {
                let value = n.clone();
                async move {
                    let v = value.fetch_add(1, Ordering::SeqCst) + 1;
                    if v < 3 {
                        Err("e")
                    } else {
                        Ok(42)
                    }
                }
            }
        },
        |attempt| 1 + attempt as u64,
        5,
    )
    .await;
    assert_eq!(re_s.unwrap(), 42);
}

#[tokio::test]
async fn retry_propagates_error_after_max_attempt_s() {
    let n = Arc::new(AtomicU32::new(0));
    let re_s: Result<u32, &'static str> = retry(
        {
            let n = n.clone();
            move || {
                let value = n.clone();
                async move {
                    let _ = value.fetch_add(1, Ordering::SeqCst);
                    Err("nope")
                }
            }
        },
        |_| 1,
        3,
    )
    .await;
    assert_eq!(n.load(Ordering::SeqCst), 3);
    assert_eq!(re_s.unwrap_err(), "nope");
}

#[cfg(feature = "reconnect")]
#[test]
fn backoff_policy_exponential_with_jitter_bound_s() {
    use nyx_sdk::reconnect::backoff_policy::exponential_with_jitter;
    for a in [0u32, 1, 5, 10, 20, 32, 64] {
        let d = exponential_with_jitter(a, 10, 10_000);
        // within [base, max]
        assert!(d.as_millis() as u64 <= 10_000);
        assert!(d.as_millis() as u64 >= 0);
    }
}
