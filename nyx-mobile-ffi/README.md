# Nyx Mobile FFI

Pure Rust C-ABI for mobile platform integration. Provides a minimal, stable surface callable from Kotlin/Swift without JNI/Objective-C dependencies.

## Overview

This crate exposes:

- Initialization/shutdown and logging level control
- Power/app lifecycle state input (Active/Background/Inactive/Critical)
- Push wake and explicit resume triggers for low-power reactivation
- Version and last-error retrieval helpers

No C/C++ libraries are required. See `include/nyx_mobile_ffi.h` for the C header.

## Public API (C)

Key functions (see header for full details):

- `int nyx_mobile_init(void);`
- `int nyx_mobile_shutdown(void);`
- `int nyx_mobile_set_log_level(int level); // 0..4`
- `int nyx_mobile_version(char* buf, size_t buf_len);`
- `int nyx_mobile_last_error(char* buf, size_t buf_len);`
- `int nyx_power_set_state(uint32_t state); // 0..3`
- `int nyx_power_get_state(uint32_t* out_state);`
- `int nyx_push_wake(void);`
- `int nyx_resume_low_power_session(void);`

Status codes:

- `0`: OK
- `1`: Already initialized
- `2`: Not initialized
- `3`: Invalid argument
- `4`: Internal error

## Using from Kotlin/Android

This crate exposes a pure C ABI. On Android, call it via a minimal JNI stub in your app's NDK layer (thin forwarders only). The legacy `android/*.java` in this crate are deprecated and no longer backed by native symbols.

Guideline:
- Link `libnyx_mobile_ffi.so` in your app's CMake/ndk-build.
- Write tiny JNI methods that call C functions: `nyx_mobile_init`, `nyx_power_set_state`, `nyx_push_wake`, `nyx_resume_low_power_session`.
- Keep JNI surface minimal and stateless; do not mirror Android APIs—just forward lifecycle events to the C ABI.

Call from a background-safe context respecting Android’s background policies.

## Using from Swift/iOS

Add `include/nyx_mobile_ffi.h` to your bridging header. The legacy Objective‑C bridge in `ios/` now forwards directly to the C ABI and avoids extra surface.

```swift
let rc = nyx_mobile_init()
if rc != 0 { /* handle error */ }
```

## Build

```bash
cargo build -p nyx-mobile-ffi
cargo test  -p nyx-mobile-ffi
```

Cross-compilation targets can be set as usual for iOS/Android. No extra C deps are needed.

## Telemetry

If built with `--features telemetry`, counters are emitted for power state sets, wakes, and resumes via the workspace telemetry stack.

Note: Collector startup/labeling is owned by the daemon or host; mobile bridges no longer attempt in-process collectors.

## Safety

- C-ABI with simple types only; explicit buffer length contracts
- Thread-safe global state; idempotent init/shutdown
- Errors accessible via `nyx_mobile_last_error`

## License

MIT License - see [LICENSE](../LICENSE) for details.
