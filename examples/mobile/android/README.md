# Android (NDK + JNI thin stubs)

Steps:
1) Link libnyx_mobile_ffi.so via CMake/ndk-build.
2) Create a tiny JNI class in your app:

```java
public final class NyxNative {
  static { System.loadLibrary("nyx_mobile_ffi"); }
  private NyxNative() {}
  public static native int nyx_mobile_init();
  public static native int nyx_mobile_shutdown();
  public static native int nyx_power_set_state(int state);
  public static native int nyx_push_wake();
  public static native int nyx_resume_low_power_session();
}
```

3) In your C/C++ JNI implementation, forward to the C ABI declared in include/nyx_mobile_ffi.h.
4) From your Application/Activity lifecycle, call these functions accordingly.

Notes:
- Avoid long-running work in background. Respect Doze/App Standby.
- Use WorkManager/Foreground Service for controlled resumes, coupled with nyx_push_wake/nyx_resume_low_power_session.
