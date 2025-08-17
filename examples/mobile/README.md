# Mobile Integration Examples (C-ABI)

This folder contains platform-side examples showing how to call Nyx's pure C ABI from Android and iOS.

Highlights:
- Keep platform bridges thin. Do not mirror platform APIs; just forward lifecycle/power events.
- Call these C functions:
  - nyx_mobile_init / nyx_mobile_shutdown
  - nyx_mobile_set_log_level
  - nyx_power_set_state (0=Active,1=Background,2=Inactive,3=Critical)
  - nyx_push_wake / nyx_resume_low_power_session
  - nyx_mobile_set_telemetry_label / nyx_mobile_clear_telemetry_labels (optional)

See:
- android/README.md
- ios/README.md
