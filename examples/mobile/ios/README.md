# iOS (Swift + Bridging Header)

Steps:
1) Add nyx-mobile-ffi/include/nyx_mobile_ffi.h to your bridging header.
2) Call C functions directly from Swift:

```swift
import Foundation

func nyxInit() {
  let rc = nyx_mobile_init()
  guard rc == 0 else { print("init failed: \(rc)"); return }
  _ = nyx_mobile_set_log_level(2) // info
}

func onSceneActive() { _ = nyx_power_set_state(0); _ = nyx_resume_low_power_session() }
func onSceneBackground() { _ = nyx_power_set_state(1) }
func onSceneInactive() { _ = nyx_power_set_state(2) }
```

Notes:
- Keep UI thread responsive; call into C-ABI from appropriate lifecycle delegates.
- If using BackgroundTasks, trigger nyx_push_wake/nyx_resume_low_power_session accordingly.
