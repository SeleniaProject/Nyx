//
//  NyxMobile.h
//  Nyx Mobile iOS Bridge
//
//  iOS Objective-C bridge for Nyx mobile platform integration
//

#import <Foundation/Foundation.h>
#import <UIKit/UIKit.h>
#import <Network/Network.h>
// Use the generated pure C-ABI header from the Rust crate
#import "../include/nyx_mobile_ffi.h"

NS_ASSUME_NONNULL_BEGIN

// MARK: - Mobile State Types

typedef NS_ENUM(NSInteger, NyxAppState) {
    NyxAppStateActive = 0,
    NyxAppStateBackground = 1,
    NyxAppStateInactive = 2
};

typedef NS_ENUM(NSInteger, NyxNetworkState) {
    NyxNetworkStateWiFi = 0,
    NyxNetworkStateCellular = 1,
    NyxNetworkStateEthernet = 2,
    NyxNetworkStateNone = 3
};

typedef NS_ENUM(NSInteger, NyxPlatform) {
    NyxPlatformOther = 0,
    NyxPlatformiOS = 1,
    NyxPlatformAndroid = 2
};

// MARK: - iOS Bridge Interface

@interface NyxMobileBridge : NSObject

// Initialization
+ (instancetype)sharedInstance;
- (BOOL)initializeMonitoring;
- (void)cleanup;
// Telemetry control (optional)
- (void)startTelemetryIfAvailable;
- (void)stopTelemetryIfAvailable;

/// Inject telemetry labels into native layer (keys like device_model / os_version)
- (void)injectTelemetryLabels;

// Battery Monitoring
@property (nonatomic, readonly) NSInteger batteryLevel;
@property (nonatomic, readonly) BOOL isCharging;
@property (nonatomic, readonly) BOOL isBatteryMonitoringEnabled;
- (void)enableBatteryMonitoring:(BOOL)enabled;

// Power Management
@property (nonatomic, readonly) BOOL isLowPowerModeEnabled;
@property (nonatomic, readonly) BOOL isScreenOn;

// App State Monitoring
@property (nonatomic, readonly) NyxAppState appState;
- (void)registerForAppStateNotifications;
- (void)unregisterFromAppStateNotifications;

// Network Monitoring
@property (nonatomic, readonly) NyxNetworkState networkState;
- (void)startNetworkMonitoring;
- (void)stopNetworkMonitoring;

// Notification Callbacks
- (void)onBatteryLevelChanged:(NSInteger)level;
- (void)onChargingStateChanged:(BOOL)charging;
- (void)onLowPowerModeChanged:(BOOL)lowPowerMode;
- (void)onAppStateChanged:(NyxAppState)state;
- (void)onNetworkStateChanged:(NyxNetworkState)state;

@end

// No additional C bridge declarations are needed; we directly call
// the C-ABI functions declared in nyx_mobile_ffi.h from the Objective-C
// implementation where appropriate.

NS_ASSUME_NONNULL_END
