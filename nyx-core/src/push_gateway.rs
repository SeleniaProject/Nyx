//! Push Notification Path / Gateway Reconnection Manager
//!
//! Provides a lightweight pathway to (re)establish a minimal Nyx path set
//! after a mobile push wake event. Designed for Low Power scenarios where
//! the primary multipath set has been torn down or quiesced to save power.
//!
//! Responsibilities:
//! - Track last wake timestamps and debounce spurious multiple wakes.
//! - Initiate fast path builder request (1 control + 1 data) via callback.
//! - Expose FFI functions: nyx_push_wake(), nyx_resume_low_power_session().
//! - Provide exponential backoff capped retry for reconnection failures.
//!
//! This module intentionally avoids direct dependency on heavy routing
//! components; instead it relies on an injected trait object implementing
//! a minimal reconnection contract so that integration stays decoupled.
//!
//! Safety: All extern "C" functions are thin wrappers that delegate into
//! thread-safe interior (Arc + Mutex). No unsafe code required.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{info, warn, error};
use once_cell::sync::OnceCell;
#[cfg(feature = "telemetry")] use nyx_telemetry::metrics::BasicMetrics; // assuming re-export path

/// Error type for push gateway operations.
#[derive(thiserror::Error, Debug)]
pub enum PushGatewayError {
    #[error("Reconnection already in progress")] AlreadyInProgress,
    #[error("Too soon since last wake (debounced)")] Debounced,
    #[error("Executor unavailable")] ExecutorUnavailable,
    #[error("Maximum retries exhausted")] RetriesExhausted,
}

/// Minimal trait abstracting a reconnection path builder.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub trait MinimalReconnector: Send + Sync + 'static {
    /// Attempt to (re)establish a minimal path set. Should be idempotent.
    fn reconnect_minimal(&self) -> BoxFuture<'_, Result<(), String>>;
}

/// Internal mutable state.
#[derive(Debug)]
struct InnerState {
    last_wake: Option<Instant>,
    reconnect_in_flight: bool,
    total_wake_events: u64,
    total_reconnect_attempts: u64,
    total_reconnect_failures: u64,
    total_reconnect_success: u64,
    cumulative_latency_ms: u128,
    #[cfg(feature = "telemetry")] wake_metric: BasicMetrics,
}

impl Default for InnerState { fn default() -> Self { Self { last_wake: None, reconnect_in_flight: false, total_wake_events:0, total_reconnect_attempts:0, total_reconnect_failures:0, total_reconnect_success:0, cumulative_latency_ms:0, #[cfg(feature="telemetry")] wake_metric: BasicMetrics::new() } } }

/// Manager object.
pub struct PushGatewayManager {
    state: Mutex<InnerState>,
    reconnector: Arc<dyn MinimalReconnector>,
    debounce: Duration,
    max_retries: u8,
    base_backoff: Duration,
}

impl PushGatewayManager {
    pub fn new(reconnector: Arc<dyn MinimalReconnector>) -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(InnerState::default()), reconnector, debounce: Duration::from_secs(2), max_retries: 5, base_backoff: Duration::from_millis(200) })
    }

    /// Construct from a simple async closure returning Result<(), String>
    pub fn from_async_fn<F, Fut>(f: F) -> Arc<Self>
    where
        F: Send + Sync + 'static + Fn() -> Fut,
        Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
    {
        struct FnReconnector<F>(F);
        impl<F, Fut> MinimalReconnector for FnReconnector<F>
        where
            F: Send + Sync + 'static + Fn() -> Fut,
            Fut: std::future::Future<Output = Result<(), String>> + Send + 'static,
        {
            fn reconnect_minimal(&self) -> BoxFuture<'_, Result<(), String>> {
                let fut = (self.0)();
                Box::pin(async move { fut.await })
            }
        }
        let reconnector: Arc<dyn MinimalReconnector> = Arc::new(FnReconnector(f));
        Self::new(reconnector)
    }

    /// Record a push wake event (may trigger reconnection later via resume call).
    pub fn push_wake(&self) -> Result<(), PushGatewayError> {
        let mut s = self.state.lock().unwrap();
        let now = Instant::now();
        if let Some(prev) = s.last_wake { if now.duration_since(prev) < self.debounce { return Err(PushGatewayError::Debounced); } }
    s.last_wake = Some(now); s.total_wake_events += 1; #[cfg(feature="telemetry")] { s.wake_metric.increment(); } info!("push wake recorded"); Ok(())
    }

    /// Explicit resume command: attempt immediate minimal reconnection with retry.
    pub async fn resume_low_power_session(self: &Arc<Self>) -> Result<(), PushGatewayError> {
    let start_all = Instant::now();
        {
            let mut s = self.state.lock().unwrap();
            if s.reconnect_in_flight { return Err(PushGatewayError::AlreadyInProgress); }
            s.reconnect_in_flight = true;
        }
        let mut attempt: u8 = 0;
        loop {
            attempt += 1;
            // Record attempt (short critical section)
            { let mut s = self.state.lock().unwrap(); s.total_reconnect_attempts += 1; }

            let res = self.reconnector.reconnect_minimal().await;
            match res {
                Ok(_) => { let mut s = self.state.lock().unwrap(); s.reconnect_in_flight = false; s.total_reconnect_success += 1; s.cumulative_latency_ms += start_all.elapsed().as_millis() as u128; info!(attempt, latency_ms = start_all.elapsed().as_millis(), "minimal path reconnection succeeded"); return Ok(()); }
                Err(e) => {
                    let mut s = self.state.lock().unwrap(); s.total_reconnect_failures += 1; warn!(attempt, error=%e, "reconnection attempt failed");
                    if attempt >= self.max_retries { s.reconnect_in_flight = false; error!("reconnection retries exhausted"); return Err(PushGatewayError::RetriesExhausted); }
                }
            }
            let backoff = self.base_backoff * 2u32.pow((attempt-1) as u32);
            tokio::time::sleep(backoff).await;
        }
    }

    pub fn stats(&self) -> PushGatewayStats { let s = self.state.lock().unwrap(); let avg = if s.total_reconnect_success>0 { Some((s.cumulative_latency_ms / s.total_reconnect_success as u128) as u64) } else { None }; PushGatewayStats { total_wake_events: s.total_wake_events, total_reconnect_attempts: s.total_reconnect_attempts, total_reconnect_failures: s.total_reconnect_failures, total_reconnect_success: s.total_reconnect_success, avg_reconnect_latency_ms: avg } }
}

/// Exposed statistics snapshot.
#[derive(Debug, Clone)]
pub struct PushGatewayStats { pub total_wake_events: u64, pub total_reconnect_attempts: u64, pub total_reconnect_failures: u64, pub total_reconnect_success: u64, pub avg_reconnect_latency_ms: Option<u64> }

// Global singleton (simple for FFI calls)
static GLOBAL_MANAGER: OnceCell<Arc<PushGatewayManager>> = OnceCell::new();

/// Initialize global manager (called by daemon setup)
pub fn install_global_manager(mgr: Arc<PushGatewayManager>) -> bool { GLOBAL_MANAGER.set(mgr).is_ok() }

fn with_manager<F, R>(f: F) -> Option<R> where F: FnOnce(&Arc<PushGatewayManager>) -> R { GLOBAL_MANAGER.get().map(f) }

/// FFI: record a push wake event (debounced). Returns 0 success, >0 debounced, <0 error.
#[no_mangle]
pub extern "C" fn nyx_push_wake() -> i32 {
    with_manager(|m| match m.push_wake() { Ok(_) => 0, Err(PushGatewayError::Debounced) => 1, Err(_) => -1 }).unwrap_or(-2)
}

/// FFI: attempt resume (async dispatch). Returns immediately (0 queued / -1 error / -2 uninit).
#[no_mangle]
pub extern "C" fn nyx_resume_low_power_session() -> i32 {
    if let Some(m) = GLOBAL_MANAGER.get() {
        let m_clone = m.clone();
        // Spawn onto a default runtime (expect caller has a Tokio runtime installed)
        tokio::spawn(async move { let _ = m_clone.resume_low_power_session().await; });
        0
    } else { -2 }
}

// --- Tests ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU8, Ordering};

    struct MockReconnector { attempts: AtomicU8, fail_until: u8 }
    impl MinimalReconnector for MockReconnector {
        fn reconnect_minimal(&self) -> BoxFuture<'_, Result<(), String>> {
            let attempts_ref = &self.attempts; let fail_until = self.fail_until;
            Box::pin(async move {
                let a = attempts_ref.fetch_add(1, Ordering::SeqCst) + 1; if a <= fail_until { Err("fail".into()) } else { Ok(()) }
            })
        }
    }

    #[tokio::test]
    async fn test_retry_succeeds() {
        let reconn = Arc::new(MockReconnector { attempts: AtomicU8::new(0), fail_until: 2 });
        let mgr = PushGatewayManager::new(reconn);
        mgr.push_wake().unwrap();
        mgr.resume_low_power_session().await.unwrap();
        let st = mgr.stats();
        assert_eq!(st.total_reconnect_failures, 2);
        assert_eq!(st.total_reconnect_attempts, 3);
    }

    #[tokio::test]
    async fn test_debounce() {
        let reconn = Arc::new(MockReconnector { attempts: AtomicU8::new(0), fail_until: 0 });
        let mgr = PushGatewayManager::new(reconn);
        assert_eq!(mgr.push_wake().is_ok(), true);
        // Immediate second wake should debounce
        assert!(matches!(mgr.push_wake(), Err(PushGatewayError::Debounced)));    }
}
