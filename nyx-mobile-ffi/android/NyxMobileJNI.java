package com.nyx.mobile;

/**
 * NyxMobileJNI (legacy shim)
 *
 * This class previously declared many JNI-native methods. The project now
 * exposes a stable pure C-ABI via nyx_mobile_ffi.h and no longer provides
 * JNI entry points. Keeping this class as a minimal placeholder avoids
 * breaking older app code during migration. New code should call the C-ABI
 * directly via the Android NDK or a thin Kotlin/NDK wrapper.
 */
public final class NyxMobileJNI {
    private static final String TAG = "NyxMobileJNI";
    private NyxMobileJNI() {}

    // Compatibility no-ops: return success and log a warning.
    public static int nativeInit() { android.util.Log.w(TAG, "nativeInit is deprecated; call C-ABI nyx_mobile_init()"); return 0; }
    public static void nativeCleanup() { android.util.Log.w(TAG, "nativeCleanup is deprecated; call C-ABI nyx_mobile_shutdown()"); }
}
