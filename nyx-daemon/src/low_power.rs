//! Low power mode integration (desktop stub).
//! Provides a unified interface also used by mobile FFI layer. Desktop implementation
//! simulates screen on/off and battery level via environment variables for testing.

use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration, Instant};
use tokio::sync::watch;
use tracing::{debug, info, warn};

// Mobile FFI (nyx-mobile-ffi) – behind optional build; if absent these functions fall back.
#[allow(unused_imports)]
use nyx_mobile_ffi::{nyx_mobile_get_battery_level, nyx_mobile_is_screen_on, nyx_mobile_is_low_power_mode, nyx_mobile_get_app_state};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppLifecycle { Active, Background, Inactive }

impl From<i32> for AppLifecycle {
    fn from(v: i32) -> Self { match v { 0 => Self::Active, 1 => Self::Background, 2 => Self::Inactive, _ => Self::Active } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState { Normal, Low }

#[derive(Debug, Clone)]
pub struct LowPowerConfig {
    pub inactivity_seconds: u64,
    pub battery_threshold_percent: u8,
}

impl Default for LowPowerConfig { fn default() -> Self { Self { inactivity_seconds: 300, battery_threshold_percent: 15 } } }

pub struct LowPowerManager {
    config: LowPowerConfig,
    last_activity: Instant,
    manual_override: Arc<AtomicBool>,
    tx: watch::Sender<PowerState>,
    rx: watch::Receiver<PowerState>,
    last_emitted: Arc<parking_lot::Mutex<PowerState>>, // suppress duplicate events
    app_state: Arc<parking_lot::Mutex<AppLifecycle>>,
    event_system: Option<crate::event_system::EventSystem>,
    dynamic_low_power: Arc<AtomicBool>, // current evaluated state
}

impl LowPowerManager {
    pub fn new(config: LowPowerConfig) -> Arc<Self> {
        let (tx, rx) = watch::channel(PowerState::Normal);
        Arc::new(Self {
            config,
            last_activity: Instant::now(),
            manual_override: Arc::new(AtomicBool::new(false)),
            tx,
            rx,
            last_emitted: Arc::new(parking_lot::Mutex::new(PowerState::Normal)),
            app_state: Arc::new(parking_lot::Mutex::new(AppLifecycle::Active)),
            event_system: None,
            dynamic_low_power: Arc::new(AtomicBool::new(false)),
        })
    }
    pub fn with_event_system(mut self: Arc<Self>, es: crate::event_system::EventSystem) -> Arc<Self> {
        Arc::get_mut(&mut Arc::clone(&self)).map(|inner| inner.event_system = Some(es));
        self
    }
    pub fn touch(&self) { self.last_activity = Instant::now(); }
    pub fn set_manual_low_power(&self, enable: bool) { self.manual_override.store(enable, Ordering::Relaxed); let _ = self.evaluate(); }
    pub fn subscribe(&self) -> watch::Receiver<PowerState> { self.rx.clone() }
    pub fn current_state(&self) -> PowerState { *self.rx.borrow() }
    fn read_mobile_power(&self) -> (Option<u8>, Option<bool>, Option<bool>, Option<AppLifecycle>) {
        // Attempt to pull from mobile FFI; desktop build may just return defaults.
        let battery = (nyx_mobile_get_battery_level as fn() -> i32)();
        let screen = (nyx_mobile_is_screen_on as fn() -> i32)();
        let low_mode = (nyx_mobile_is_low_power_mode as fn() -> i32)();
        let app = (nyx_mobile_get_app_state as fn() -> i32)();
        let map_flag = |v: i32| if v >= 0 { Some(v != 0) } else { None };
        let map_opt_u8 = |v: i32| if v >= 0 { Some((v as u8).min(100)) } else { None };
        (map_opt_u8(battery), map_flag(screen), map_flag(low_mode), Some(app.into()))
    }
    fn evaluate(&self) -> PowerState {
        // Inactivity heuristic (only if app active or screen on; if background reduce weight)
        let (battery_level, screen_on, os_low_power, app_state_opt) = self.read_mobile_power();
        if let Some(a) = app_state_opt { *self.app_state.lock() = a; }
        let app_state = *self.app_state.lock();
        let inactivity_threshold = match app_state { AppLifecycle::Active => self.config.inactivity_seconds, AppLifecycle::Background => self.config.inactivity_seconds / 2, AppLifecycle::Inactive => self.config.inactivity_seconds / 3 };
        let inactivity = self.last_activity.elapsed() > Duration::from_secs(inactivity_threshold);
        let env_battery_low = std::env::var("NYX_BATTERY_PERCENT").ok().and_then(|v| v.parse::<u8>().ok()).map(|b| b <= self.config.battery_threshold_percent).unwrap_or(false);
        let battery_low = battery_level.map(|b| b <= self.config.battery_threshold_percent).unwrap_or(env_battery_low);
        let screen_off = screen_on.map(|s| !s).unwrap_or(false);
        let os_low = os_low_power.unwrap_or(false);
        let manual = self.manual_override.load(Ordering::Relaxed);
        let trigger = manual || battery_low || os_low || (inactivity && (screen_off || app_state != AppLifecycle::Active));
        let new_state = if trigger { PowerState::Low } else { PowerState::Normal };
        self.dynamic_low_power.store(new_state == PowerState::Low, Ordering::Relaxed);
        let _ = self.tx.send_replace(new_state);
        // Emit event if changed
        let mut last = self.last_emitted.lock();
        if *last != new_state { *last = new_state; if let Some(es) = &self.event_system { if crate::event_system::EventSystem::is_enabled() { let mut ev = crate::event_system::EventSystem::build_simple_event("power.state.changed", "info", format!("state={:?} battery_low={} inactivity={} screen_off={} os_low={} manual={} app_state={:?}", new_state, battery_low, inactivity, screen_off, os_low, manual, app_state)); es.publish_event(ev); } } }
        new_state
    }
    pub fn is_low_power(&self) -> bool { self.dynamic_low_power.load(Ordering::Relaxed) }
    pub fn recommended_cover_ratio(&self) -> f32 { if self.is_low_power() { 0.1 } else { 1.0 } }
    pub async fn run(self: Arc<Self>) {
        info!("Starting LowPowerManager loop");
        loop {
            let app_state = *self.app_state.lock();
            let interval = match app_state { AppLifecycle::Active => 30, AppLifecycle::Background => 15, AppLifecycle::Inactive => 10 };
            tokio::time::sleep(Duration::from_secs(interval)).await;
            let st = self.evaluate();
            debug!(?st, "evaluated low power state");
        }
    }
}
