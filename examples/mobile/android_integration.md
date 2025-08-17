# Android Integration Guide

This guide shows how to integrate Nyx mobile client using the stable C ABI via JNI.

## Architecture

```
Android App (Java/Kotlin)
    ↓ JNI calls  
libnyx_mobile_ffi.so (Rust C-ABI)
    ↓ 
Nyx Core (nyx-mobile-ffi crate)
```

## Step 1: Native Library Setup

### CMakeLists.txt
```cmake
cmake_minimum_required(VERSION 3.10.2)
project("nyxmobile")

add_library(nyxmobile SHARED native-lib.cpp)

find_library(log-lib log)

# Link against prebuilt Nyx mobile FFI library
add_library(nyx_mobile_ffi SHARED IMPORTED)
set_target_properties(nyx_mobile_ffi PROPERTIES
    IMPORTED_LOCATION ${CMAKE_SOURCE_DIR}/src/main/jniLibs/${ANDROID_ABI}/libnyx_mobile_ffi.so
)

target_link_libraries(nyxmobile 
    ${log-lib}
    nyx_mobile_ffi
)
```

### native-lib.cpp
```cpp
#include <jni.h>
#include <string>
#include <android/log.h>
#include "nyx_mobile_ffi.h"

#define LOG_TAG "NyxNative"
#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, LOG_TAG, __VA_ARGS__)

extern "C" JNIEXPORT jint JNICALL
Java_com_nyx_mobile_NyxMobileBridge_nyx_1mobile_1init(JNIEnv *env, jclass clazz) {
    return nyx_mobile_init();
}

extern "C" JNIEXPORT jint JNICALL
Java_com_nyx_mobile_NyxMobileBridge_nyx_1mobile_1shutdown(JNIEnv *env, jclass clazz) {
    return nyx_mobile_shutdown();
}

extern "C" JNIEXPORT jint JNICALL
Java_com_nyx_mobile_NyxMobileBridge_nyx_1power_1set_1state(JNIEnv *env, jclass clazz, jint state) {
    return nyx_power_set_state(static_cast<uint32_t>(state));
}

extern "C" JNIEXPORT jint JNICALL
Java_com_nyx_mobile_NyxMobileBridge_nyx_1power_1get_1state(JNIEnv *env, jclass clazz, jintArray out_state) {
    if (out_state == nullptr) return 3; // InvalidArgument
    
    uint32_t state;
    int result = nyx_power_get_state(&state);
    
    if (result == 0) {
        jint java_state = static_cast<jint>(state);
        env->SetIntArrayRegion(out_state, 0, 1, &java_state);
    }
    
    return result;
}

extern "C" JNIEXPORT jint JNICALL
Java_com_nyx_mobile_NyxMobileBridge_nyx_1push_1wake(JNIEnv *env, jclass clazz) {
    return nyx_push_wake();
}

extern "C" JNIEXPORT jint JNICALL
Java_com_nyx_mobile_NyxMobileBridge_nyx_1resume_1low_1power_1session(JNIEnv *env, jclass clazz) {
    return nyx_resume_low_power_session();
}

extern "C" JNIEXPORT jint JNICALL
Java_com_nyx_mobile_NyxMobileBridge_nyx_1mobile_1set_1telemetry_1label(
    JNIEnv *env, jclass clazz, jstring key, jstring value) {
    
    if (key == nullptr) return 3; // InvalidArgument
    
    const char* key_str = env->GetStringUTFChars(key, nullptr);
    const char* value_str = value ? env->GetStringUTFChars(value, nullptr) : nullptr;
    
    int result = nyx_mobile_set_telemetry_label(key_str, value_str);
    
    env->ReleaseStringUTFChars(key, key_str);
    if (value_str) env->ReleaseStringUTFChars(value, value_str);
    
    return result;
}
```

## Step 2: Application Integration

### MainActivity.java
```java
package com.example.nyxdemo;

import android.app.Application;
import android.os.Bundle;
import android.util.Log;
import androidx.appcompat.app.AppCompatActivity;
import com.nyx.mobile.NyxMobileBridge;

public class MainActivity extends AppCompatActivity {
    private static final String TAG = "NyxDemo";
    private NyxMobileBridge nyxBridge;
    
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);
        
        // Initialize Nyx bridge
        nyxBridge = new NyxMobileBridge(this);
        nyxBridge.setCallback(new NyxMobileBridge.PowerStateCallback() {
            @Override
            public void onPowerStateChanged(int newState) {
                Log.i(TAG, "Power state changed to: " + newState);
                // Update UI based on power state
                updatePowerStateUI(newState);
            }
            
            @Override
            public void onScreenOffRatioChanged(float ratio) {
                Log.d(TAG, "Screen off ratio: " + ratio);
                // Adapt behavior based on usage patterns
                if (ratio > 0.8f) {
                    // User keeps screen off frequently - apply aggressive power savings
                    enableAggressivePowerSaving();
                }
            }
        });
        
        boolean success = nyxBridge.initialize();
        if (!success) {
            Log.e(TAG, "Failed to initialize Nyx mobile bridge");
        } else {
            Log.i(TAG, "Nyx mobile bridge initialized successfully");
        }
    }
    
    @Override
    protected void onResume() {
        super.onResume();
        if (nyxBridge != null) {
            nyxBridge.onAppResume();
        }
    }
    
    @Override
    protected void onPause() {
        super.onPause();
        if (nyxBridge != null) {
            nyxBridge.onAppPause();
        }
    }
    
    @Override
    protected void onDestroy() {
        super.onDestroy();
        if (nyxBridge != null) {
            nyxBridge.shutdown();
        }
    }
    
    private void updatePowerStateUI(int powerState) {
        runOnUiThread(() -> {
            switch (powerState) {
                case NyxMobileBridge.POWER_STATE_ACTIVE:
                    // Show full functionality
                    setStatusText("Active - Full functionality");
                    enableAllFeatures();
                    break;
                case NyxMobileBridge.POWER_STATE_BACKGROUND:
                    // Reduce background activity
                    setStatusText("Background - Reduced activity");
                    limitBackgroundFeatures();
                    break;
                case NyxMobileBridge.POWER_STATE_INACTIVE:
                    // Minimal activity
                    setStatusText("Inactive - Minimal activity");
                    enableMinimalFeatures();
                    break;
                case NyxMobileBridge.POWER_STATE_CRITICAL:
                    // Critical power saving
                    setStatusText("Critical - Power saving mode");
                    enableCriticalPowerSaving();
                    break;
            }
        });
    }
    
    private void enableAggressivePowerSaving() {
        // Implement aggressive power saving based on user behavior
        Log.i(TAG, "Enabling aggressive power saving mode");
    }
    
    private void setStatusText(String status) {
        // Update UI status indicator
    }
    
    private void enableAllFeatures() {
        // Enable full app functionality
    }
    
    private void limitBackgroundFeatures() {
        // Reduce background processing
    }
    
    private void enableMinimalFeatures() {
        // Keep only essential features active
    }
    
    private void enableCriticalPowerSaving() {
        // Maximum power savings
    }
}
```

## Step 3: Power Policy Implementation

The screen off ratio tracking provides data for adaptive power policies:

```java
// Example power policy based on usage patterns
public class NyxPowerPolicy {
    private static final float HIGH_SCREEN_OFF_THRESHOLD = 0.8f;
    private static final float MEDIUM_SCREEN_OFF_THRESHOLD = 0.5f;
    
    public static void adaptPowerBehavior(float screenOffRatio, int batteryLevel) {
        if (screenOffRatio > HIGH_SCREEN_OFF_THRESHOLD || batteryLevel < 15) {
            // User frequently has screen off or low battery
            // Apply aggressive power optimization
            enableAggressiveMode();
        } else if (screenOffRatio > MEDIUM_SCREEN_OFF_THRESHOLD || batteryLevel < 30) {
            // Moderate power optimization
            enableBalancedMode();
        } else {
            // Normal operation
            enableNormalMode();
        }
    }
    
    private static void enableAggressiveMode() {
        // Reduce background sync frequency
        // Limit network usage
        // Reduce cover traffic generation
    }
    
    private static void enableBalancedMode() {
        // Moderate reductions in background activity
    }
    
    private static void enableNormalMode() {
        // Full functionality
    }
}
```

## Important Notes

1. **No Legacy JNI**: The old `NyxMobileJNI` class has been removed. Use the C ABI directly.

2. **WorkManager Integration**: For background processing, use Android's WorkManager with Nyx power state awareness:

```java
// Schedule work based on power state
if (powerState == NyxMobileBridge.POWER_STATE_ACTIVE) {
    WorkManager.getInstance(context)
        .enqueue(OneTimeWorkRequest.from(SyncWorker.class));
} else {
    // Defer non-critical work
}
```

3. **Battery Optimization**: Respect Android's battery optimization features:
   - Handle Doze mode gracefully
   - Use foreground services for critical operations
   - Implement adaptive behavior based on screen off ratio

4. **Testing**: Test power state transitions and screen off ratio calculations:
   - Simulate various usage patterns
   - Verify proper state transitions
   - Validate telemetry data accuracy
