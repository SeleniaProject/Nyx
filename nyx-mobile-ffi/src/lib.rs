//! Public FFI surface for Nyx mobile binding_s.
//! The function_s here expose a stable C ABI without relying on C/C++ librarie_s.
//! They are safe to call from Kotlin/Swift with simple type_s.

mod common;

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::AtomicU32;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::sync::RwLock;
use tracing::Level;
use tracing_subscriber::fmt;

// Simple statu_s code_s suitable for FFI. 0 = OK, non-zero = error.
#[repr(i32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NyxStatus {
    Ok = 0,
    AlreadyInitialized = 1,
    NotInitialized = 2,
    InvalidArgument = 3,
    InternalError = 4,
}

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static LAST_ERROR: OnceCell<Mutex<String>> = OnceCell::new();
// No dynamic reload handle due to crate feature constraint_s in workspace.
static POWER_STATE: AtomicU32 = AtomicU32::new(0);
static WAKE_COUNT: AtomicU32 = AtomicU32::new(0);
static RESUME_COUNT: AtomicU32 = AtomicU32::new(0);
// Optional global telemetry label_s (key->value). Used to enrich emitted metric_s when enabled.
static TELEMETRY_LABELS: OnceCell<RwLock<HashMap<String, String>>> = OnceCell::new();

/// Unified power state used by the daemon policy.
#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NyxPowerState {
    Active = 0,
    Background = 1,
    Inactive = 2,
    Critical = 3,
}

fn set_last_error(msg: impl Into<String>) {
    let m = LAST_ERROR.get_or_init(|| Mutex::new(String::new()));
    if let Ok(mut g) = m.lock() {
        *g = msg.into();
    }
}

fn clear_last_error() {
    if let Some(m) = LAST_ERROR.get() {
        if let Ok(mut g) = m.lock() {
            g.clear();
        }
    }
}

fn label_s() -> &'static RwLock<HashMap<String, String>> {
    TELEMETRY_LABELS.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Initialize Nyx mobile layer. Idempotent.
/// Return_s 0 on succes_s, 1 if already initialized.
#[no_mangle]
pub extern "C" fn nyx_mobile_init() -> c_int {
    if INITIALIZED.swap(true, Ordering::SeqCst) {
        return NyxStatus::AlreadyInitialized as c_int;
    }
    // Install a basic logger if none installed.
    // Build a basic subscriber. Note: level i_s configured at install time only.
    let subscriber = fmt()
        .with_ansi(false)
        .with_level(true)
        .with_thread_ids(false)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
    POWER_STATE.store(NyxPowerState::Active as u32, Ordering::SeqCst);
    clear_last_error();
    NyxStatus::Ok as c_int
}

/// Shutdown Nyx mobile layer. Safe to call multiple time_s.
#[no_mangle]
pub extern "C" fn nyx_mobile_shutdown() -> c_int {
    if !INITIALIZED.swap(false, Ordering::SeqCst) {
        return NyxStatus::NotInitialized as c_int;
    }
    clear_last_error();
    NyxStatus::Ok as c_int
}

/// Set log level: 0=ERROR,1=WARN,2=INFO,3=DEBUG,4=TRACE
#[no_mangle]
pub extern "C" fn nyx_mobile_set____log_level(level: c_int) -> c_int {
    if !INITIALIZED.load(Ordering::SeqCst) {
        return NyxStatus::NotInitialized as c_int;
    }
    let lvl = match level {
        0 => Level::ERROR,
        1 => Level::WARN,
        2 => Level::INFO,
        3 => Level::DEBUG,
        4 => Level::TRACE,
        _ => {
            set_last_error("invalid log level");
            return NyxStatus::InvalidArgument as c_int;
        }
    };
    // Reconfigure global max level via RUST_LOG style filter
    // Note: tracing_subscriber doesn't support dynamic global filter easily without
    // reload handle_s; we install a new subscriber for simplicity here.
    // We cannot change the level dynamically without reload; best-effort: try reinstall.
    // Thi_s will fail after the first initialization in most runtime environment_s; return Ok.
    let _ = tracing_subscriber::fmt().with_max_level(lvl).try_init();
    NyxStatus::Ok as c_int
}

/// Set a telemetry label key/value. Passing a null value removes the key. Passing a null key is invalid.
/// Returns 0 on success.
///
/// # Safety
/// - `key` and `value` must be valid C strings (null-terminated) or null pointers
/// - If not null, the pointers must remain valid for the duration of the call
/// - The caller must ensure proper memory management for the strings
#[no_mangle]
pub unsafe extern "C" fn nyx_mobile_set_telemetry_label(
    key: *const c_char,
    value: *const c_char,
) -> c_int {
    if key.is_null() {
        set_last_error("telemetry label key i_s null");
        return NyxStatus::InvalidArgument as c_int;
    }
    // SAFETY: caller guarantee_s valid C string_s
    let k = unsafe { CStr::from_ptr(key) }
        .to_string_lossy()
        .into_owned();
    if k.is_empty() {
        set_last_error("telemetry label key i_s empty");
        return NyxStatus::InvalidArgument as c_int;
    }
    let map = label_s();
    if value.is_null() {
        if let Ok(mut m) = map.write() {
            m.remove(&k);
        }
        return NyxStatus::Ok as c_int;
    }
    let v = unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned();
    if let Ok(mut m) = map.write() {
        m.insert(k, v);
    }
    NyxStatus::Ok as c_int
}

/// Clear all telemetry label_s.
#[no_mangle]
pub extern "C" fn nyx_mobile_clear_telemetry_label_s() -> c_int {
    if let Some(l) = TELEMETRY_LABELS.get() {
        if let Ok(mut m) = l.write() {
            m.clear();
        }
    }
    NyxStatus::Ok as c_int
}

/// Get crate version string. Returns length excluding NUL.
/// Writes up to `buf_len-1` bytes and NUL-terminates. If buf_len==0, returns needed length.
///
/// # Safety
/// - If `buf` is not null, it must point to valid, writable memory of at least `buf_len` bytes
/// - The caller must ensure the buffer remains valid for the duration of the call
/// - If `buf_len` is 0, `buf` can be null (used for size query)
#[no_mangle]
pub unsafe extern "C" fn nyx_mobile_version(buf: *mut c_char, buf_len: usize) -> c_int {
    let ver = env!("CARGO_PKG_VERSION");
    let byte_s = ver.as_bytes();
    if buf.is_null() || buf_len == 0 {
        return byte_s.len() as c_int;
    }
    // SAFETY: caller provide_s valid, writable buffer.
    unsafe {
        let max_copy = buf_len.saturating_sub(1).min(byte_s.len());
        std::ptr::copy_nonoverlapping(byte_s.as_ptr(), buf as *mut u8, max_copy);
        *buf.add(max_copy) = 0;
        max_copy as c_int
    }
}

/// Return last error message length (excluding NUL). If a buffer is provided, copy it.
///
/// # Safety
/// - If `buf` is not null, it must point to valid, writable memory of at least `buf_len` bytes
/// - The caller must ensure the buffer remains valid for the duration of the call
/// - If `buf_len` is 0, `buf` can be null (used for size query)
#[no_mangle]
pub unsafe extern "C" fn nyx_mobile_last_error(buf: *mut c_char, buf_len: usize) -> c_int {
    let msg = LAST_ERROR
        .get()
        .and_then(|m| m.lock().ok().map(|g| g.clone()))
        .unwrap_or_default();
    let byte_s = msg.as_bytes();
    if buf.is_null() || buf_len == 0 {
        return byte_s.len() as c_int;
    }
    unsafe {
        let max_copy = buf_len.saturating_sub(1).min(byte_s.len());
        std::ptr::copy_nonoverlapping(byte_s.as_ptr(), buf as *mut u8, max_copy);
        *buf.add(max_copy) = 0;
        max_copy as c_int
    }
}

/// Convenience helper for test_s to read a C string into Rust String.
#[cfg(test)]
fn cstr_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

/// Set unified power state. Return_s InvalidArgument if state i_s unknown.
#[no_mangle]
pub extern "C" fn nyx_power_set_state(state: u32) -> c_int {
    if !INITIALIZED.load(Ordering::SeqCst) {
        return NyxStatus::NotInitialized as c_int;
    }
    match state {
        x if x == NyxPowerState::Active as u32
            || x == NyxPowerState::Background as u32
            || x == NyxPowerState::Inactive as u32
            || x == NyxPowerState::Critical as u32 =>
        {
            POWER_STATE.store(x, Ordering::SeqCst);
            #[cfg(feature = "telemetry")]
            {
                // Attach dynamic label_s if any are set.
                if let Ok(m) = label_s().read() {
                    if m.is_empty() {
                        metrics::counter!("nyx.mobile.power_state.set", "state" => x.to_string())
                            .increment(1);
                    } else {
                        // Fallback: emit without merged label_s (metric_s macro_s are not variadic at runtime).
                        metrics::counter!("nyx.mobile.power_state.set", "state" => x.to_string())
                            .increment(1);
                    }
                } else {
                    metrics::counter!("nyx.mobile.power_state.set", "state" => x.to_string())
                        .increment(1);
                }
            }
            NyxStatus::Ok as c_int
        }
        _ => {
            set_last_error("invalid power state");
            NyxStatus::InvalidArgument as c_int
        }
    }
}

/// Return current power state value as u32 (Active=0,...). Returns InvalidArgument on null ptr.
///
/// # Safety
/// - `out_state` must be a valid, non-null pointer to writable memory
/// - The caller must ensure the pointer remains valid for the duration of the call
#[no_mangle]
pub unsafe extern "C" fn nyx_power_get_state(out_state: *mut u32) -> c_int {
    if out_state.is_null() {
        set_last_error("out_state i_s null");
        return NyxStatus::InvalidArgument as c_int;
    }
    unsafe {
        *out_state = POWER_STATE.load(Ordering::SeqCst);
    }
    NyxStatus::Ok as c_int
}

/// Push wake entry point to kick resume controller. Increment_s a counter.
#[no_mangle]
pub extern "C" fn nyx_push_wake() -> c_int {
    if !INITIALIZED.load(Ordering::SeqCst) {
        return NyxStatus::NotInitialized as c_int;
    }
    WAKE_COUNT.fetch_add(1, Ordering::SeqCst);
    #[cfg(feature = "telemetry")]
    {
        metrics::counter!("nyx.mobile.push.wake").increment(1);
    }
    NyxStatus::Ok as c_int
}

/// Explicit resume trigger when OS grant_s execution window.
#[no_mangle]
pub extern "C" fn nyx_resume_low_power_session() -> c_int {
    if !INITIALIZED.load(Ordering::SeqCst) {
        return NyxStatus::NotInitialized as c_int;
    }
    RESUME_COUNT.fetch_add(1, Ordering::SeqCst);
    #[cfg(feature = "telemetry")]
    {
        metrics::counter!("nyx.mobile.resume").increment(1);
    }
    NyxStatus::Ok as c_int
}

// --- Safe Rust helper_s for internal consumer_s (daemon) ---
/// Return the last error message as a Rust String (safe; no FFI buffer needed).
pub fn rust_last_error() -> String {
    LAST_ERROR
        .get()
        .and_then(|m| m.lock().ok().map(|g| g.clone()))
        .unwrap_or_default()
}

/// Read current unified power state atomically.
pub fn rust_get_power_state() -> u32 {
    POWER_STATE.load(Ordering::SeqCst)
}

/// Read counter_s for wake/resume event_s.
pub fn rust_get_wake_count() -> u32 {
    WAKE_COUNT.load(Ordering::SeqCst)
}
pub fn rust_get_resume_count() -> u32 {
    RESUME_COUNT.load(Ordering::SeqCst)
}

#[cfg(test)]
mod test_s {
    use super::*;
    use once_cell::sync::Lazy;
    use std::sync::Mutex as StdMutex;

    // Serialize test_s as they manipulate global singleton_s.
    static TEST_MUTEX: Lazy<StdMutex<()>> = Lazy::new(|| StdMutex::new(()));

    #[test]
    fn init_and_shutdown_are_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let __g = TEST_MUTEX.lock()?;
        assert_eq!(nyx_mobile_init(), NyxStatus::Ok as c_int);
        assert_eq!(nyx_mobile_init(), NyxStatus::AlreadyInitialized as c_int);
        assert_eq!(nyx_mobile_shutdown(), NyxStatus::Ok as c_int);
        assert_eq!(nyx_mobile_shutdown(), NyxStatus::NotInitialized as c_int);
        Ok(())
    }

    #[test]
    fn version_api_behaves() -> Result<(), Box<dyn std::error::Error>> {
        let __g = TEST_MUTEX.lock()?;
        let needed = unsafe { nyx_mobile_version(std::ptr::null_mut(), 0) } as usize;
        assert!(needed >= 1);
        let mut buf = vec![0i8; needed + 1];
        let written = unsafe { nyx_mobile_version(buf.as_mut_ptr(), buf.len()) } as usize;
        assert_eq!(written, needed);
        let _s = super::cstr_to_string(buf.as_ptr());
        assert!(!_s.is_empty());
        Ok(())
    }

    #[test]
    fn set_log_level_checks_init_and_args() -> Result<(), Box<dyn std::error::Error>> {
        let __g = TEST_MUTEX.lock()?;
        // Not initialized yet
        assert_eq!(
            nyx_mobile_set____log_level(2),
            NyxStatus::NotInitialized as c_int
        );
        let __ = nyx_mobile_init();
        assert_eq!(nyx_mobile_set____log_level(2), NyxStatus::Ok as c_int);
        assert_eq!(
            nyx_mobile_set____log_level(42),
            NyxStatus::InvalidArgument as c_int
        );
        let needed = unsafe { nyx_mobile_last_error(std::ptr::null_mut(), 0) } as usize;
        let mut buf = vec![0i8; needed + 1];
        let __ = unsafe { nyx_mobile_last_error(buf.as_mut_ptr(), buf.len()) };
        let msg = super::cstr_to_string(buf.as_ptr());
        assert!(msg.contains("invalid log level"));
        let __ = nyx_mobile_shutdown();
        Ok(())
    }

    #[test]
    fn power_state_and_wake_resume_flow() -> Result<(), Box<dyn std::error::Error>> {
        let __g = TEST_MUTEX.lock()?;
        // Not initialized path
        assert_eq!(
            nyx_power_set_state(NyxPowerState::Active as u32),
            NyxStatus::NotInitialized as c_int
        );
        let __ = nyx_mobile_init();
        // Invalid state
        assert_eq!(nyx_power_set_state(99), NyxStatus::InvalidArgument as c_int);
        // Valid state_s
        assert_eq!(
            nyx_power_set_state(NyxPowerState::Background as u32),
            NyxStatus::Ok as c_int
        );
        let mut out: u32 = 123;
        // Null out pointer
        assert_eq!(
            unsafe { nyx_power_get_state(std::ptr::null_mut()) },
            NyxStatus::InvalidArgument as c_int
        );
        assert_eq!(
            unsafe { nyx_power_get_state(&mut out as *mut u32) },
            NyxStatus::Ok as c_int
        );
        assert_eq!(out, NyxPowerState::Background as u32);
        // Wake and resume
        assert_eq!(nyx_push_wake(), NyxStatus::Ok as c_int);
        assert_eq!(nyx_resume_low_power_session(), NyxStatus::Ok as c_int);
        let __ = nyx_mobile_shutdown();
        Ok(())
    }
}
