//! Low-power integration bridge for mobile platforms.
//!
//! When feature `low_power` is enabled, this module initializes the
//! `nyx-mobile-ffi` layer and periodically polls the unified power state,
//! emitting daemon events and metrics on changes.

use std::time::Duration;

use nyx_core::{
    low_power::{screen_off_ratio, InactivityTrigger, ScreenState},
    types::TimestampMs,
};
use tokio::{task::JoinHandle, time::interval};
use tracing::{debug, info, warn};

use crate::event_system::{Event, EventSystem};
use serde::Serialize;

// Re-export FFI functions from nyx-mobile-ffi. These are normal Rust functions with C ABI
// and are safe to call directly (no unsafe block needed).
use nyx_mobile_ffi::{
    nyx_mobile_init, nyx_mobile_set____log_level, nyx_mobile_shutdown, nyx_power_get_state,
    rust_get_power_state, rust_get_resume_count, rust_get_wake_count, NyxPowerState, NyxStatus,
};

/// Background task handle for the low-power bridge.
pub struct LowPowerBridge {
    #[allow(dead_code)]
    handle: JoinHandle<()>,
}

impl LowPowerBridge {
    /// Starts the bridge, initializes the FFI, and spawns a polling loop.
    /// Returns a handle to the background task.
    pub fn start(events: EventSystem) -> anyhow::Result<Self> {
        // Initialize FFI layer (idempotent)
        let rc = nyx_mobile_init();
        if rc != NyxStatus::Ok as i32 && rc != NyxStatus::AlreadyInitialized as i32 {
            warn!(code = rc, "nyx_mobile_init returned non-ok status");
        } else {
            info!("nyx_mobile_ffi initialized");
        }

        // Optional log level from env: error|warn|info|debug|trace or numeric 0..4
        if let Ok(lv) = std::env::var("NYX_MOBILE_LOG_LEVEL") {
            let code = match lv.to_ascii_lowercase().as_str() {
                "error" => 0,
                "warn" | "warning" => 1,
                "info" => 2,
                "debug" => 3,
                "trace" => 4,
                _ => lv.parse::<i32>().unwrap_or(2),
            };
            let _ = nyx_mobile_set____log_level(code);
        }

        // Poll interval and inactivity threshold via env with sensible defaults
        let poll_ms: u64 = std::env::var("NYX_POWER_POLL_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1000);
        let inactivity_ms: u64 = std::env::var("NYX_INACTIVITY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5 * 60 * 1000);

        // Rate limiter: at most one inactivity event per minute by default
        let rate_per_sec: f64 = std::env::var("NYX_INACTIVITY_RATE_PER_SEC")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0 / 60.0);

        let handle = tokio::spawn(async move {
            let mut intv = interval(Duration::from_millis(poll_ms));
            // Tick once to establish a baseline without immediate work
            intv.tick().await;

            // Maintain a compact history of (timestamp, screen state)
            let mut history: Vec<(TimestampMs, ScreenState)> = Vec::with_capacity(128);

            // Initialize inactivity trigger
            let start_ms = now_ms();
            let mut inactivity = InactivityTrigger::new(
                Duration::from_millis(inactivity_ms),
                rate_per_sec,
                TimestampMs(start_ms),
            );

            let mut prev_state: Option<u32> = None;
            let mut prev_wake: u32 = rust_get_wake_count();
            let mut prev_resume: u32 = rust_get_resume_count();

            // Simple debounce window to avoid chattering
            let debounce_ms: u64 = 300;
            let mut last_emit_ms: u64 = 0;

            loop {
                intv.tick().await;
                // Primary source via FFI getter
                let mut cur: u32 = 0;
                let rc = unsafe { nyx_power_get_state(&mut cur as *mut u32) };
                if rc != NyxStatus::Ok as i32 {
                    cur = rust_get_power_state();
                }

                let now = now_ms();
                if prev_state != Some(cur) {
                    // Debounce: suppress rapid toggles within debounce_ms
                    if now.saturating_sub(last_emit_ms) < debounce_ms {
                        continue;
                    }
                    let stamp = now;
                    let screen = map_power_to_screen(cur);
                    history.push((TimestampMs(stamp), screen));
                    // Trim history to last 10 minutes
                    let windowstart = stamp.saturating_sub(10 * 60 * 1000);
                    while history.len() > 2 {
                        if let Some(&(TimestampMs(t0), _)) = history.first() {
                            if t0 < windowstart {
                                history.remove(0);
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    // Emit rich event
                    let detail = serde_json::to_string(&PowerEvent::State {
                        state: display_power(cur).to_string(),
                    })
                    .unwrap_or_else(|_| "{\"type\":\"state\"}".into());
                    let _ = events.sender().send(Event {
                        _ty: "power".into(),
                        _detail: detail,
                    });
                    // Metrics
                    metrics::counter!("nyx.power.state.change", "state" => cur.to_string())
                        .increment(1);

                    // Activity heuristic: entering Active resets inactivity
                    if cur == NyxPowerState::Active as u32 {
                        inactivity.record_activity(TimestampMs(stamp));
                    }

                    prev_state = Some(cur);
                    last_emit_ms = stamp;
                }

                // Periodically compute off ratio over the last minute (if enough data)
                let one_min_start = now.saturating_sub(60 * 1000);
                let slice: Vec<(TimestampMs, ScreenState)> = history
                    .iter()
                    .copied()
                    .filter(|(TimestampMs(t), _)| *t >= one_min_start)
                    .collect();
                if slice.len() >= 2 {
                    let ratio = screen_off_ratio(&slice);
                    metrics::gauge!("nyx.power.screen_off_ratio_1m").set(ratio);
                    debug!("screen_off_ratio_1m = {:.3}", ratio);
                }

                // Inactivity trigger
                if inactivity.should_trigger(TimestampMs(now)) {
                    let detail = serde_json::to_string(&PowerEvent::Inactivity)
                        .unwrap_or_else(|_| "{\"type\":\"inactivity\"}".into());
                    let _ = events.sender().send(Event {
                        _ty: "power".into(),
                        _detail: detail,
                    });
                    metrics::counter!("nyx.power.inactivity.trigger").increment(1);
                }

                // Detect wake/resume counters and emit events
                let wk = rust_get_wake_count();
                if wk > prev_wake {
                    let detail = serde_json::to_string(&PowerEvent::Wake)
                        .unwrap_or_else(|_| "{\"type\":\"wake\"}".into());
                    let _ = events.sender().send(Event {
                        _ty: "power".into(),
                        _detail: detail,
                    });
                    prev_wake = wk;
                }
                let rs = rust_get_resume_count();
                if rs > prev_resume {
                    let detail = serde_json::to_string(&PowerEvent::Resume)
                        .unwrap_or_else(|_| "{\"type\":\"resume\"}".into());
                    let _ = events.sender().send(Event {
                        _ty: "power".into(),
                        _detail: detail,
                    });
                    prev_resume = rs;
                }
            }
        });

        Ok(Self { handle })
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum PowerEvent {
    State { state: String },
    Wake,
    Resume,
    Inactivity,
}

impl Drop for LowPowerBridge {
    fn drop(&mut self) {
        // Best-effort shutdown (idempotent)
        let _ = nyx_mobile_shutdown();
    }
}

fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn map_power_to_screen(state: u32) -> ScreenState {
    match state {
        x if x == NyxPowerState::Active as u32 => ScreenState::On,
        x if x == NyxPowerState::Background as u32 => ScreenState::Off,
        x if x == NyxPowerState::Inactive as u32 => ScreenState::Off,
        x if x == NyxPowerState::Critical as u32 => ScreenState::Off,
        _ => ScreenState::Off,
    }
}

fn display_power(state: u32) -> &'static str {
    match state {
        x if x == NyxPowerState::Active as u32 => "active",
        x if x == NyxPowerState::Background as u32 => "background",
        x if x == NyxPowerState::Inactive as u32 => "inactive",
        x if x == NyxPowerState::Critical as u32 => "critical",
        _ => "unknown",
    }
}

#[cfg(all(test, feature = "low_power"))]
mod test_s {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;
    use tokio::time::timeout;

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        // In tests we can't use ? since we don't return Result; unwrap is fine here.
        L.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[tokio::test]
    async fn emits_initial_state_event() {
        let __g = test_lock();
        std::env::set_var("NYX_POWER_POLL_MS", "50");
        let event_s = EventSystem::new(16);
        let mut rx = event_s.subscribe();
        let __bridge = LowPowerBridge::start(event_s).unwrap();
        // Expect first event due to initial state observation
        let ev = timeout(Duration::from_millis(1000), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ev._ty, "power");
        // detail i_s JSON like {"type":"State","state":"active"}
        assert!(ev._detail.contains("\"type\":") && ev._detail.contains("state"));
    }

    #[tokio::test]
    async fn emits_on_state_change() {
        let __g = test_lock();
        std::env::set_var("NYX_POWER_POLL_MS", "50");
        let event_s = EventSystem::new(16);
        let mut rx = event_s.subscribe();
        let __bridge = LowPowerBridge::start(event_s.clone()).unwrap();
        // Drain the first initial event if present
        let __ = timeout(Duration::from_millis(500), rx.recv()).await;

        // Change state to Background
        let __ = nyx_mobile_ffi::nyx_power_set_state(NyxPowerState::Background as u32);

        // Expect a state:background event
        let ev = timeout(Duration::from_millis(1500), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ev._ty, "power");
        assert!(
            ev._detail.contains("\"state\":\"background\""),
            "got {}",
            ev._detail
        );
    }
}
