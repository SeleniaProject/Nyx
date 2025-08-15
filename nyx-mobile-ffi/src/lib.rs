//! Mobile Platform FFI Bindings
//!
//! This crate provides Foreign Function Interface (FFI) bindings for mobile platforms,
//! enabling integration with iOS (Objective-C/Swift) and Android (JNI) native APIs.
//!
//! ## iOS Integration
//! - UIDevice battery monitoring
//! - UIApplication state notifications
//! - Network framework connectivity detection
//! - ProcessInfo power management
//!
//! ## Android Integration
//! - BatteryManager APIs
//! - PowerManager state monitoring
//! - ConnectivityManager network detection
//! - ActivityManager lifecycle tracking
//!
//! ## Safety
//! All FFI functions are implemented with safe Rust wrappers around platform-specific
//! APIs, with comprehensive error handling and type safety guarantees.

#[cfg(feature = "telemetry")]
use nyx_telemetry::{MetricType, TelemetryCollector, TelemetryConfig};
use once_cell::sync::OnceCell;
#[cfg(feature = "telemetry")]
use std::collections::HashMap;
use std::os::raw::c_char;
use std::os::raw::c_int;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

// Internal mobile state definitions (avoiding nyx-core dependency)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Active = 0,
    Background = 1,
    Inactive = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkState {
    WiFi = 0,
    Cellular = 1,
    Ethernet = 2,
    None = 3,
}

#[derive(Debug, Clone, Copy)]
pub struct PowerState {
    pub battery_level: u8,
    pub is_charging: bool,
    pub screen_on: bool,
    pub low_power_mode: bool,
}

// Simple state storage without complex monitor
static CURRENT_POWER_STATE: OnceCell<std::sync::RwLock<PowerState>> = OnceCell::new();
static CURRENT_APP_STATE: OnceCell<std::sync::RwLock<AppState>> = OnceCell::new();
static CURRENT_NETWORK_STATE: OnceCell<std::sync::RwLock<NetworkState>> = OnceCell::new();

mod android;
mod common;
mod ios;

pub use android::*;
pub use common::*;
pub use ios::*;

/// Global tokio runtime for async operations
static RUNTIME: OnceCell<Arc<Runtime>> = OnceCell::new();
#[cfg(feature = "telemetry")]
static TELEMETRY: OnceCell<Arc<TelemetryCollector>> = OnceCell::new();
#[cfg(feature = "telemetry")]
static LABELS: OnceCell<std::sync::RwLock<HashMap<String, String>>> = OnceCell::new();
#[cfg(not(feature = "telemetry"))]
type HashMap<K, V> = std::collections::HashMap<K, V>; // keep type references compilable when telemetry disabled

// ---- Cross-crate event callback bridge ----
// Allows core (nyx-core) to register a single C-ABI callback to receive
// immediate mobile events in addition to polling.
type EventCallback = extern "C" fn(event: i32, value: i32);
static EVENT_CB: OnceCell<std::sync::Mutex<Option<EventCallback>>> = OnceCell::new();

/// Register a global event callback. Passing NULL clears the callback.
#[no_mangle]
pub extern "C" fn nyx_mobile_register_event_callback(cb: Option<EventCallback>) -> c_int {
    let cell = EVENT_CB.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = cell.lock().unwrap();
    *guard = cb;
    0
}

/// Notify an event from platform bridges (Objective-C/Java) into Rust.
/// event codes: 0=SCREEN(1 on/0 off), 1=LOW_POWER(1/0), 2=BATTERY(level 0..100), 3=APP_STATE(0..2), 4=NETWORK(0..3)
#[no_mangle]
pub extern "C" fn nyx_mobile_notify_event(event: i32, value: i32) {
    // Update local state snapshots where applicable
    match event {
        0 => {
            if let Some(st) = CURRENT_POWER_STATE.get() {
                let mut w = st.write().unwrap();
                w.screen_on = value != 0;
            }
        }
        1 => {
            if let Some(st) = CURRENT_POWER_STATE.get() {
                let mut w = st.write().unwrap();
                w.low_power_mode = value != 0;
            }
        }
        2 => {
            if let Some(st) = CURRENT_POWER_STATE.get() {
                let mut w = st.write().unwrap();
                w.battery_level = value.max(0).min(100) as u8;
            }
        }
        3 => {
            if let Some(st) = CURRENT_APP_STATE.get() {
                let mut w = st.write().unwrap();
                *w = match value {
                    0 => AppState::Active,
                    1 => AppState::Background,
                    _ => AppState::Inactive,
                };
            }
        }
        4 => {
            if let Some(st) = CURRENT_NETWORK_STATE.get() {
                let mut w = st.write().unwrap();
                *w = match value {
                    0 => NetworkState::WiFi,
                    1 => NetworkState::Cellular,
                    2 => NetworkState::Ethernet,
                    _ => NetworkState::None,
                };
            }
        }
        _ => {}
    }
    // Dispatch to registered callback if any
    if let Some(m) = EVENT_CB.get() {
        if let Some(cb) = *m.lock().unwrap() {
            cb(event, value);
        }
    }
}

/// Initialize the mobile FFI runtime
#[no_mangle]
pub extern "C" fn nyx_mobile_init() -> c_int {
    // Initialize tracing (ignore error if already initialized)
    let _ = tracing_subscriber::fmt::try_init();

    info!("Initializing Nyx Mobile FFI");

    // Create tokio runtime
    let runtime = match Runtime::new() {
        Ok(rt) => Arc::new(rt),
        Err(e) => {
            error!("Failed to create tokio runtime: {}", e);
            return -1;
        }
    };

    if RUNTIME.set(runtime).is_err() {
        warn!("Mobile FFI already initialized");
        return 1; // Already initialized
    }

    // Initialize state storage
    let initial_power = PowerState {
        battery_level: 100,
        is_charging: false,
        screen_on: true,
        low_power_mode: false,
    };

    let _ = CURRENT_POWER_STATE.set(std::sync::RwLock::new(initial_power));
    let _ = CURRENT_APP_STATE.set(std::sync::RwLock::new(AppState::Active));
    let _ = CURRENT_NETWORK_STATE.set(std::sync::RwLock::new(NetworkState::WiFi));

    #[cfg(feature = "telemetry")]
    {
        let _ = LABELS.set(std::sync::RwLock::new(HashMap::new()));
    }

    info!("Mobile FFI initialization complete");
    0 // Success
}

/// Cleanup mobile FFI resources
#[no_mangle]
pub extern "C" fn nyx_mobile_cleanup() {
    info!("Cleaning up Mobile FFI resources");
    #[cfg(feature = "telemetry")]
    {
        // Best-effort stop flag; TelemetryCollector currently uses internal atomic
        // and warp server, so we don't forcibly shutdown here.
        if let Some(_tel) = TELEMETRY.get() {
            debug!("Telemetry instance present during cleanup");
        }
    }
    debug!("Mobile FFI cleanup complete");
}

/// Get current battery level (0-100)
#[no_mangle]
pub extern "C" fn nyx_mobile_get_battery_level() -> c_int {
    #[cfg(target_os = "ios")]
    {
        return ios_get_battery_level() as c_int;
    }

    #[cfg(target_os = "android")]
    {
        return android_get_battery_level();
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop fallback
        match CURRENT_POWER_STATE.get() {
            Some(state) => state.read().unwrap().battery_level as c_int,
            None => 80, // Default
        }
    }
}

/// Check if device is charging (1 = true, 0 = false, -1 = error)
#[no_mangle]
pub extern "C" fn nyx_mobile_is_charging() -> c_int {
    #[cfg(target_os = "ios")]
    {
        return ios_is_charging();
    }

    #[cfg(target_os = "android")]
    {
        return android_is_charging();
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop fallback
        match CURRENT_POWER_STATE.get() {
            Some(state) => {
                if state.read().unwrap().is_charging {
                    1
                } else {
                    0
                }
            }
            None => 0, // Default
        }
    }
}

/// Check if screen is on (1 = true, 0 = false, -1 = error)
#[no_mangle]
pub extern "C" fn nyx_mobile_is_screen_on() -> c_int {
    #[cfg(target_os = "ios")]
    {
        return ios_is_screen_on();
    }

    #[cfg(target_os = "android")]
    {
        return android_is_screen_on();
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop fallback
        match CURRENT_POWER_STATE.get() {
            Some(state) => {
                if state.read().unwrap().screen_on {
                    1
                } else {
                    0
                }
            }
            None => 1, // Default
        }
    }
}

/// Check if low power mode is enabled (1 = true, 0 = false, -1 = error)
#[no_mangle]
pub extern "C" fn nyx_mobile_is_low_power_mode() -> c_int {
    #[cfg(target_os = "ios")]
    {
        return ios_is_low_power_mode();
    }

    #[cfg(target_os = "android")]
    {
        return android_is_power_save_mode();
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop fallback
        match CURRENT_POWER_STATE.get() {
            Some(state) => {
                if state.read().unwrap().low_power_mode {
                    1
                } else {
                    0
                }
            }
            None => 0, // Default
        }
    }
}

/// Get current app state (0 = Active, 1 = Background, 2 = Inactive, -1 = error)
#[no_mangle]
pub extern "C" fn nyx_mobile_get_app_state() -> c_int {
    #[cfg(target_os = "ios")]
    {
        return ios_get_app_state();
    }

    #[cfg(target_os = "android")]
    {
        return android_get_app_state();
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop fallback
        match CURRENT_APP_STATE.get() {
            Some(state) => *state.read().unwrap() as c_int,
            None => AppState::Active as c_int, // Default
        }
    }
}

/// Get current network state (0 = WiFi, 1 = Cellular, 2 = Ethernet, 3 = None, -1 = error)
#[no_mangle]
pub extern "C" fn nyx_mobile_get_network_state() -> c_int {
    #[cfg(target_os = "ios")]
    {
        return ios_get_network_state();
    }

    #[cfg(target_os = "android")]
    {
        return android_get_network_state();
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        // Desktop fallback
        match CURRENT_NETWORK_STATE.get() {
            Some(state) => *state.read().unwrap() as c_int,
            None => NetworkState::WiFi as c_int, // Default
        }
    }
}

/// Start mobile state monitoring
#[no_mangle]
pub extern "C" fn nyx_mobile_start_monitoring() -> c_int {
    info!("Starting mobile state monitoring");

    #[cfg(target_os = "ios")]
    {
        let result = ios_register_battery_notifications();
        if result != 0 {
            return result;
        }
        let result = ios_register_app_notifications();
        if result != 0 {
            return result;
        }
        info!("iOS monitoring started successfully");
    }

    #[cfg(target_os = "android")]
    {
        info!("Android monitoring ready (requires JNI initialization from Java)");
    }

    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        info!("Desktop platform - mobile monitoring simulated");
    }

    // Spawn periodic pollers to keep CURRENT_* states in sync (cross-platform)
    if let Some(rt) = RUNTIME.get() {
        let power_handle = {
            rt.spawn(async move {
                loop {
                    let level = nyx_mobile_get_battery_level();
                    let charging = nyx_mobile_is_charging();
                    let screen_on = nyx_mobile_is_screen_on();
                    let low_power = nyx_mobile_is_low_power_mode();
                    if let Some(state) = CURRENT_POWER_STATE.get() {
                        let mut w = state.write().unwrap();
                        // Clamp and map
                        w.battery_level = level.max(0).min(100) as u8;
                        w.is_charging = charging == 1;
                        w.screen_on = screen_on == 1;
                        w.low_power_mode = low_power == 1;
                    }
                    #[cfg(feature = "telemetry")]
                    {
                        if let Some(tel) = TELEMETRY.get() {
                            let mut labels = HashMap::new();
                            labels.insert(
                                "platform".to_string(),
                                if cfg!(target_os = "ios") {
                                    "ios".to_string()
                                } else if cfg!(target_os = "android") {
                                    "android".to_string()
                                } else {
                                    "other".to_string()
                                },
                            );
                            labels.insert(
                                "charging".to_string(),
                                if nyx_mobile_is_charging() == 1 {
                                    "true".to_string()
                                } else {
                                    "false".to_string()
                                },
                            );
                            // Merge extra labels if provided
                            if let Some(map) = LABELS.get() {
                                for (k, v) in map.read().unwrap().iter() {
                                    labels.insert(k.clone(), v.clone());
                                }
                            }
                            let _ = tel
                                .record_metric(
                                    "nyx_mobile_battery_level_percent",
                                    MetricType::Gauge,
                                    level.max(0).min(100) as f64,
                                    std::time::Instant::now(),
                                    Some(labels),
                                )
                                .await;
                        }
                        // Export via metrics crate for daemon-side Prometheus exporter
                        metrics::gauge!("nyx_mobile_battery_level_percent")
                            .set(level.max(0).min(100) as f64);
                    }
                    sleep(Duration::from_millis(5000)).await;
                }
            })
        };

        let app_handle = {
            rt.spawn(async move {
                loop {
                    let app = nyx_mobile_get_app_state();
                    if let Some(st) = CURRENT_APP_STATE.get() {
                        let mut w = st.write().unwrap();
                        *w = match app {
                            0 => AppState::Active,
                            1 => AppState::Background,
                            _ => AppState::Inactive,
                        };
                    }
                    #[cfg(feature = "telemetry")]
                    {
                        if let Some(tel) = TELEMETRY.get() {
                            let mut labels = HashMap::new();
                            labels.insert(
                                "platform".to_string(),
                                if cfg!(target_os = "ios") {
                                    "ios".to_string()
                                } else if cfg!(target_os = "android") {
                                    "android".to_string()
                                } else {
                                    "other".to_string()
                                },
                            );
                            labels.insert("app_state".to_string(), app.to_string());
                            if let Some(map) = LABELS.get() {
                                for (k, v) in map.read().unwrap().iter() {
                                    labels.insert(k.clone(), v.clone());
                                }
                            }
                            let _ = tel
                                .record_metric(
                                    "nyx_mobile_app_state",
                                    MetricType::Gauge,
                                    app as f64,
                                    std::time::Instant::now(),
                                    Some(labels),
                                )
                                .await;
                        }
                        metrics::gauge!("nyx_mobile_app_state").set(app as f64);
                    }
                    sleep(Duration::from_millis(5000)).await;
                }
            })
        };

        let net_handle = {
            rt.spawn(async move {
                loop {
                    let net = nyx_mobile_get_network_state();
                    if let Some(st) = CURRENT_NETWORK_STATE.get() {
                        let mut w = st.write().unwrap();
                        *w = match net {
                            0 => NetworkState::WiFi,
                            1 => NetworkState::Cellular,
                            2 => NetworkState::Ethernet,
                            _ => NetworkState::None,
                        };
                    }
                    #[cfg(feature = "telemetry")]
                    {
                        if let Some(tel) = TELEMETRY.get() {
                            let mut labels = HashMap::new();
                            labels.insert(
                                "platform".to_string(),
                                if cfg!(target_os = "ios") {
                                    "ios".to_string()
                                } else if cfg!(target_os = "android") {
                                    "android".to_string()
                                } else {
                                    "other".to_string()
                                },
                            );
                            labels.insert("network".to_string(), net.to_string());
                            if let Some(map) = LABELS.get() {
                                for (k, v) in map.read().unwrap().iter() {
                                    labels.insert(k.clone(), v.clone());
                                }
                            }
                            let _ = tel
                                .record_metric(
                                    "nyx_mobile_network_state",
                                    MetricType::Gauge,
                                    net as f64,
                                    std::time::Instant::now(),
                                    Some(labels),
                                )
                                .await;
                        }
                        metrics::gauge!("nyx_mobile_network_state").set(net as f64);
                    }
                    sleep(Duration::from_millis(5000)).await;
                }
            })
        };

        debug!(
            "Spawned mobile monitoring tasks: power={:?} app={:?} net={:?}",
            power_handle.id(),
            app_handle.id(),
            net_handle.id()
        );
    }

    0
}

/// Initialize in-process telemetry collector from Mobile FFI side.
/// This is optional and used when daemon-side exporter is not available.
#[no_mangle]
#[cfg(feature = "telemetry")]
pub extern "C" fn nyx_mobile_telemetry_init() -> c_int {
    if TELEMETRY.get().is_some() {
        return 1;
    }
    // Read configuration from environment with safe defaults
    let metrics_port = std::env::var("NYX_MOBILE_METRICS_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(0);
    let collection_interval = std::env::var("NYX_MOBILE_METRICS_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(15);
    let otlp_enabled = std::env::var("NYX_MOBILE_OTLP_ENABLED")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let otlp_endpoint = std::env::var("NYX_MOBILE_OTLP_ENDPOINT").ok();
    let trace_sampling = std::env::var("NYX_MOBILE_TRACE_SAMPLING")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    let cfg = TelemetryConfig {
        metrics_enabled: true,
        metrics_port,
        collection_interval,
        otlp_enabled,
        otlp_endpoint,
        trace_sampling,
        attribute_filter_config: None,
        exporter_recovery: false,
    };

    match TelemetryCollector::new(cfg) {
        Ok(t) => {
            let arc = Arc::new(t);
            // Start telemetry in background using the global runtime
            if let Some(rt) = RUNTIME.get() {
                let tel = arc.clone();
                rt.spawn(async move {
                    // Use full start() to run collection loop and metrics server; ignore errors to avoid crashing host.
                    let _ = tel.start().await;
                });
            }
            let _ = TELEMETRY.set(arc);
            0
        }
        Err(_) => -1,
    }
}

/// No-op variants to keep link compatibility when telemetry feature is disabled
#[cfg(not(feature = "telemetry"))]
#[no_mangle]
pub extern "C" fn nyx_mobile_telemetry_init() -> c_int {
    1
}

#[cfg(not(feature = "telemetry"))]
#[no_mangle]
pub extern "C" fn nyx_mobile_telemetry_shutdown() {}

/// Shutdown telemetry collector started via nyx_mobile_telemetry_init (no-op if not started)
#[no_mangle]
#[cfg(feature = "telemetry")]
pub extern "C" fn nyx_mobile_telemetry_shutdown() {
    if let Some(tel) = TELEMETRY.get() {
        if let Some(rt) = RUNTIME.get() {
            let tel = tel.clone();
            rt.spawn(async move {
                tel.stop().await;
            });
        }
    }
}

/// Set extra telemetry labels (key/value). Passing NULL clears the map.
#[no_mangle]
#[cfg(feature = "telemetry")]
pub extern "C" fn nyx_mobile_set_telemetry_label(key: *const c_char, value: *const c_char) {
    use crate::common::c_str_to_string;
    if let Some(map) = LABELS.get() {
        let mut w = map.write().unwrap();
        if key.is_null() || value.is_null() {
            w.clear();
            return;
        }
        if let (Ok(k), Ok(v)) = (c_str_to_string(key), c_str_to_string(value)) {
            if !k.is_empty() {
                w.insert(k, v);
            }
        }
    }
}

/// No-op when telemetry feature is disabled to keep platform bridges link-safe.
#[cfg(not(feature = "telemetry"))]
#[no_mangle]
pub extern "C" fn nyx_mobile_set_telemetry_label(_key: *const c_char, _value: *const c_char) {
    // no-op
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn ensure_init() {
        INIT.call_once(|| {
            let result = nyx_mobile_init();
            assert!(result >= 0, "FFI initialization should succeed");
        });
    }

    #[test]
    fn test_ffi_initialization() {
        ensure_init();
        // Already initialized by ensure_init()
    }

    #[test]
    fn test_battery_level_ffi() {
        ensure_init();

        let battery_level = nyx_mobile_get_battery_level();
        assert!(
            battery_level >= 0 && battery_level <= 100,
            "Battery level should be 0-100"
        );
    }

    #[test]
    fn test_charging_state_ffi() {
        ensure_init();

        let charging_state = nyx_mobile_is_charging();
        assert!(
            charging_state >= 0 && charging_state <= 1,
            "Charging state should be 0 or 1"
        );
    }
}
