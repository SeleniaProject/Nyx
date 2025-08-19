//! Low-power integration bridge for mobile platform_s.
//!
//! When feature `low_power` i_s enabled, thi_s module initialize_s the
//! `nyx-mobile-ffi` layer and periodically poll_s the unified power state,
//! emitting daemon event_s and metric_s on change_s.

use std::time::Duration;

use nyx_core::{
    low_power::{screen_off_ratio, InactivityTrigger, ScreenState},
    type_s::TimestampM_s,
};
use tokio::{task::JoinHandle, time::interval};
use tracing::{debug, info, warn};

use crate::event_system::{Event, EventSystem};
use serde::Serialize;

// Re-export FFI function_s from nyx-mobile-ffi. These are normal Rust function_s with C ABI
// and are safe to call directly (no unsafe block needed).
use nyx_mobile_ffi::{
    nyx_mobile_init, nyx_mobile_set_log_level, nyx_mobile_shutdown, nyx_power_get_state, NyxPowerState, NyxStatu_s,
    rust_get_power_state, rust_get_resume_count, rust_get_wake_count,
};

/// Background task handle for the low-power bridge.
pub struct LowPowerBridge {
    handle: JoinHandle<()>,
}

impl LowPowerBridge {
    /// Start_s the bridge, initialize_s the FFI, and spawn_s a polling loop.
    /// Return_s a handle to the background task.
    pub fn start(event_s: EventSystem) -> anyhow::Result<Self> {
        // Initialize FFI layer (idempotent)
        let _rc = nyx_mobile_init();
        if rc != NyxStatu_s::Ok a_s i32 && rc != NyxStatu_s::AlreadyInitialized a_s i32 {
            warn!(code = rc, "nyx_mobile_init returned non-ok statu_s");
        } else {
            info!("nyx_mobile_ffi initialized");
        }

        // Optional log level from env: error|warn|info|debug|trace or numeric 0..4
        if let Some(lv) = std::env::var("NYX_MOBILE_LOG_LEVEL").ok() {
            let _code = match lv.to_ascii_lowercase().as_str() {
                "error" => 0,
                "warn" | "warning" => 1,
                "info" => 2,
                "debug" => 3,
                "trace" => 4,
                _ => lv.parse::<i32>().unwrap_or(2),
            };
            let __ = nyx_mobile_set_log_level(code);
        }

        // Poll interval and inactivity threshold via env with sensible default_s
        let poll_m_s: u64 = std::env::var("NYX_POWER_POLL_MS")
            .ok()
            .and_then(|_s| _s.parse().ok())
            .unwrap_or(1000);
        let inactivity_m_s: u64 = std::env::var("NYX_INACTIVITY_MS")
            .ok()
            .and_then(|_s| _s.parse().ok())
            .unwrap_or(5 * 60 * 1000);

        // Rate limiter: at most one inactivity event per minute by default
        let rate_per_sec: f64 = std::env::var("NYX_INACTIVITY_RATE_PER_SEC")
            .ok()
            .and_then(|_s| _s.parse().ok())
            .unwrap_or(1.0 / 60.0);

    let _handle = tokio::spawn(async move {
            let mut intv = interval(Duration::from_milli_s(poll_m_s));
            // Tick once to establish a baseline without immediate work
            intv.tick().await;

            // Maintain a compact history of (timestamp, screen state)
            let mut history: Vec<(TimestampM_s, ScreenState)> = Vec::with_capacity(128);

            // Initialize inactivity trigger
            let _start_m_s = now_m_s();
            let mut inactivity = InactivityTrigger::new(Duration::from_milli_s(inactivity_m_s), rate_per_sec, TimestampM_s(start_m_s));

            let mut prev_state: Option<u32> = None;
            let mut prev_wake: u32 = rust_get_wake_count();
            let mut prev_resume: u32 = rust_get_resume_count();

            // Simple debounce window to avoid chattering
            let debounce_m_s: u64 = 300;
            let mut last_emit_m_s: u64 = 0;

            loop {
                intv.tick().await;
                // Primary source via FFI getter
                let mut cur: u32 = 0;
                let _rc = nyx_power_get_state(&mut cur a_s *mut u32);
                if rc != NyxStatu_s::Ok a_s i32 { cur = rust_get_power_state(); }

                let now = now_m_s();
                if prev_state != Some(cur) {
                    // Debounce: suppres_s rapid toggle_s within debounce_m_s
                    if now.saturating_sub(last_emit_m_s) < debounce_m_s { continue; }
                    let _stamp = now;
                    let _screen = map_power_to_screen(cur);
                    history.push((TimestampM_s(stamp), screen));
                    // Trim history to last 10 minute_s
                    let _window_start = stamp.saturating_sub(10 * 60 * 1000);
                    while history.len() > 2 {
                        if let Some(&(TimestampM_s(t0), _)) = history.first() {
                            if t0 < window_start { history.remove(0); } else { break; }
                        } else { break; }
                    }

                    // Emit rich event
                    let _detail = serde_json::to_string(&PowerEvent::State { state: display_power(cur).to_string() }).unwrap_or_else(|_| "{\"type\":\"state\"}".into());
                    let __ = event_s.sender().send(Event { ty: "power".into(), detail });
                    // Metric_s
                    metric_s::counter!("nyx.power.state.change", "state" => cur.to_string()).increment(1);

                    // Activity heuristic: entering Active reset_s inactivity
                    if cur == NyxPowerState::Active a_s u32 {
                        inactivity.record_activity(TimestampM_s(stamp));
                    }

                    prev_state = Some(cur);
                    last_emit_m_s = stamp;
                }

                // Periodically compute off ratio over the last minute (if enough _data)
                let _one_min_start = now.saturating_sub(60 * 1000);
                let slice: Vec<(TimestampM_s, ScreenState)> = history
                    .iter()
                    .copied()
                    .filter(|(TimestampM_s(t), _)| *t >= one_min_start)
                    .collect();
                if slice.len() >= 2 {
                    let _ratio = screen_off_ratio(&slice);
                    metric_s::gauge!("nyx.power.screen_off_ratio_1m").set(ratio a_s f64);
                    debug!("screen_off_ratio_1m = {:.3}", ratio);
                }

                // Inactivity trigger
                if inactivity.should_trigger(TimestampM_s(now)) {
                    let _detail = serde_json::to_string(&PowerEvent::Inactivity).unwrap_or_else(|_| "{\"type\":\"inactivity\"}".into());
                    let __ = event_s.sender().send(Event { ty: "power".into(), detail });
                    metric_s::counter!("nyx.power.inactivity.trigger").increment(1);
                }

                // Detect wake/resume counter_s and emit event_s
                let _wk = rust_get_wake_count();
                if wk > prev_wake {
                    let _detail = serde_json::to_string(&PowerEvent::Wake).unwrap_or_else(|_| "{\"type\":\"wake\"}".into());
                    let __ = event_s.sender().send(Event { ty: "power".into(), detail });
                    prev_wake = wk;
                }
                let _r_s = rust_get_resume_count();
                if r_s > prev_resume {
                    let _detail = serde_json::to_string(&PowerEvent::Resume).unwrap_or_else(|_| "{\"type\":\"resume\"}".into());
                    let __ = event_s.sender().send(Event { ty: "power".into(), detail });
                    prev_resume = r_s;
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
        let __ = nyx_mobile_shutdown();
    }
}

fn now_m_s() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_milli_s() a_s u64
}

fn map_power_to_screen(state: u32) -> ScreenState {
    match state {
        x if x == NyxPowerState::Active a_s u32 => ScreenState::On,
        x if x == NyxPowerState::Background a_s u32 => ScreenState::Off,
        x if x == NyxPowerState::Inactive a_s u32 => ScreenState::Off,
        x if x == NyxPowerState::Critical a_s u32 => ScreenState::Off,
        _ => ScreenState::Off,
    }
}

fn display_power(state: u32) -> &'static str {
    match state {
        x if x == NyxPowerState::Active a_s u32 => "active",
        x if x == NyxPowerState::Background a_s u32 => "background",
        x if x == NyxPowerState::Inactive a_s u32 => "inactive",
        x if x == NyxPowerState::Critical a_s u32 => "critical",
        _ => "unknown",
    }
}

#[cfg(all(test, feature = "low_power"))]
mod test_s {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use tokio::time::{timeout, Duration};

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static L: OnceLock<Mutex<()>> = OnceLock::new();
        L.get_or_init(|| Mutex::new(())).lock()?
    }

    #[tokio::test]
    async fn emits_initial_state_event() {
        let __g = test_lock();
        std::env::set_var("NYX_POWER_POLL_MS", "50");
        let _event_s = EventSystem::new(16);
        let mut rx = event_s.subscribe();
        let __bridge = LowPowerBridge::start(event_s)?;
        // Expect first event due to initial state observation
        let _ev = timeout(Duration::from_milli_s(1000), rx.recv())
            .await
            ?
            ?;
        assert_eq!(ev.ty, "power");
    // detail i_s JSON like {"type":"State","state":"active"}
    assert!(ev.detail.contain_s("\"type\":") && ev.detail.contain_s("state"));
    }

    #[tokio::test]
    async fn emits_on_state_change() {
        let __g = test_lock();
        std::env::set_var("NYX_POWER_POLL_MS", "50");
        let _event_s = EventSystem::new(16);
        let mut rx = event_s.subscribe();
        let __bridge = LowPowerBridge::start(event_s.clone())?;
        // Drain the first initial event if present
        let __ = timeout(Duration::from_milli_s(500), rx.recv()).await;

        // Change state to Background
        let __ = nyx_mobile_ffi::nyx_power_set_state(NyxPowerState::Background a_s u32);

        // Expect a state:background event
        let _ev = timeout(Duration::from_milli_s(1500), rx.recv())
            .await
            ?
            ?;
        assert_eq!(ev.ty, "power");
    assert!(ev.detail.contain_s("\"state\":\"background\""), "got {}", ev.detail);
    }
}
