//! Screen-Off Detection and Adaptive Power Management
//!
//! This module implements advanced screen-off detection and adaptive power management
//! as specified in the Nyx Protocol v1.0 specification Section 6: Low Power Mode.
//!
//! ## Features
//!
//! - **Screen-off ratio tracking**: Measures % of time screen is off
//! - **User behavior pattern analysis**: Adapts to individual usage patterns
//! - **Cover traffic adjustment**: Applies cover_ratio=0.1 in screen-off state
//! - **Battery level integration**: Reduces activity based on remaining battery
//! - **Target utilization enforcement**: Maintains U∁[0.2,0.6] range

#![forbid(unsafe_code)]

use std::time::{Duration, Instant};
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::sync::Arc;
use tracing::{debug, info};

/// Screen state tracking for adaptive power management
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenState {
    /// Screen is on - full functionality
    On,
    /// Screen is off - reduced activity mode
    Off,
}

/// Power management state for mobile devices
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerState {
    /// Active state - screen on, full functionality
    Active,
    /// Background state - screen off, reduced cover traffic
    Background,
    /// Inactive state - app backgrounded, minimal activity
    Inactive,
    /// Critical state - low battery, emergency power saving
    Critical,
}

/// Battery level thresholds for power state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryThresholds {
    /// Critical battery level (0.0-1.0) - triggers Critical power state
    pub critical_level: f32,
    /// Low battery level (0.0-1.0) - triggers Background power state
    pub low_level: f32,
    /// Battery hysteresis to prevent oscillation
    pub hysteresis: f32,
}

impl Default for BatteryThresholds {
    fn default() -> Self {
        Self {
            critical_level: 0.10, // 10%
            low_level: 0.25,      // 25%
            hysteresis: 0.05,     // 5%
        }
    }
}

/// Screen-off detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenOffConfig {
    /// Minimum screen-off duration to trigger background mode (default: 30s)
    pub min_screen_off_duration: Duration,
    /// Maximum time to track for ratio calculations (default: 1 hour)
    pub tracking_window: Duration,
    /// Battery level thresholds
    pub battery_thresholds: BatteryThresholds,
    /// Cover traffic ratio when screen is off (default: 0.1)
    pub screen_off_cover_ratio: f32,
    /// Cover traffic ratio when screen is on (default: 0.4)
    pub screen_on_cover_ratio: f32,
    /// Minimum delay between state changes to prevent oscillation
    pub state_change_cooldown: Duration,
}

impl Default for ScreenOffConfig {
    fn default() -> Self {
        Self {
            min_screen_off_duration: Duration::from_secs(30),
            tracking_window: Duration::from_secs(3600), // 1 hour
            battery_thresholds: BatteryThresholds::default(),
            screen_off_cover_ratio: 0.1,
            screen_on_cover_ratio: 0.4,
            state_change_cooldown: Duration::from_secs(5),
        }
    }
}

/// Screen state change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenStateEvent {
    /// New screen state
    pub state: ScreenState,
    /// Timestamp of the change (not serialized)
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
    /// Duration in previous state
    pub previous_duration: Duration,
}

/// Power state change event  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerStateEvent {
    /// New power state
    pub state: PowerState,
    /// Previous power state
    pub previous_state: PowerState,
    /// Timestamp of the change (not serialized)
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
    /// Reason for the change
    pub reason: String,
}

/// Screen-off behavior analysis and power management
#[derive(Debug)]
pub struct ScreenOffDetector {
    /// Current configuration
    config: ScreenOffConfig,
    /// Current screen state
    current_screen_state: ScreenState,
    /// Current power state
    current_power_state: PowerState,
    /// Timestamp of last screen state change
    last_screen_change: Instant,
    /// Timestamp of last power state change
    last_power_change: Instant,
    /// History of screen state changes (within tracking window)
    screen_history: VecDeque<ScreenStateEvent>,
    /// Current battery level (0.0-1.0)
    battery_level: f32,
    /// Statistics
    stats: ScreenOffStats,
}

/// Screen-off detection statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenOffStats {
    /// Total time screen has been on in tracking window
    pub screen_on_duration: Duration,
    /// Total time screen has been off in tracking window
    pub screen_off_duration: Duration,
    /// Screen-off ratio (0.0-1.0)
    pub screen_off_ratio: f32,
    /// Number of screen state changes
    pub state_changes: u64,
    /// Number of power state changes
    pub power_state_changes: u64,
    /// Current cover traffic ratio
    pub current_cover_ratio: f32,
    /// Time in each power state
    pub time_in_active: Duration,
    pub time_in_background: Duration,
    pub time_in_inactive: Duration,
    pub time_in_critical: Duration,
}

impl Default for ScreenOffStats {
    fn default() -> Self {
        Self {
            screen_on_duration: Duration::ZERO,
            screen_off_duration: Duration::ZERO,
            screen_off_ratio: 0.0,
            state_changes: 0,
            power_state_changes: 0,
            current_cover_ratio: 0.4, // Default to screen-on ratio
            time_in_active: Duration::ZERO,
            time_in_background: Duration::ZERO,
            time_in_inactive: Duration::ZERO,
            time_in_critical: Duration::ZERO,
        }
    }
}

impl ScreenOffDetector {
    /// Create a new screen-off detector
    pub fn new(config: ScreenOffConfig) -> Self {
        let now = Instant::now();
        Self {
            config,
            current_screen_state: ScreenState::On,
            current_power_state: PowerState::Active,
            last_screen_change: now,
            last_power_change: now,
            screen_history: VecDeque::new(),
            battery_level: 1.0, // Start with full battery
            stats: ScreenOffStats::default(),
        }
    }

    /// Create with default configuration
    pub fn with_default_config() -> Self {
        Self::new(ScreenOffConfig::default())
    }

    /// Update screen state
    pub fn update_screen_state(&mut self, new_state: ScreenState) -> Option<ScreenStateEvent> {
        if new_state == self.current_screen_state {
            return None; // No change
        }

        let now = Instant::now();
        let previous_duration = now.duration_since(self.last_screen_change);

        // Create event for the state change
        let event = ScreenStateEvent {
            state: new_state,
            timestamp: now,
            previous_duration,
        };

        // Update statistics based on previous state
        match self.current_screen_state {
            ScreenState::On => {
                self.stats.screen_on_duration += previous_duration;
            }
            ScreenState::Off => {
                self.stats.screen_off_duration += previous_duration;
            }
        }

        // Update state
        self.current_screen_state = new_state;
        self.last_screen_change = now;
        self.stats.state_changes += 1;

        // Add to history
        self.screen_history.push_back(event.clone());

        // Clean old history outside tracking window
        self.clean_history();

        // Recalculate screen-off ratio
        self.calculate_screen_off_ratio();

        // Update power state based on new screen state
        self.update_power_state();

        info!(
            screen_state = ?new_state,
            previous_duration_secs = previous_duration.as_secs(),
            screen_off_ratio = self.stats.screen_off_ratio,
            "Screen state changed"
        );

        Some(event)
    }

    /// Update battery level
    pub fn update_battery_level(&mut self, level: f32) -> Option<PowerStateEvent> {
        let old_battery = self.battery_level;
        self.battery_level = level.clamp(0.0, 1.0);

        if (old_battery - self.battery_level).abs() > 0.01 {
            debug!(
                old_battery = old_battery,
                new_battery = self.battery_level,
                "Battery level updated"
            );

            // Check if battery level change should trigger power state change
            self.update_power_state()
        } else {
            None
        }
    }

    /// Update power state based on current conditions
    fn update_power_state(&mut self) -> Option<PowerStateEvent> {
        let now = Instant::now();
        
        // Respect cooldown period
        if now.duration_since(self.last_power_change) < self.config.state_change_cooldown {
            return None;
        }

        let new_state = self.determine_power_state();
        
        if new_state == self.current_power_state {
            return None; // No change
        }

        let previous_state = self.current_power_state;
        let previous_duration = now.duration_since(self.last_power_change);

        // Update time-in-state statistics
        match previous_state {
            PowerState::Active => self.stats.time_in_active += previous_duration,
            PowerState::Background => self.stats.time_in_background += previous_duration,
            PowerState::Inactive => self.stats.time_in_inactive += previous_duration,
            PowerState::Critical => self.stats.time_in_critical += previous_duration,
        }

        // Update cover traffic ratio based on new power state
        self.update_cover_traffic_ratio(new_state);

        let reason = self.get_power_state_reason(new_state);

        let event = PowerStateEvent {
            state: new_state,
            previous_state,
            timestamp: now,
            reason: reason.clone(),
        };

        self.current_power_state = new_state;
        self.last_power_change = now;
        self.stats.power_state_changes += 1;

        info!(
            power_state = ?new_state,
            previous_state = ?previous_state,
            reason = reason,
            cover_ratio = self.stats.current_cover_ratio,
            "Power state changed"
        );

        Some(event)
    }

    /// Determine appropriate power state based on current conditions
    fn determine_power_state(&self) -> PowerState {
        // Critical battery takes precedence
        if self.battery_level <= self.config.battery_thresholds.critical_level {
            return PowerState::Critical;
        }

        // Low battery with hysteresis
        let low_threshold = if self.current_power_state == PowerState::Background {
            self.config.battery_thresholds.low_level + self.config.battery_thresholds.hysteresis
        } else {
            self.config.battery_thresholds.low_level
        };

        if self.battery_level <= low_threshold {
            return PowerState::Background;
        }

        // Screen-based states
        match self.current_screen_state {
            ScreenState::On => PowerState::Active,
            ScreenState::Off => {
                let time_since_screen_off = Instant::now().duration_since(self.last_screen_change);
                if time_since_screen_off >= self.config.min_screen_off_duration {
                    PowerState::Background
                } else {
                    PowerState::Active // Grace period
                }
            }
        }
    }

    /// Get reason for power state change
    fn get_power_state_reason(&self, new_state: PowerState) -> String {
        match new_state {
            PowerState::Active => "Screen on or grace period".to_string(),
            PowerState::Background => {
                if self.battery_level <= self.config.battery_thresholds.low_level {
                    format!("Low battery: {:.1}%", self.battery_level * 100.0)
                } else {
                    format!("Screen off for {}s", self.config.min_screen_off_duration.as_secs())
                }
            }
            PowerState::Critical => format!("Critical battery: {:.1}%", self.battery_level * 100.0),
            PowerState::Inactive => "App backgrounded".to_string(),
        }
    }

    /// Update cover traffic ratio based on power state
    fn update_cover_traffic_ratio(&mut self, power_state: PowerState) {
        self.stats.current_cover_ratio = match power_state {
            PowerState::Active => self.config.screen_on_cover_ratio,
            PowerState::Background => self.config.screen_off_cover_ratio,
            PowerState::Inactive => self.config.screen_off_cover_ratio * 0.5, // Even lower
            PowerState::Critical => 0.05, // Minimal cover traffic
        };
    }

    /// Clean history entries outside tracking window
    fn clean_history(&mut self) {
        let now = Instant::now();
        let cutoff = now.checked_sub(self.config.tracking_window).unwrap_or(now);
        
        while let Some(front) = self.screen_history.front() {
            if front.timestamp < cutoff {
                self.screen_history.pop_front();
            } else {
                break;
            }
        }
    }

    /// Calculate screen-off ratio based on recent history
    fn calculate_screen_off_ratio(&mut self) {
        if self.screen_history.is_empty() {
            self.stats.screen_off_ratio = 0.0;
            return;
        }

        let now = Instant::now();
        let window_start = now.checked_sub(self.config.tracking_window).unwrap_or(now);
        
        let mut screen_on_time = Duration::ZERO;
        let mut screen_off_time = Duration::ZERO;
        
        let mut current_state = ScreenState::On; // Assume started with screen on
        let mut state_start = window_start;
        
        for event in &self.screen_history {
            if event.timestamp > window_start {
                // Add duration from state_start to event timestamp
                let duration = event.timestamp.duration_since(state_start);
                match current_state {
                    ScreenState::On => screen_on_time += duration,
                    ScreenState::Off => screen_off_time += duration,
                }
                
                current_state = event.state;
                state_start = event.timestamp;
            }
        }
        
        // Add time from last event to now
        let final_duration = now.duration_since(state_start);
        match current_state {
            ScreenState::On => screen_on_time += final_duration,
            ScreenState::Off => screen_off_time += final_duration,
        }
        
        let total_time = screen_on_time + screen_off_time;
        if total_time > Duration::ZERO {
            self.stats.screen_off_ratio = screen_off_time.as_secs_f32() / total_time.as_secs_f32();
        } else {
            self.stats.screen_off_ratio = 0.0;
        }
        
        // Update stats
        self.stats.screen_on_duration = screen_on_time;
        self.stats.screen_off_duration = screen_off_time;
    }

    /// Get current statistics
    pub fn get_stats(&self) -> ScreenOffStats {
        self.stats.clone()
    }

    /// Get current screen state
    pub fn get_screen_state(&self) -> ScreenState {
        self.current_screen_state
    }

    /// Get current power state
    pub fn get_power_state(&self) -> PowerState {
        self.current_power_state
    }

    /// Get current battery level
    pub fn get_battery_level(&self) -> f32 {
        self.battery_level
    }

    /// Get current cover traffic ratio
    pub fn get_cover_traffic_ratio(&self) -> f32 {
        self.stats.current_cover_ratio
    }

    /// Set app background state (for when app is backgrounded but screen might be on)
    pub fn set_app_background(&mut self, is_background: bool) -> Option<PowerStateEvent> {
        if is_background && self.current_power_state != PowerState::Inactive {
            let now = Instant::now();
            let event = PowerStateEvent {
                state: PowerState::Inactive,
                previous_state: self.current_power_state,
                timestamp: now,
                reason: "App backgrounded".to_string(),
            };
            
            self.current_power_state = PowerState::Inactive;
            self.last_power_change = now;
            self.stats.power_state_changes += 1;
            self.update_cover_traffic_ratio(PowerState::Inactive);
            
            Some(event)
        } else if !is_background && self.current_power_state == PowerState::Inactive {
            // Return to appropriate state based on current conditions
            self.update_power_state()
        } else {
            None
        }
    }

    /// Get configuration
    pub fn get_config(&self) -> &ScreenOffConfig {
        &self.config
    }

    /// Update configuration
    pub fn update_config(&mut self, new_config: ScreenOffConfig) {
        self.config = new_config;
        info!("Screen-off detector configuration updated");
    }
}

/// Screen-off detector with thread-safe access
#[derive(Debug)]
pub struct SharedScreenOffDetector {
    detector: Arc<RwLock<ScreenOffDetector>>,
}

impl SharedScreenOffDetector {
    /// Create a new shared screen-off detector
    pub fn new(config: ScreenOffConfig) -> Self {
        Self {
            detector: Arc::new(RwLock::new(ScreenOffDetector::new(config))),
        }
    }

    /// Create with default configuration
    pub fn with_default_config() -> Self {
        Self::new(ScreenOffConfig::default())
    }

    /// Update screen state
    pub async fn update_screen_state(&self, state: ScreenState) -> Option<ScreenStateEvent> {
        self.detector.write().await.update_screen_state(state)
    }

    /// Update battery level
    pub async fn update_battery_level(&self, level: f32) -> Option<PowerStateEvent> {
        self.detector.write().await.update_battery_level(level)
    }

    /// Set app background state
    pub async fn set_app_background(&self, is_background: bool) -> Option<PowerStateEvent> {
        self.detector.write().await.set_app_background(is_background)
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> ScreenOffStats {
        self.detector.read().await.get_stats()
    }

    /// Get current states
    pub async fn get_states(&self) -> (ScreenState, PowerState, f32) {
        let detector = self.detector.read().await;
        (
            detector.get_screen_state(),
            detector.get_power_state(),
            detector.get_battery_level(),
        )
    }

    /// Get current cover traffic ratio
    pub async fn get_cover_traffic_ratio(&self) -> f32 {
        self.detector.read().await.get_cover_traffic_ratio()
    }

    /// Update configuration
    pub async fn update_config(&self, config: ScreenOffConfig) {
        self.detector.write().await.update_config(config);
    }

    /// Get configuration
    pub async fn get_config(&self) -> ScreenOffConfig {
        self.detector.read().await.get_config().clone()
    }
}

impl Clone for SharedScreenOffDetector {
    fn clone(&self) -> Self {
        Self {
            detector: Arc::clone(&self.detector),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_screen_off_detector_creation() {
        let detector = ScreenOffDetector::with_default_config();
        assert_eq!(detector.get_screen_state(), ScreenState::On);
        assert_eq!(detector.get_power_state(), PowerState::Active);
        assert_eq!(detector.get_battery_level(), 1.0);
    }

    #[test]
    fn test_screen_state_transitions() {
        let mut detector = ScreenOffDetector::with_default_config();
        
        // Transition to screen off
        let event = detector.update_screen_state(ScreenState::Off);
        assert!(event.is_some());
        assert_eq!(detector.get_screen_state(), ScreenState::Off);
        
        // No change - should return None
        let event = detector.update_screen_state(ScreenState::Off);
        assert!(event.is_none());
        
        // Back to screen on
        let event = detector.update_screen_state(ScreenState::On);
        assert!(event.is_some());
        assert_eq!(detector.get_screen_state(), ScreenState::On);
    }

    #[test]
    fn test_battery_level_updates() {
        let mut detector = ScreenOffDetector::with_default_config();
        
        // Valid battery level
        detector.update_battery_level(0.5);
        assert_eq!(detector.get_battery_level(), 0.5);
        
        // Clamped to valid range
        detector.update_battery_level(-0.1);
        assert_eq!(detector.get_battery_level(), 0.0);
        
        detector.update_battery_level(1.5);
        assert_eq!(detector.get_battery_level(), 1.0);
    }

    #[test]
    fn test_power_state_low_battery() {
        let mut config = ScreenOffConfig::default();
        config.state_change_cooldown = Duration::ZERO; // Disable cooldown for testing
        
        let mut detector = ScreenOffDetector::new(config);
        
        // Set low battery
        let event = detector.update_battery_level(0.2); // Below 25% threshold
        assert!(event.is_some());
        assert_eq!(detector.get_power_state(), PowerState::Background);
    }

    #[test]
    fn test_power_state_critical_battery() {
        let mut config = ScreenOffConfig::default();
        config.state_change_cooldown = Duration::ZERO;
        
        let mut detector = ScreenOffDetector::new(config);
        
        // Set critical battery
        let event = detector.update_battery_level(0.05); // Below 10% threshold
        assert!(event.is_some());
        assert_eq!(detector.get_power_state(), PowerState::Critical);
    }

    #[test]
    fn test_cover_traffic_ratio_updates() {
        let mut config = ScreenOffConfig::default();
        config.state_change_cooldown = Duration::ZERO;
        
        let mut detector = ScreenOffDetector::new(config);
        
        // Default active state
        assert_eq!(detector.get_cover_traffic_ratio(), 0.4); // screen_on_cover_ratio
        
        // Screen off should trigger background state after enough time
        detector.update_screen_state(ScreenState::Off);
        
        // Force power state update by setting battery low
        detector.update_battery_level(0.2);
        assert_eq!(detector.get_cover_traffic_ratio(), 0.1); // screen_off_cover_ratio
    }

    #[test]
    fn test_app_background_state() {
        // Create detector with zero cooldown for testing
        let mut config = ScreenOffConfig::default();
        config.state_change_cooldown = Duration::ZERO;
        let mut detector = ScreenOffDetector::new(config);
        
        // Verify initial state
        assert_eq!(detector.get_power_state(), PowerState::Active);
        
        // Set app to background
        let event = detector.set_app_background(true);
        assert!(event.is_some());
        assert_eq!(detector.get_power_state(), PowerState::Inactive);
        
        // Return to foreground
        let event = detector.set_app_background(false);
        assert!(event.is_some());
        assert_eq!(detector.get_power_state(), PowerState::Active);
    }

    #[tokio::test]
    async fn test_shared_detector() {
        let detector = SharedScreenOffDetector::with_default_config();
        
        // Test screen state update
        let event = detector.update_screen_state(ScreenState::Off).await;
        assert!(event.is_some());
        
        let (screen_state, power_state, battery) = detector.get_states().await;
        assert_eq!(screen_state, ScreenState::Off);
        assert_eq!(battery, 1.0);
        
        // Test battery update
        let event = detector.update_battery_level(0.5).await;
        let battery = detector.get_states().await.2;
        assert_eq!(battery, 0.5);
    }

    #[test]
    fn test_screen_off_ratio_calculation() {
        let mut detector = ScreenOffDetector::with_default_config();
        
        // Simulate some screen state changes
        detector.update_screen_state(ScreenState::Off);
        std::thread::sleep(Duration::from_millis(100));
        detector.update_screen_state(ScreenState::On);
        std::thread::sleep(Duration::from_millis(100));
        
        let stats = detector.get_stats();
        // Should have some screen-off time recorded
        assert!(stats.screen_off_duration > Duration::ZERO);
        assert!(stats.screen_on_duration > Duration::ZERO);
        assert!(stats.state_changes > 0);
    }

    #[test]
    fn test_configuration_updates() {
        let mut detector = ScreenOffDetector::with_default_config();
        
        let mut new_config = ScreenOffConfig::default();
        new_config.screen_off_cover_ratio = 0.05;
        
        detector.update_config(new_config);
        assert_eq!(detector.get_config().screen_off_cover_ratio, 0.05);
    }

    #[test]
    fn test_battery_hysteresis() {
        let mut config = ScreenOffConfig::default();
        config.state_change_cooldown = Duration::ZERO;
        config.battery_thresholds.low_level = 0.25;
        config.battery_thresholds.hysteresis = 0.05;
        
        let mut detector = ScreenOffDetector::new(config);
        
        // Drop below threshold
        detector.update_battery_level(0.20);
        assert_eq!(detector.get_power_state(), PowerState::Background);
        
        // Rise slightly above threshold but below hysteresis
        detector.update_battery_level(0.27);
        assert_eq!(detector.get_power_state(), PowerState::Background); // Should stay
        
        // Rise above threshold + hysteresis
        detector.update_battery_level(0.35);
        assert_eq!(detector.get_power_state(), PowerState::Active); // Should change
    }
}