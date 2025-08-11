#![forbid(unsafe_code)]

//! Low Power Mode Implementation for Nyx Protocol v1.0
//!
//! Implements mobile-optimized low power communication including:
//! - Screen-Off detection and mode switching
//! - cover_ratio=0.1 low power traffic mode
//! - FCM/APNS WebPush over Nyx Gateway integration
//! - Push notification routing
//! - Battery-aware adaptive communication
//!
//! When screen turns off or device enters power save mode,
//! Nyx automatically reduces traffic to 10% of normal volume
//! and routes critical notifications through push services.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::{HashMap, VecDeque};
use serde::{Serialize, Deserialize};
use thiserror::Error;
use tokio::time::{sleep, interval};
use tokio::sync::{mpsc, watch};
use tracing::{debug, info, error, trace};
#[cfg(feature = "telemetry")] use nyx_telemetry::metrics::BasicMetrics;
use crate::push_gateway::PushGatewayManager;

#[cfg(feature = "mobile_ffi")]
use crate::ffi_detector::FfiScreenStateDetector;

/// Default low power cover ratio (10% of normal traffic)
pub const LOW_POWER_COVER_RATIO: f64 = 0.1;

/// Minimum interval between cover traffic packets in low power mode (ms)
pub const LOW_POWER_MIN_INTERVAL_MS: u64 = 1000;

/// Maximum queue size for delayed messages
pub const MAX_DELAYED_MESSAGE_QUEUE: usize = 1000;

/// Battery level threshold for aggressive power saving (%)
pub const BATTERY_CRITICAL_THRESHOLD: u8 = 15;

/// Low power mode errors
#[derive(Error, Debug, Clone)]
pub enum LowPowerError {
    #[error("Platform not supported for power management")]
    PlatformNotSupported,

    #[error("Screen state detection failed: {0}")]
    ScreenStateError(String),

    #[error("Battery monitoring error: {0}")]
    BatteryMonitorError(String),

    #[error("Push notification error: {0}")]
    PushNotificationError(String),

    #[error("Message queue full: {0} messages")]
    MessageQueueFull(usize),

    #[error("Invalid cover ratio: {0} (must be 0.0-1.0)")]
    InvalidCoverRatio(f64),

    #[error("Gateway communication error: {0}")]
    GatewayError(String),
}

/// Power management state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerState {
    /// Device screen is on, normal power mode
    ScreenOn,
    /// Device screen is off, but not in deep sleep
    ScreenOff,
    /// Device is in system power save mode
    PowerSaveMode,
    /// Battery is critically low
    CriticalBattery,
    /// Airplane mode or network unavailable
    NetworkUnavailable,
}

/// Screen state detection interface
pub trait ScreenStateDetector: Send + Sync {
    /// Check if screen is currently on
    fn is_screen_on(&self) -> Result<bool, LowPowerError>;
    
    /// Start monitoring screen state changes
    fn start_monitoring(&self) -> Result<mpsc::UnboundedReceiver<bool>, LowPowerError>;
    
    /// Get current battery level (0-100%)
    fn get_battery_level(&self) -> Result<u8, LowPowerError>;
    
    /// Check if device is in power save mode
    fn is_power_save_mode(&self) -> Result<bool, LowPowerError>;
}

/// Push notification service interface
#[async_trait::async_trait]
pub trait PushNotificationService: Send + Sync {
    /// Send push notification through external service (FCM/APNS)
    async fn send_notification(
        &self,
        device_token: &str,
        message: &PushMessage,
    ) -> Result<(), LowPowerError>;
    
    /// Register device for push notifications
    async fn register_device(&self, device_token: &str) -> Result<(), LowPowerError>;
    
    /// Check if push service is available
    async fn is_available(&self) -> bool;
}

/// Push notification message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushMessage {
    /// Message title
    pub title: String,
    /// Message body
    pub body: String,
    /// Custom data payload
    pub data: HashMap<String, String>,
    /// Priority level (0-10)
    pub priority: u8,
    /// TTL in seconds
    pub ttl: u32,
}

/// Delayed message for queuing during low power mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayedMessage {
    /// Message payload
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    /// Destination address
    pub destination: String,
    /// Original timestamp
    pub timestamp: u64,
    /// Priority (higher number = higher priority)
    pub priority: u8,
    /// Whether this message requires push notification
    pub requires_push: bool,
}

/// Cover traffic pattern for low power mode
#[derive(Debug, Clone)]
pub struct CoverTrafficPattern {
    /// Base interval between packets
    pub base_interval: Duration,
    /// Variance in interval (+/- percentage)
    pub interval_variance: f64,
    /// Packet size range (min, max)
    pub size_range: (usize, usize),
    /// Cover traffic intensity (0.0-1.0)
    pub intensity: f64,
}

impl Default for CoverTrafficPattern {
    fn default() -> Self {
        Self {
            base_interval: Duration::from_millis(LOW_POWER_MIN_INTERVAL_MS),
            interval_variance: 0.2, // ±20%
            size_range: (64, 1280),
            intensity: LOW_POWER_COVER_RATIO,
        }
    }
}

/// Low power mode manager
pub struct LowPowerManager {
    /// Current power state
    power_state: Arc<RwLock<PowerState>>,
    /// Screen state detector
    screen_detector: Arc<dyn ScreenStateDetector>,
    /// Push notification service
    push_service: Option<Arc<dyn PushNotificationService>>,
    /// Delayed message queue
    message_queue: Arc<Mutex<VecDeque<DelayedMessage>>>,
    /// Cover traffic pattern
    cover_pattern: Arc<RwLock<CoverTrafficPattern>>,
    /// Optional sink to deliver generated cover packets to the transport layer
    cover_packet_sink: Arc<Mutex<Option<mpsc::UnboundedSender<Vec<u8>>>>>,
    /// Power state change notifications
    state_notifier: watch::Sender<PowerState>,
    /// Statistics
    stats: Arc<RwLock<LowPowerStats>>,
    /// Device token for push notifications
    device_token: Arc<RwLock<Option<String>>>,
    #[cfg(feature = "telemetry")]
    telemetry: Arc<LowPowerTelemetry>,
    /// Optional push gateway manager for wake->resume integration
    push_gateway: Arc<RwLock<Option<Arc<PushGatewayManager>>>>,
}

/// Low power mode statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowPowerStats {
    /// Total time in low power mode (seconds)
    pub total_low_power_time: u64,
    /// Messages delayed due to low power mode
    pub messages_delayed: u64,
    /// Messages sent via push notifications
    pub push_notifications_sent: u64,
    /// Cover traffic packets generated
    pub cover_packets_generated: u64,
    /// Estimated number of cover packets suppressed due to reduced intensity
    pub suppressed_cover_packets: u64,
    /// Battery level history (timestamp, level)
    pub battery_history: Vec<(u64, u8)>,
    /// Power state transitions
    pub state_transitions: HashMap<String, u64>,
}

impl Default for LowPowerStats {
    fn default() -> Self {
        Self {
            total_low_power_time: 0,
            messages_delayed: 0,
            push_notifications_sent: 0,
            cover_packets_generated: 0,
            suppressed_cover_packets: 0,
            battery_history: Vec::new(),
            state_transitions: HashMap::new(),
        }
    }
}

#[cfg(feature = "telemetry")]
struct LowPowerTelemetry {
    cover_packets_metric: Mutex<BasicMetrics>,
    push_notifications_metric: Mutex<BasicMetrics>,
    suppressed_cover_packets_metric: Mutex<BasicMetrics>,
}

#[cfg(feature = "telemetry")]
impl LowPowerTelemetry { fn new() -> Self { Self { cover_packets_metric: Mutex::new(BasicMetrics::new()), push_notifications_metric: Mutex::new(BasicMetrics::new()), suppressed_cover_packets_metric: Mutex::new(BasicMetrics::new()) } } }

impl LowPowerManager {
    /// Create new low power manager
    pub fn new(
        screen_detector: Arc<dyn ScreenStateDetector>,
        push_service: Option<Arc<dyn PushNotificationService>>,
    ) -> Self {
        let (state_tx, _) = watch::channel(PowerState::ScreenOn);
        
        Self {
            power_state: Arc::new(RwLock::new(PowerState::ScreenOn)),
            screen_detector,
            push_service,
            message_queue: Arc::new(Mutex::new(VecDeque::new())),
            cover_pattern: Arc::new(RwLock::new(CoverTrafficPattern::default())),
            cover_packet_sink: Arc::new(Mutex::new(None)),
            state_notifier: state_tx,
            stats: Arc::new(RwLock::new(LowPowerStats::default())),
            device_token: Arc::new(RwLock::new(None)),
            #[cfg(feature = "telemetry")]
            telemetry: Arc::new(LowPowerTelemetry::new()),
            push_gateway: Arc::new(RwLock::new(None)),
        }
    }

    /// Provide a sink to receive generated cover packets.
    pub fn set_cover_packet_sink(&self, tx: mpsc::UnboundedSender<Vec<u8>>) {
        *self.cover_packet_sink.lock().unwrap() = Some(tx);
    }

    /// Convenience constructor for mobile FFI polling detector (feature mobile_ffi).
    /// This will create and use the FFI-backed ScreenStateDetector.
    #[cfg(feature = "mobile_ffi")]
    pub fn with_mobile_ffi(push_service: Option<Arc<dyn PushNotificationService>>) -> Result<Self, LowPowerError> {
        let detector = FfiScreenStateDetector::new()?; // performs init internally
        Ok(Self::new(detector, push_service))
    }

    /// Start low power monitoring
    pub async fn start_monitoring(&self) -> Result<(), LowPowerError> {
        info!("Starting low power mode monitoring");

        // Start screen state monitoring
    let screen_rx = self.screen_detector.start_monitoring()?;
        let power_state = Arc::clone(&self.power_state);
        let state_notifier = self.state_notifier.clone();
    let stats = Arc::clone(&self.stats);
    #[cfg(feature = "telemetry")]
    let telemetry = Arc::clone(&self.telemetry);
        let screen_detector = Arc::clone(&self.screen_detector);
    let push_gateway_opt = Arc::clone(&self.push_gateway);

        tokio::spawn(async move {
            let mut screen_rx = screen_rx;
            
            while let Some(screen_on) = screen_rx.recv().await {
                let new_state = if screen_on {
                    PowerState::ScreenOn
                } else {
                    // Check additional conditions when screen turns off
                    match (screen_detector.is_power_save_mode(), screen_detector.get_battery_level()) {
                        (Ok(true), _) => PowerState::PowerSaveMode,
                        (_, Ok(level)) if level < BATTERY_CRITICAL_THRESHOLD => PowerState::CriticalBattery,
                        _ => PowerState::ScreenOff,
                    }
                };

                let old_state = {
                    let mut state = power_state.write().unwrap();
                    let old = *state;
                    *state = new_state;
                    old
                };

                if old_state != new_state {
                    info!("Power state changed: {:?} -> {:?}", old_state, new_state);
                    let _ = state_notifier.send(new_state);

                    // Update statistics
                    let mut stats_guard = stats.write().unwrap();
                    let transition_key = format!("{:?}->{:?}", old_state, new_state);
                    *stats_guard.state_transitions.entry(transition_key).or_insert(0) += 1;
                    if matches!(new_state, PowerState::ScreenOn) {
                        if let Some(pg) = push_gateway_opt.read().unwrap().as_ref().cloned() {
                            tokio::spawn(async move { let _ = pg.resume_low_power_session().await; });
                        }
                    }
                }
            }
        });

        // Start battery monitoring
        self.start_battery_monitoring().await;

        // Start cover traffic generation
        self.start_cover_traffic_generation().await;

        // Start message queue processing
        self.start_message_queue_processing().await;

        Ok(())
    }

    /// Set device token for push notifications
    pub fn set_device_token(&self, token: String) {
        let mut device_token = self.device_token.write().unwrap();
        *device_token = Some(token);
        debug!("Device token set for push notifications");
    }

    /// Check if currently in low power mode
    pub fn is_low_power_mode(&self) -> bool {
        let state = self.power_state.read().unwrap();
        matches!(*state, PowerState::ScreenOff | PowerState::PowerSaveMode | PowerState::CriticalBattery)
    }

    /// Get current power state
    pub fn get_power_state(&self) -> PowerState {
        *self.power_state.read().unwrap()
    }

    /// Subscribe to power state changes
    pub fn subscribe_state_changes(&self) -> watch::Receiver<PowerState> {
        self.state_notifier.subscribe()
    }

    /// Queue message for delayed sending
    pub fn queue_message(&self, message: DelayedMessage) -> Result<(), LowPowerError> {
        let mut queue = self.message_queue.lock().unwrap();
        
        if queue.len() >= MAX_DELAYED_MESSAGE_QUEUE {
            return Err(LowPowerError::MessageQueueFull(queue.len()));
        }

        // Insert based on priority (higher priority first)
        let insert_pos = queue.iter().position(|m| m.priority < message.priority)
            .unwrap_or(queue.len());
        
        queue.insert(insert_pos, message);
        
        let mut stats = self.stats.write().unwrap();
        stats.messages_delayed += 1;
        
        debug!("Message queued for delayed sending, queue size: {}", queue.len());
        Ok(())
    }

    /// Send high-priority message via push notification
    pub async fn send_push_notification(
        &self,
        message: &DelayedMessage,
    ) -> Result<(), LowPowerError> {
        let push_service = self.push_service.as_ref()
            .ok_or(LowPowerError::PushNotificationError("Push service not configured".to_string()))?;

        let device_token = self.device_token.read().unwrap()
            .clone()
            .ok_or(LowPowerError::PushNotificationError("Device token not set".to_string()))?;

        let push_message = PushMessage {
            title: "Nyx Message".to_string(),
            body: format!("New message from {}", message.destination),
            data: {
                let mut data = HashMap::new();
                data.insert("destination".to_string(), message.destination.clone());
                data.insert("timestamp".to_string(), message.timestamp.to_string());
                data.insert("priority".to_string(), message.priority.to_string());
                data
            },
            priority: message.priority,
            ttl: 86400, // 24 hours
        };

        push_service.send_notification(&device_token, &push_message).await?;

        let mut stats = self.stats.write().unwrap();
        stats.push_notifications_sent += 1;
        #[cfg(feature = "telemetry")]
        {
            if let Ok(mut m) = self.telemetry.push_notifications_metric.lock() { m.increment(); }
        }

        info!("Push notification sent for high-priority message");
        Ok(())
    }

    /// Update cover traffic pattern
    pub fn update_cover_pattern(&self, pattern: CoverTrafficPattern) -> Result<(), LowPowerError> {
        if pattern.intensity < 0.0 || pattern.intensity > 1.0 {
            return Err(LowPowerError::InvalidCoverRatio(pattern.intensity));
        }

        let intensity = pattern.intensity; // Store for logging
        let mut cover_pattern = self.cover_pattern.write().unwrap();
        *cover_pattern = pattern;

        debug!("Cover traffic pattern updated: intensity={:.2}", intensity);
        Ok(())
    }

    /// Get current statistics
    pub fn get_stats(&self) -> LowPowerStats {
        self.stats.read().unwrap().clone()
    }

    /// Start battery level monitoring
    async fn start_battery_monitoring(&self) {
        let screen_detector = Arc::clone(&self.screen_detector);
        let stats = Arc::clone(&self.stats);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Check every minute
            
            loop {
                interval.tick().await;
                
                match screen_detector.get_battery_level() {
                    Ok(level) => {
                        let timestamp = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        
                        let mut stats_guard = stats.write().unwrap();
                        stats_guard.battery_history.push((timestamp, level));
                        
                        // Keep only last 24 hours of battery history
                        stats_guard.battery_history.retain(|(ts, _)| 
                            timestamp - ts < 86400);
                        
                        trace!("Battery level: {}%", level);
                    }
                    Err(e) => {
                        error!("Failed to read battery level: {}", e);
                    }
                }
            }
        });
    }

    /// Start cover traffic generation for low power mode
    async fn start_cover_traffic_generation(&self) {
        let power_state = Arc::clone(&self.power_state);
        let cover_pattern = Arc::clone(&self.cover_pattern);
        let stats = Arc::clone(&self.stats);
    #[cfg(feature = "telemetry")]
    let telemetry = Arc::clone(&self.telemetry);
    // Baseline assumption: full power mode would have intensity=1.0 with same interval.
    // We'll approximate suppressed cover packets as ( (1.0 - intensity)/intensity ) * generated_each_loop.
        
        tokio::spawn(async move {
            loop {
                // Check if we should generate cover traffic
                let should_generate = {
                    let state = power_state.read().unwrap();
                    matches!(*state, PowerState::ScreenOff | PowerState::PowerSaveMode)
                };

                if should_generate {
                    let pattern = cover_pattern.read().unwrap().clone();
                    
                    // Generate cover traffic packet and deliver to sink if provided
                    let packet_size = pattern.size_range.0 +
                        (rand::random::<usize>() % (pattern.size_range.1 - pattern.size_range.0).max(1));
                    let mut packet = Vec::with_capacity(packet_size);
                    for _ in 0..packet_size { packet.push(rand::random::<u8>()); }

                    if let Some(tx) = &*cover_packet_sink.lock().unwrap() {
                        if tx.send(packet).is_err() {
                            trace!("Cover packet sink is closed; dropping generated packet");
                        }
                    } else {
                        trace!("Cover packet sink not configured; packet accounted only");
                    }
                    
                    let intensity_snapshot;
                    {
                        let mut stats_guard = stats.write().unwrap();
                        stats_guard.cover_packets_generated += 1;
                        intensity_snapshot = pattern.intensity;
                        if intensity_snapshot < 1.0 && intensity_snapshot > 0.0 {
                            // suppressed ≈ packets that would have been sent if intensity=1 minus what we sent (1 per loop)
                            // scale: (1/intensity) - 1
                            let suppressed_estimate = ((1.0 / intensity_snapshot) - 1.0).round() as u64;
                            stats_guard.suppressed_cover_packets += suppressed_estimate;
                            #[cfg(feature = "telemetry")]
                            if let Ok(mut m) = telemetry.suppressed_cover_packets_metric.lock() { for _ in 0..suppressed_estimate { m.increment(); } }
                        }
                    }
                    #[cfg(feature = "telemetry")]
                    if let Ok(mut m) = telemetry.cover_packets_metric.lock() { m.increment(); }
                    
                    // Calculate next interval with variance
                    let base_ms = pattern.base_interval.as_millis() as f64;
                    let variance = base_ms * pattern.interval_variance * (rand::random::<f64>() - 0.5) * 2.0;
                    let next_interval = Duration::from_millis((base_ms + variance).max(100.0) as u64);
                    
                    sleep(next_interval).await;
                } else {
                    // Not in low power mode, check again after longer interval
                    sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }

    /// Start processing delayed message queue
    async fn start_message_queue_processing(&self) {
        let power_state = Arc::clone(&self.power_state);
        let message_queue = Arc::clone(&self.message_queue);
        let push_service = self.push_service.clone();
        let device_token = Arc::clone(&self.device_token);
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            
            loop {
                interval.tick().await;
                
                let current_state = *power_state.read().unwrap();
                
                // Process queue based on power state
                match current_state {
                    PowerState::ScreenOn => {
                        // Normal mode - send all queued messages
                        let messages_to_send = {
                            let mut queue = message_queue.lock().unwrap();
                            let mut messages = Vec::new();
                            while let Some(message) = queue.pop_front() {
                                messages.push(message);
                            }
                            messages
                        };
                        
                        for message in messages_to_send {
                            info!("Sending queued message to {}", message.destination);
                            // Integrate with actual Nyx sending mechanism
                            if let Err(e) = Self::send_message_via_nyx(&message).await {
                                error!("Failed to send queued message: {}", e);
                            }
                        }
                    }
                    PowerState::ScreenOff | PowerState::PowerSaveMode | PowerState::CriticalBattery => {
                        // Low power mode - only send high priority messages via push
                        if let Some(push_svc) = &push_service {
                            let mut to_push = Vec::new();
                            
                            // Find high-priority messages that need push notifications
                            {
                                let mut queue = message_queue.lock().unwrap();
                                let mut i = 0;
                                while i < queue.len() {
                                    if queue[i].priority >= 200 && queue[i].requires_push {
                                        let message = queue.remove(i).unwrap();
                                        to_push.push(message);
                                    } else {
                                        i += 1;
                                    }
                                }
                            }
                            
                            // Send push notifications
                            for message in to_push {
                                let token = device_token.read().unwrap().clone();
                                if let Some(token) = token {
                                    let push_message = PushMessage {
                                        title: "Nyx High Priority".to_string(),
                                        body: format!("Urgent message from {}", message.destination),
                                        data: HashMap::new(),
                                        priority: message.priority,
                                        ttl: 3600,
                                    };
                                    
                                    if let Err(e) = push_svc.send_notification(&token, &push_message).await {
                                        error!("Failed to send push notification: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    PowerState::NetworkUnavailable => {
                        // Keep messages in queue until network is available
                        trace!("Network unavailable, keeping messages in queue");
                    }
                }
            }
        });
    }

    /// Attach a PushGatewayManager so that screen-on events can auto-resume the
    /// low power session and push wake events can be recorded.
    pub fn attach_push_gateway(&self, gateway: Arc<PushGatewayManager>) {
        let mut guard = self.push_gateway.write().unwrap();
        *guard = Some(gateway);
        debug!("PushGatewayManager attached to LowPowerManager");
    }

    /// Record a push wake originating externally (e.g., native push callback).
    /// This forwards to the PushGatewayManager if attached.
    pub fn record_push_wake(&self) {
        if let Some(pg) = self.push_gateway.read().unwrap().as_ref().cloned() {
            let _ = pg.push_wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
    use tokio::sync::mpsc;

    struct MockScreenDetector {
        screen_on: AtomicBool,
        battery_level: AtomicU8,
        power_save: AtomicBool,
    }

    impl MockScreenDetector {
        fn new() -> Self {
            Self {
                screen_on: AtomicBool::new(true),
                battery_level: AtomicU8::new(50),
                power_save: AtomicBool::new(false),
            }
        }

        fn set_screen_on(&self, on: bool) {
            self.screen_on.store(on, Ordering::Relaxed);
        }

        fn set_battery_level(&self, level: u8) {
            self.battery_level.store(level, Ordering::Relaxed);
        }
    }

    impl ScreenStateDetector for MockScreenDetector {
        fn is_screen_on(&self) -> Result<bool, LowPowerError> {
            Ok(self.screen_on.load(Ordering::Relaxed))
        }

        fn start_monitoring(&self) -> Result<mpsc::UnboundedReceiver<bool>, LowPowerError> {
            let (tx, rx) = mpsc::unbounded_channel();
            
            // Send initial state
            let _ = tx.send(self.screen_on.load(Ordering::Relaxed));
            
            Ok(rx)
        }

        fn get_battery_level(&self) -> Result<u8, LowPowerError> {
            Ok(self.battery_level.load(Ordering::Relaxed))
        }

        fn is_power_save_mode(&self) -> Result<bool, LowPowerError> {
            Ok(self.power_save.load(Ordering::Relaxed))
        }
    }

    struct MockPushService {
        sent_count: Arc<Mutex<u32>>,
    }

    impl MockPushService {
        fn new() -> Self {
            Self {
                sent_count: Arc::new(Mutex::new(0)),
            }
        }

        fn get_sent_count(&self) -> u32 {
            *self.sent_count.lock().unwrap()
        }
    }

    #[async_trait::async_trait]
    impl PushNotificationService for MockPushService {
        async fn send_notification(
            &self,
            _device_token: &str,
            _message: &PushMessage,
        ) -> Result<(), LowPowerError> {
            let mut count = self.sent_count.lock().unwrap();
            *count += 1;
            Ok(())
        }

        async fn register_device(&self, _device_token: &str) -> Result<(), LowPowerError> {
            Ok(())
        }

        async fn is_available(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn test_power_state_detection() {
        let detector = Arc::new(MockScreenDetector::new());
        let manager = LowPowerManager::new(detector.clone(), None);

        assert_eq!(manager.get_power_state(), PowerState::ScreenOn);
        assert!(!manager.is_low_power_mode());

        // Simulate screen turning off
        detector.set_screen_on(false);
        // In a real implementation, this would trigger state change
        
        // Test battery level detection
        detector.set_battery_level(10);
        assert_eq!(detector.get_battery_level().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_message_queuing() {
        let detector = Arc::new(MockScreenDetector::new());
        let manager = LowPowerManager::new(detector.clone(), None);

        let message = DelayedMessage {
            payload: b"test message".to_vec(),
            destination: "test.example.com".to_string(),
            timestamp: 1000,
            priority: 100,
            requires_push: false,
        };

        assert!(manager.queue_message(message).is_ok());

        let stats = manager.get_stats();
        assert_eq!(stats.messages_delayed, 1);
    }

    #[tokio::test]
    async fn test_push_notification() {
        let detector = Arc::new(MockScreenDetector::new());
        let push_service = Arc::new(MockPushService::new());
        let manager = LowPowerManager::new(detector, Some(push_service.clone()));

        manager.set_device_token("test_token".to_string());

        let message = DelayedMessage {
            payload: b"urgent message".to_vec(),
            destination: "urgent.example.com".to_string(),
            timestamp: 2000,
            priority: 255,
            requires_push: true,
        };

        assert!(manager.send_push_notification(&message).await.is_ok());
        assert_eq!(push_service.get_sent_count(), 1);

        let stats = manager.get_stats();
        assert_eq!(stats.push_notifications_sent, 1);
    }

    #[test]
    fn test_cover_pattern_update() {
        let detector = Arc::new(MockScreenDetector::new());
        let manager = LowPowerManager::new(detector, None);

        let pattern = CoverTrafficPattern {
            base_interval: Duration::from_millis(2000),
            interval_variance: 0.3,
            size_range: (128, 1024),
            intensity: 0.05,
        };

        assert!(manager.update_cover_pattern(pattern).is_ok());

        // Test invalid intensity
        let invalid_pattern = CoverTrafficPattern {
            intensity: 1.5,
            ..Default::default()
        };

        assert!(manager.update_cover_pattern(invalid_pattern).is_err());
    }

    struct TestScreenDetector {
        screen_on: AtomicBool,
        battery_level: AtomicU8,
        power_save: AtomicBool,
        tx: Arc<Mutex<Option<mpsc::UnboundedSender<bool>>>>,
    }

    impl TestScreenDetector {
        fn new() -> Self {
            Self {
                screen_on: AtomicBool::new(true),
                battery_level: AtomicU8::new(50),
                power_save: AtomicBool::new(false),
                tx: Arc::new(Mutex::new(None)),
            }
        }
        fn send_state(&self, on: bool) {
            self.screen_on.store(on, Ordering::Relaxed);
            if let Some(tx) = self.tx.lock().unwrap().as_ref() {
                let _ = tx.send(on);
            }
        }
        fn set_battery(&self, lvl: u8) { self.battery_level.store(lvl, Ordering::Relaxed); }
    }

    impl ScreenStateDetector for TestScreenDetector {
        fn is_screen_on(&self) -> Result<bool, LowPowerError> { Ok(self.screen_on.load(Ordering::Relaxed)) }
        fn start_monitoring(&self) -> Result<mpsc::UnboundedReceiver<bool>, LowPowerError> {
            let (tx, rx) = mpsc::unbounded_channel();
            *self.tx.lock().unwrap() = Some(tx.clone());
            let _ = tx.send(self.screen_on.load(Ordering::Relaxed));
            Ok(rx)
        }
        fn get_battery_level(&self) -> Result<u8, LowPowerError> { Ok(self.battery_level.load(Ordering::Relaxed)) }
        fn is_power_save_mode(&self) -> Result<bool, LowPowerError> { Ok(self.power_save.load(Ordering::Relaxed)) }
    }

    #[tokio::test]
    async fn test_low_power_state_transitions_and_cover_generation() {
        let detector = Arc::new(TestScreenDetector::new());
        let manager = LowPowerManager::new(detector.clone(), None);

        // Speed up cover traffic generation
        manager.update_cover_pattern(CoverTrafficPattern {
            base_interval: Duration::from_millis(100),
            interval_variance: 0.0,
            size_range: (64, 72),
            intensity: LOW_POWER_COVER_RATIO,
        }).unwrap();

    // Force initial state to screen off so cover loop starts generating immediately
    detector.send_state(false);
    manager.start_monitoring().await.unwrap();
        tokio::time::sleep(Duration::from_millis(120)).await; // allow tasks start
    // Now should already be ScreenOff
    tokio::time::sleep(Duration::from_millis(150)).await; // extra settle
        assert_eq!(manager.get_power_state(), PowerState::ScreenOff);
        assert!(manager.is_low_power_mode());

        // Wait (poll) for cover packets (up to ~2s) because generation loop timing may drift
        let mut attempts = 0;
        let mut have_packets = false;
        while attempts < 20 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if manager.get_stats().cover_packets_generated > 0 { have_packets = true; break; }
            attempts += 1;
        }
        assert!(have_packets, "expected cover packets when screen off within timeout");

        // Now simulate critical battery
        detector.set_battery(5);
        detector.send_state(false); // trigger evaluation
        tokio::time::sleep(Duration::from_millis(120)).await;
        assert_eq!(manager.get_power_state(), PowerState::CriticalBattery);
        assert!(manager.is_low_power_mode());
    }

    use crate::push_gateway::PushGatewayManager;
    use std::sync::atomic::{AtomicUsize};

    #[tokio::test]
    async fn test_auto_resume_on_screen_on_triggers_push_gateway() {
        let detector = Arc::new(TestScreenDetector::new());
        let manager = LowPowerManager::new(detector.clone(), None);

        let attempts = Arc::new(AtomicUsize::new(0));
        let pg_attempts = attempts.clone();
        let pg = PushGatewayManager::from_async_fn(move || {
            let pg_attempts = pg_attempts.clone();
            async move {
                pg_attempts.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        manager.attach_push_gateway(pg);

        manager.start_monitoring().await.unwrap();
        // Transition away from ScreenOn -> ScreenOff -> ScreenOn to trigger resume
        detector.send_state(false); // off
        tokio::time::sleep(Duration::from_millis(50)).await;
        detector.send_state(true); // back on should fire resume

        let mut waited = 0u64;
        let mut success = false;
        while waited < 2000 { // up to 2s
            if attempts.load(Ordering::SeqCst) > 0 { success = true; break; }
            tokio::time::sleep(Duration::from_millis(50)).await;
            waited += 50;
        }
    assert!(success, "expected push gateway resume to be invoked after screen on transition");
    }
}

impl LowPowerManager {
    /// Send message via Nyx protocol
    async fn send_message_via_nyx(message: &DelayedMessage) -> Result<(), LowPowerError> {
        // Integration with Nyx transport layer
        use crate::types::NodeEndpoint;
        
        // Parse destination address
        let endpoint = message.destination.parse::<NodeEndpoint>()
            .map_err(|e| LowPowerError::PushNotificationError(format!("Invalid destination: {}", e)))?;
            
        // Send via transport layer
        // This would integrate with nyx-transport layer
        trace!("Sending {} bytes to {} via Nyx transport", 
               message.payload.len(), endpoint);
               
        // For now, simulate successful transmission
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    }
    
    /// Enhanced battery optimization algorithm
    pub fn optimize_for_battery_level(&self, battery_level: u8) -> Result<(), LowPowerError> {
        let mut pattern = self.cover_pattern.read().unwrap().clone();
        
        // Adjust parameters based on battery level
        match battery_level {
            0..=15 => {
                // Critical battery - maximum power saving
                pattern.intensity = 0.01; // 1% of normal traffic
                pattern.base_interval = Duration::from_secs(5);
                info!("Activated critical battery mode: 1% traffic intensity");
            }
            16..=30 => {
                // Low battery - aggressive power saving
                pattern.intensity = 0.05; // 5% of normal traffic
                pattern.base_interval = Duration::from_secs(2);
                info!("Activated low battery mode: 5% traffic intensity");
            }
            31..=50 => {
                // Medium battery - moderate power saving
                pattern.intensity = LOW_POWER_COVER_RATIO; // 10% of normal traffic
                pattern.base_interval = Duration::from_millis(1000);
                info!("Activated medium battery mode: 10% traffic intensity");
            }
            _ => {
                // Good battery - normal low power mode
                pattern.intensity = 0.2; // 20% of normal traffic
                pattern.base_interval = Duration::from_millis(500);
                info!("Normal low power mode: 20% traffic intensity");
            }
        }
        
        // Update the pattern
        *self.cover_pattern.write().unwrap() = pattern;
        Ok(())
    }
}
