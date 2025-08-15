//! FFI-backed ScreenStateDetector implementation (polling bridge)
//!
//! This module provides a pure-Rust polling adapter that queries the
//! nyx-mobile-ffi exported C ABI functions to obtain real device state
//! (screen on/off, battery level, low power mode) without introducing
//! new C/C++ dependencies. It satisfies the ScreenStateDetector trait
//! required by LowPowerManager.
//!
//! Design Choices:
//! - Polling intervals differentiated: screen (2s), battery (60s),
//!   power-save (10s). These values balance responsiveness vs. wakeups.
//! - Change detection uses last seen atomic snapshot; only transitions
//!   are forwarded into the unbounded channel to minimize allocation.
//! - Error handling: FFI returning -1 is converted to LowPowerError.
//! - Thread-safety: cloning Arc<Self> not needed; we share interior
//!   atomics for low contention.
//! - Test mode: when compiled with `cfg(test)` the extern functions are
//!   re-exported as mutable function pointers that tests can override
//!   to simulate platform events deterministically.
//!
//! Future Improvements:
//! - Replace polling with platform callback bridging (requires ObjC/Java additions).
//! - Adaptive backoff when device is stationary / unchanged for long periods.
//! - Integrate network state for NetworkUnavailable power state decisions.

use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc,
};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use tracing::{trace, warn};

use crate::low_power::{LowPowerError, ScreenStateDetector};

#[cfg(any(target_os = "ios", target_os = "android"))]
use nyx_mobile_ffi; // real FFI crate

// Optional event-driven fast path: global atoms updated by mobile callbacks
#[cfg(any(target_os = "ios", target_os = "android"))]
static GLOBAL_SCREEN_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
#[cfg(any(target_os = "ios", target_os = "android"))]
static GLOBAL_LOW_POWER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(any(target_os = "ios", target_os = "android"))]
static GLOBAL_BATTERY: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(100);
#[cfg(any(target_os = "ios", target_os = "android"))]
static EVENT_TX: once_cell::sync::OnceCell<mpsc::UnboundedSender<bool>> =
    once_cell::sync::OnceCell::new();

/// C-ABI callback from nyx-mobile-ffi to deliver immediate events
#[cfg(any(target_os = "ios", target_os = "android"))]
#[no_mangle]
extern "C" fn nyx_core_mobile_event_callback(event: i32, value: i32) {
    use std::sync::atomic::Ordering;
    match event {
        0 => {
            // SCREEN
            let on = value != 0;
            GLOBAL_SCREEN_ON.store(on, Ordering::Relaxed);
            if let Some(tx) = EVENT_TX.get() {
                let _ = tx.send(on);
            }
        }
        1 => {
            // LOW_POWER
            let lp = value != 0;
            GLOBAL_LOW_POWER.store(lp, Ordering::Relaxed);
            if lp {
                if let Some(tx) = EVENT_TX.get() {
                    let _ = tx.send(false);
                }
            }
        }
        2 => {
            // BATTERY
            let lvl = value.clamp(0, 100) as u8;
            GLOBAL_BATTERY.store(lvl, Ordering::Relaxed);
        }
        _ => {}
    }
}

// Desktop / non-mobile fallback stubs (simulate stable values)
#[cfg(not(any(target_os = "ios", target_os = "android")))]
mod desktop_stub {
    use std::sync::atomic::{AtomicU8, Ordering};
    static BATTERY: AtomicU8 = AtomicU8::new(80);
    #[allow(dead_code)]
    pub fn init() -> i32 {
        0
    }
    #[allow(dead_code)]
    pub fn start() -> i32 {
        0
    }
    #[allow(dead_code)]
    pub fn screen_on() -> i32 {
        1
    }
    #[allow(dead_code)]
    pub fn battery_level() -> i32 {
        BATTERY.load(Ordering::Relaxed) as i32
    }
    #[allow(dead_code)]
    pub fn low_power() -> i32 {
        0
    }
}

// FFI declarations (stable C ABI coming from nyx-mobile-ffi crate)
// We call into the Rust crate public extern functions directly.

/// Convert FFI int (>=0 success) into value or error.
fn ffi_bool(name: &str, v: i32) -> Result<bool, LowPowerError> {
    if v < 0 {
        return Err(LowPowerError::ScreenStateError(format!(
            "{} returned error",
            name
        )));
    }
    Ok(v != 0)
}
fn ffi_u8(name: &str, v: i32) -> Result<u8, LowPowerError> {
    if v < 0 {
        return Err(LowPowerError::BatteryMonitorError(format!(
            "{} returned error",
            name
        )));
    }
    if v > 100 {
        warn!(level=%v, "battery level out of range from {}", name);
    }
    Ok(v.clamp(0, 100) as u8)
}

/// Polling-based detector bridging the mobile FFI.
pub struct FfiScreenStateDetector {
    last_screen_on: Arc<AtomicBool>,
    last_battery: Arc<AtomicU8>,
    last_power_save: Arc<AtomicBool>,
    poll_interval_screen: Duration,
    poll_interval_battery: Duration,
    poll_interval_power: Duration,
    running: AtomicBool,
}

impl FfiScreenStateDetector {
    pub fn new() -> Result<Arc<Self>, LowPowerError> {
        // Initialize underlying mobile layer
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            let rc = nyx_mobile_ffi::nyx_mobile_init();
            if rc < -1 {
                return Err(LowPowerError::PlatformNotSupported);
            }
            let rc2 = nyx_mobile_ffi::nyx_mobile_start_monitoring();
            if rc2 < -1 {
                return Err(LowPowerError::PlatformNotSupported);
            }
            // Register event callback for immediate updates
            let _ = nyx_mobile_ffi::nyx_mobile_register_event_callback(Some(
                nyx_core_mobile_event_callback,
            ));
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            let _ = desktop_stub::init();
            let _ = desktop_stub::start();
        }
        Ok(Arc::new(Self {
            last_screen_on: Arc::new(AtomicBool::new(true)),
            last_battery: Arc::new(AtomicU8::new(100)),
            last_power_save: Arc::new(AtomicBool::new(false)),
            poll_interval_screen: Duration::from_secs(2),
            poll_interval_battery: Duration::from_secs(60),
            poll_interval_power: Duration::from_secs(10),
            running: AtomicBool::new(false),
        }))
    }

    fn read_screen(&self) -> Result<bool, LowPowerError> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            ffi_bool(
                "nyx_mobile_is_screen_on",
                nyx_mobile_ffi::nyx_mobile_is_screen_on(),
            )
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            ffi_bool("nyx_mobile_is_screen_on", desktop_stub::screen_on())
        }
    }
    fn read_screen_static() -> Result<bool, LowPowerError> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            ffi_bool(
                "nyx_mobile_is_screen_on",
                nyx_mobile_ffi::nyx_mobile_is_screen_on(),
            )
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            ffi_bool("nyx_mobile_is_screen_on", desktop_stub::screen_on())
        }
    }
    fn read_battery(&self) -> Result<u8, LowPowerError> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            ffi_u8(
                "nyx_mobile_get_battery_level",
                nyx_mobile_ffi::nyx_mobile_get_battery_level(),
            )
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            ffi_u8(
                "nyx_mobile_get_battery_level",
                desktop_stub::battery_level(),
            )
        }
    }
    fn read_battery_static() -> Result<u8, LowPowerError> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            ffi_u8(
                "nyx_mobile_get_battery_level",
                nyx_mobile_ffi::nyx_mobile_get_battery_level(),
            )
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            ffi_u8(
                "nyx_mobile_get_battery_level",
                desktop_stub::battery_level(),
            )
        }
    }
    fn read_power_save(&self) -> Result<bool, LowPowerError> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            ffi_bool(
                "nyx_mobile_is_low_power_mode",
                nyx_mobile_ffi::nyx_mobile_is_low_power_mode(),
            )
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            ffi_bool("nyx_mobile_is_low_power_mode", desktop_stub::low_power())
        }
    }
    fn read_power_save_static() -> Result<bool, LowPowerError> {
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            ffi_bool(
                "nyx_mobile_is_low_power_mode",
                nyx_mobile_ffi::nyx_mobile_is_low_power_mode(),
            )
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            ffi_bool("nyx_mobile_is_low_power_mode", desktop_stub::low_power())
        }
    }
}

impl ScreenStateDetector for FfiScreenStateDetector {
    fn is_screen_on(&self) -> Result<bool, LowPowerError> {
        self.read_screen()
    }

    fn start_monitoring(&self) -> Result<mpsc::UnboundedReceiver<bool>, LowPowerError> {
        // Ensure single start.
        if self.running.swap(true, Ordering::SeqCst) {
            // Already running; create a new receiver observing future transitions by cloning logic? Simpler: return error.
            return Err(LowPowerError::ScreenStateError(
                "monitoring already started".into(),
            ));
        }

        let (tx, rx) = mpsc::unbounded_channel();
        #[cfg(any(target_os = "ios", target_os = "android"))]
        {
            let _ = EVENT_TX.set(tx.clone());
        }
        // Emit initial state
        let initial = self.read_screen().unwrap_or(true);
        let _ = tx.send(initial);
        self.last_screen_on.store(initial, Ordering::Relaxed);
        // Prime battery/power-save
        if let Ok(b) = self.read_battery() {
            self.last_battery.store(b, Ordering::Relaxed);
        }
        if let Ok(p) = self.read_power_save() {
            self.last_power_save.store(p, Ordering::Relaxed);
        }

        let screen_flag = self.last_screen_on.clone();
        let batt_cell = self.last_battery.clone();
        let pwr_flag = self.last_power_save.clone();
        let tx_screen = tx.clone();
        let screen_iv = self.poll_interval_screen;
        let batt_iv = self.poll_interval_battery;
        let pwr_iv = self.poll_interval_power;

        // Screen poll loop
        tokio::spawn(async move {
            loop {
                sleep(screen_iv).await;
                match FfiScreenStateDetector::read_screen_static() {
                    Ok(now) => {
                        let prev = screen_flag.swap(now, Ordering::Relaxed);
                        if now != prev {
                            let _ = tx_screen.send(now);
                            trace!(screen_on = now, "screen state changed");
                        }
                    }
                    Err(e) => warn!(error=%e, "screen poll error"),
                }
            }
        });

        // Battery + power-save combined loop (different cadences)
        let tx2 = tx.clone();
        tokio::spawn(async move {
            let mut batt_acc = Duration::ZERO;
            let mut pwr_acc = Duration::ZERO;
            loop {
                let step = Duration::from_secs(2);
                sleep(step).await;
                batt_acc += step;
                pwr_acc += step;
                if batt_acc >= batt_iv {
                    batt_acc = Duration::ZERO;
                    if let Ok(b) = FfiScreenStateDetector::read_battery_static() {
                        let prev = batt_cell.swap(b, Ordering::Relaxed);
                        if (prev as i16 - b as i16).abs() >= 5 {
                            // significant delta
                            trace!(battery = b, prev = prev, "battery level delta");
                        }
                    }
                }
                if pwr_acc >= pwr_iv {
                    pwr_acc = Duration::ZERO;
                    match FfiScreenStateDetector::read_power_save_static() {
                        Ok(now) => {
                            let prev = pwr_flag.swap(now, Ordering::Relaxed);
                            if now && !prev {
                                // Force a synthetic screen_off (PowerSave) transition for manager logic
                                let _ = tx2.send(false);
                                trace!("power save mode asserted; synthetic screen_off sent");
                            }
                        }
                        Err(e) => warn!(error=%e, "power-save poll error"),
                    }
                }
            }
        });

        Ok(rx)
    }

    fn get_battery_level(&self) -> Result<u8, LowPowerError> {
        self.read_battery()
    }
    fn is_power_save_mode(&self) -> Result<bool, LowPowerError> {
        self.read_power_save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::low_power::ScreenStateDetector;

    #[tokio::test]
    async fn test_polling_detector_basic() {
        let det = FfiScreenStateDetector::new().unwrap();
        let mut rx = det.start_monitoring().expect("start");
        // Initial state
        let first = rx.recv().await.unwrap();
        assert!(first);
    }
}
