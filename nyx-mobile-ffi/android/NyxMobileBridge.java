package com.nyx.mobile;

import android.app.Activity;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.BatteryManager;
import android.os.Build;
import android.os.PowerManager;
import android.util.Log;

/**
 * NyxMobileBridge - Android native bridge for Nyx mobile platform integration
 * 
 * This class provides Android-specific implementations for battery monitoring,
 * power management, and app lifecycle tracking using the stable C ABI.
 */
public class NyxMobileBridge {
    private static final String TAG = "NyxMobileBridge";
    
    // Power state constants matching Rust enum
    public static final int POWER_STATE_ACTIVE = 0;
    public static final int POWER_STATE_BACKGROUND = 1;
    public static final int POWER_STATE_INACTIVE = 2;
    public static final int POWER_STATE_CRITICAL = 3;
    
    // Singleton instance
    private static NyxMobileBridge instance;
    
    // Android system services
    private Context context;
    private BatteryManager batteryManager;
    private PowerManager powerManager;
    
    // Broadcast receivers
    private BatteryBroadcastReceiver batteryReceiver;
    private PowerSaveBroadcastReceiver powerSaveReceiver;
    
    // Power state callback interface
    public interface PowerStateCallback {
        void onPowerStateChanged(int newState);
        void onScreenOffRatioChanged(float ratio);
    }
    
    private PowerStateCallback callback;
    
    // Singleton access
    public static synchronized NyxMobileBridge getInstance() {
        if (instance == null) {
            instance = new NyxMobileBridge();
        }
        return instance;
    }
    
    private NyxMobileBridge() {
        Log.d(TAG, "NyxMobileBridge created");
    }
    
    /**
     * Initialize the mobile bridge with Android context
     */
    public boolean initialize(Context context) {
        Log.d(TAG, "Initializing NyxMobileBridge");
        
        this.context = context.getApplicationContext();
        
        // Get system services
        batteryManager = (BatteryManager) context.getSystemService(Context.BATTERY_SERVICE);
        powerManager = (PowerManager) context.getSystemService(Context.POWER_SERVICE);
        
        if (batteryManager == null || powerManager == null) {
            Log.e(TAG, "Failed to get required system services");
            return false;
        }
        
        // Initialize C ABI
        int result = nyx_mobile_init();
        if (result != 0) {
            Log.e(TAG, "Failed to initialize Nyx mobile C ABI: " + result);
            return false;
        }
        
        // Initialize monitoring
        startBatteryMonitoring();
        startPowerSaveMonitoring();
        
        Log.d(TAG, "NyxMobileBridge initialization complete");
        return true;
    }
    
    /**
     * Set power state callback
     */
    public void setCallback(PowerStateCallback callback) {
        this.callback = callback;
    }
    
    /**
     * Cleanup resources
     */
    public void shutdown() {
        Log.d(TAG, "Cleaning up NyxMobileBridge");
        
        stopBatteryMonitoring();
        stopPowerSaveMonitoring();
        
        nyx_mobile_shutdown();
        callback = null;
    }
    
    /**
     * Handle app resume
     */
    public void onAppResume() {
        nyx_power_set_state(POWER_STATE_ACTIVE);
        nyx_push_wake();
        
        if (callback != null) {
            callback.onPowerStateChanged(POWER_STATE_ACTIVE);
        }
    }
    
    /**
     * Handle app pause
     */
    public void onAppPause() {
        nyx_power_set_state(POWER_STATE_BACKGROUND);
        
        if (callback != null) {
            callback.onPowerStateChanged(POWER_STATE_BACKGROUND);
        }
    }
    
    // MARK: - Battery Monitoring
    
    public int getBatteryLevel() {
        if (batteryManager == null) return -1;
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            return batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY);
        } else {
            // Fallback for older Android versions
            IntentFilter filter = new IntentFilter(Intent.ACTION_BATTERY_CHANGED);
            Intent batteryStatus = context.registerReceiver(null, filter);
            if (batteryStatus == null) return -1;
            
            int level = batteryStatus.getIntExtra(BatteryManager.EXTRA_LEVEL, -1);
            int scale = batteryStatus.getIntExtra(BatteryManager.EXTRA_SCALE, -1);
            
            if (level >= 0 && scale > 0) {
                return (int) ((level / (float) scale) * 100);
            }
            return -1;
        }
    }
    
    public boolean isCharging() {
        if (batteryManager == null) return false;
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            int status = batteryManager.getIntProperty(BatteryManager.BATTERY_PROPERTY_STATUS);
            return status == BatteryManager.BATTERY_STATUS_CHARGING || 
                   status == BatteryManager.BATTERY_STATUS_FULL;
        } else {
            // Fallback for older Android versions
            IntentFilter filter = new IntentFilter(Intent.ACTION_BATTERY_CHANGED);
            Intent batteryStatus = context.registerReceiver(null, filter);
            if (batteryStatus == null) return false;
            
            int status = batteryStatus.getIntExtra(BatteryManager.EXTRA_STATUS, -1);
            return status == BatteryManager.BATTERY_STATUS_CHARGING ||
                   status == BatteryManager.BATTERY_STATUS_FULL;
        }
    }
    
    private void startBatteryMonitoring() {
        if (context == null) return;
        
        batteryReceiver = new BatteryBroadcastReceiver();
        IntentFilter filter = new IntentFilter();
        filter.addAction(Intent.ACTION_BATTERY_CHANGED);
        filter.addAction(Intent.ACTION_POWER_CONNECTED);
        filter.addAction(Intent.ACTION_POWER_DISCONNECTED);
        
        context.registerReceiver(batteryReceiver, filter);
        Log.d(TAG, "Battery monitoring started");
    }
    
    private void stopBatteryMonitoring() {
        if (context != null && batteryReceiver != null) {
            context.unregisterReceiver(batteryReceiver);
            batteryReceiver = null;
            Log.d(TAG, "Battery monitoring stopped");
        }
    }
    
    // MARK: - Power Management
    
    public boolean isScreenOn() {
        if (powerManager == null) return true;
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.KITKAT_WATCH) {
            return powerManager.isInteractive();
        } else {
            @SuppressWarnings("deprecation")
            boolean screenOn = powerManager.isScreenOn();
            return screenOn;
        }
    }
    
    public boolean isPowerSaveMode() {
        if (powerManager == null) return false;
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            return powerManager.isPowerSaveMode();
        }
        return false;
    }
    
    private void startPowerSaveMonitoring() {
        if (context == null || Build.VERSION.SDK_INT < Build.VERSION_CODES.LOLLIPOP) return;
        
        powerSaveReceiver = new PowerSaveBroadcastReceiver();
        IntentFilter filter = new IntentFilter();
        filter.addAction(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED);
        
        context.registerReceiver(powerSaveReceiver, filter);
        Log.d(TAG, "Power save monitoring started");
    }
    
    private void stopPowerSaveMonitoring() {
        if (context != null && powerSaveReceiver != null) {
            context.unregisterReceiver(powerSaveReceiver);
            powerSaveReceiver = null;
            Log.d(TAG, "Power save monitoring stopped");
        }
    }
    
    // MARK: - Broadcast Receivers
    
    private class BatteryBroadcastReceiver extends BroadcastReceiver {
        @Override
        public void onReceive(Context context, Intent intent) {
            String action = intent.getAction();
            if (action == null) return;
            
            switch (action) {
                case Intent.ACTION_BATTERY_CHANGED:
                    int level = getBatteryLevel();
                    if (level >= 0) {
                        // Update power state based on battery level
                        if (level < 15) {
                            nyx_power_set_state(POWER_STATE_CRITICAL);
                            if (callback != null) {
                                callback.onPowerStateChanged(POWER_STATE_CRITICAL);
                            }
                        }
                    }
                    break;
                    
                case Intent.ACTION_POWER_CONNECTED:
                case Intent.ACTION_POWER_DISCONNECTED:
                    boolean charging = isCharging();
                    Log.d(TAG, "Charging state changed: " + charging);
                    break;
            }
        }
    }
    
    private class PowerSaveBroadcastReceiver extends BroadcastReceiver {
        @Override
        public void onReceive(Context context, Intent intent) {
            if (PowerManager.ACTION_POWER_SAVE_MODE_CHANGED.equals(intent.getAction())) {
                boolean powerSaveMode = isPowerSaveMode();
                Log.d(TAG, "Power save mode changed: " + powerSaveMode);
                
                if (powerSaveMode) {
                    nyx_power_set_state(POWER_STATE_CRITICAL);
                    if (callback != null) {
                        callback.onPowerStateChanged(POWER_STATE_CRITICAL);
                    }
                }
            }
        }
    }
    
    // MARK: - Native C ABI Functions
    
    // Load native library
    static {
        System.loadLibrary("nyx_mobile_ffi");
    }
    
    // Native method declarations
    private static native int nyx_mobile_init();
    private static native int nyx_mobile_shutdown();
    private static native int nyx_power_set_state(int state);
    private static native int nyx_power_get_state(int[] outState);
    private static native int nyx_push_wake();
    private static native int nyx_resume_low_power_session();
    private static native int nyx_mobile_set_telemetry_label(String key, String value);
    private static native int nyx_mobile_clear_telemetry_labels();
}