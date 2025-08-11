#![forbid(unsafe_code)]

//! Plugin Handshake Mechanism for Nyx Protocol v1.0
//!
//! This module implements the plugin handshake process that occurs after the
//! initial Nyx connection establishment. It handles:
//! - Plugin capability negotiation via SETTINGS frames
//! - Required plugin validation and error handling
//! - Plugin initialization and IPC channel setup
//! - Security permission validation

use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tracing::{debug, warn, error, trace, info};
use tokio::time::timeout;
use serde::{Serialize, Deserialize};

use crate::plugin_settings::{
    PluginSettingsManager, PluginRequirement, PluginCapability, PluginSettingsError
};

#[cfg(feature = "telemetry")]
use nyx_telemetry::{register_counter, increment_counter};

/// プラグイン初期化抽象化 Trait
pub trait PluginInitializer: Send + Sync {
    fn name(&self) -> &str;
    fn load(&self, plugin_id: u32) -> Result<(), String>;
    fn establish_ipc(&self, plugin_id: u32) -> Result<(), String> { let _ = plugin_id; Ok(()) }
}

/// デフォルトのインプロセス初期化 (スタブ実装)
pub struct InProcessPluginInitializer;
impl PluginInitializer for InProcessPluginInitializer {
    fn name(&self) -> &str { "in_process_stub" }
    fn load(&self, _plugin_id: u32) -> Result<(), String> { Ok(()) }
}

/// Maximum time allowed for plugin handshake completion
pub const PLUGIN_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of handshake retries on non-fatal errors
pub const MAX_HANDSHAKE_RETRIES: u8 = 3;

/// Plugin handshake state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    /// Initial state - no handshake initiated
    Initial,
    /// Waiting for peer's SETTINGS frame with plugin requirements
    WaitingForPeerSettings,
    /// Sending our plugin capabilities to peer
    SendingCapabilities,
    /// Validating peer requirements against our capabilities
    ValidatingRequirements,
    /// Initializing required plugins
    InitializingPlugins,
    /// Handshake completed successfully
    Completed,
    /// Handshake failed due to incompatible requirements
    Failed,
    /// Handshake aborted due to timeout or critical error
    Aborted,
}

/// Plugin handshake result outcome
#[derive(Debug, Clone)]
pub enum HandshakeResult {
    /// Handshake completed successfully with active plugins
    Success {
        /// Set of plugin IDs that were successfully initialized
        active_plugins: HashSet<u32>,
        /// Duration of handshake process
        handshake_duration: Duration,
    },
    /// Handshake failed due to incompatible plugin requirements
    IncompatibleRequirements {
        /// Plugin ID that caused the failure
        conflicting_plugin_id: u32,
        /// Human-readable reason for failure
        reason: String,
    },
    /// Handshake failed due to timeout
    Timeout {
        /// How long the handshake was attempted
        attempted_duration: Duration,
    },
    /// Handshake aborted due to protocol error
    ProtocolError {
        /// Specific error that caused abortion
        error: String,
    },
}

/// Plugin handshake errors
#[derive(Error, Debug)]
pub enum PluginHandshakeError {
    #[error("Handshake timeout after {duration:?}")]
    Timeout { duration: Duration },

    #[error("Plugin settings error: {0}")]
    SettingsError(#[from] PluginSettingsError),

    #[error("Required plugin {plugin_id} not supported by peer")]
    UnsupportedRequiredPlugin { plugin_id: u32 },

    #[error("Plugin {plugin_id} initialization failed: {reason}")]
    PluginInitializationFailed { plugin_id: u32, reason: String },

    #[error("Invalid handshake state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: HandshakeState, to: HandshakeState },

    #[error("Handshake already in progress")]
    HandshakeInProgress,

    #[error("Protocol error during handshake: {0}")]
    ProtocolError(String),

    #[error("Security validation failed for plugin {plugin_id}: {reason}")]
    SecurityValidationFailed { plugin_id: u32, reason: String },
}

/// Plugin handshake coordinator handles the entire handshake process
pub struct PluginHandshakeCoordinator {
    /// Local plugin settings manager
    local_settings: PluginSettingsManager,
    /// Current handshake state
    state: HandshakeState,
    /// Timestamp when handshake was initiated
    handshake_start_time: Option<SystemTime>,
    /// Peer's plugin requirements received via SETTINGS
    peer_requirements: Option<Vec<PluginRequirement>>,
    /// Plugins that have been successfully initialized
    active_plugins: HashSet<u32>,
    /// Number of retry attempts made
    retry_count: u8,
    /// Whether this endpoint initiated the handshake
    is_initiator: bool,
    /// 初期化戦略
    initializer: std::sync::Arc<dyn PluginInitializer>,
}

impl PluginHandshakeCoordinator {
    /// Create a new plugin handshake coordinator
    ///
    /// # Arguments
    /// * `local_settings` - Local plugin settings and requirements
    /// * `is_initiator` - Whether this endpoint initiates the handshake
    pub fn new(local_settings: PluginSettingsManager, is_initiator: bool) -> Self {
        Self {
            local_settings,
            state: HandshakeState::Initial,
            handshake_start_time: None,
            peer_requirements: None,
            active_plugins: HashSet::new(),
            retry_count: 0,
            is_initiator,
            initializer: std::sync::Arc::new(InProcessPluginInitializer),
        }
    }

    /// カスタム初期化戦略を差し替え
    pub fn with_initializer(mut self, init: std::sync::Arc<dyn PluginInitializer>) -> Self {
        self.initializer = init; self
    }

    /// Initiate plugin handshake process
    ///
    /// This method starts the handshake by transitioning to the appropriate
    /// initial state based on whether this endpoint is the initiator.
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - SETTINGS frame data to send to peer (if initiator)
    /// * `Err(PluginHandshakeError)` - Handshake initiation failed
    pub async fn initiate_handshake(&mut self) -> Result<Option<Vec<u8>>, PluginHandshakeError> {
        if self.state != HandshakeState::Initial {
            return Err(PluginHandshakeError::HandshakeInProgress);
        }

        self.handshake_start_time = Some(SystemTime::now());
        
        if self.is_initiator {
            // Initiator sends plugin requirements first
            self.transition_state(HandshakeState::SendingCapabilities)?;
            
            let settings_data = self.local_settings.generate_settings_frame_data()
                .map_err(PluginHandshakeError::SettingsError)?;
            
            debug!("Initiated plugin handshake as initiator, sending {} bytes of settings", settings_data.len());
            self.transition_state(HandshakeState::WaitingForPeerSettings)?;
            
            Ok(Some(settings_data))
        } else {
            // Responder waits for peer's requirements first
            self.transition_state(HandshakeState::WaitingForPeerSettings)?;
            debug!("Initiated plugin handshake as responder, waiting for peer settings");
            Ok(None)
        }
    }

    /// Process peer's SETTINGS frame with plugin requirements
    ///
    /// # Arguments
    /// * `peer_settings_data` - Binary data from peer's SETTINGS frame
    ///
    /// # Returns
    /// * `Ok(Option<Vec<u8>>)` - Optional response SETTINGS frame data
    /// * `Err(PluginHandshakeError)` - Processing failed
    pub async fn process_peer_settings(&mut self, peer_settings_data: &[u8]) -> Result<Option<Vec<u8>>, PluginHandshakeError> {
        if self.state != HandshakeState::WaitingForPeerSettings {
            return Err(PluginHandshakeError::InvalidStateTransition {
                from: self.state,
                to: HandshakeState::ValidatingRequirements,
            });
        }

        // Parse peer's plugin requirements
        let peer_requirements = self.local_settings.parse_peer_settings_data(peer_settings_data)
            .map_err(PluginHandshakeError::SettingsError)?;
        
        debug!("Received {} plugin requirements from peer", peer_requirements.len());
        
        // Validate that we can satisfy peer's requirements
        self.transition_state(HandshakeState::ValidatingRequirements)?;
        
        if let Err(e) = self.local_settings.validate_peer_requirements(&peer_requirements) {
            warn!("Cannot satisfy peer plugin requirements: {}", e);
            self.transition_state(HandshakeState::Failed)?;
            return Err(PluginHandshakeError::SettingsError(e));
        }

        self.peer_requirements = Some(peer_requirements);
        
        // If we're the responder, send our requirements back
        let response_data = if !self.is_initiator {
            self.transition_state(HandshakeState::SendingCapabilities)?;
            let settings_data = self.local_settings.generate_settings_frame_data()
                .map_err(PluginHandshakeError::SettingsError)?;
            debug!("Sending {} bytes of settings as responder", settings_data.len());
            Some(settings_data)
        } else {
            None
        };

        // Proceed to plugin initialization
        self.transition_state(HandshakeState::InitializingPlugins)?;
        
        Ok(response_data)
    }

    /// Complete plugin initialization phase
    ///
    /// This method initializes all required plugins and establishes IPC channels.
    /// It should be called after peer requirements have been validated.
    ///
    /// # Returns
    /// * `Ok(HandshakeResult)` - Handshake completion result
    /// * `Err(PluginHandshakeError)` - Initialization failed
    pub async fn complete_plugin_initialization(&mut self) -> Result<HandshakeResult, PluginHandshakeError> {
        if self.state != HandshakeState::InitializingPlugins {
            return Err(PluginHandshakeError::InvalidStateTransition {
                from: self.state,
                to: HandshakeState::InitializingPlugins,
            });
        }

        // Apply handshake timeout
        let initialization_result = timeout(
            PLUGIN_HANDSHAKE_TIMEOUT,
            self.initialize_plugins_internal()
        ).await;

        match initialization_result {
            Ok(Ok(active_plugins)) => {
                self.active_plugins = active_plugins.clone();
                self.transition_state(HandshakeState::Completed)?;
                
                let handshake_duration = self.handshake_start_time
                    .map(|start| start.elapsed().unwrap_or(Duration::ZERO))
                    .unwrap_or(Duration::ZERO);
                
                info!("Plugin handshake completed successfully: {} active plugins, duration: {:?}", 
                      active_plugins.len(), handshake_duration);
                #[cfg(all(feature="telemetry", feature="prometheus"))]
                { nyx_telemetry::observe_plugin_init_duration(handshake_duration.as_secs_f64()); }
                
                Ok(HandshakeResult::Success {
                    active_plugins,
                    handshake_duration,
                })
            }
            Ok(Err(e)) => {
                self.transition_state(HandshakeState::Failed)?;
                error!("Plugin initialization failed: {}", e);
                
                match e {
                    PluginHandshakeError::UnsupportedRequiredPlugin { plugin_id } => {
                        Ok(HandshakeResult::IncompatibleRequirements {
                            conflicting_plugin_id: plugin_id,
                            reason: format!("Required plugin {} not supported", plugin_id),
                        })
                    }
                    PluginHandshakeError::PluginInitializationFailed { plugin_id, reason } => {
                        Ok(HandshakeResult::IncompatibleRequirements {
                            conflicting_plugin_id: plugin_id,
                            reason,
                        })
                    }
                    other => Ok(HandshakeResult::ProtocolError {
                        error: other.to_string(),
                    })
                }
            }
            Err(_) => {
                self.transition_state(HandshakeState::Aborted)?;
                let attempted_duration = self.handshake_start_time
                    .map(|start| start.elapsed().unwrap_or(PLUGIN_HANDSHAKE_TIMEOUT))
                    .unwrap_or(PLUGIN_HANDSHAKE_TIMEOUT);
                
                warn!("Plugin handshake timed out after {:?}", attempted_duration);
                
                Ok(HandshakeResult::Timeout {
                    attempted_duration,
                })
            }
        }
    }

    /// Internal plugin initialization logic
    async fn initialize_plugins_internal(&self) -> Result<HashSet<u32>, PluginHandshakeError> {
        let mut active_plugins = HashSet::new();

        // Get list of plugins that need to be initialized
        let mut plugins_to_initialize = HashSet::new();
        
        // Add our required plugins
        for plugin_id in self.local_settings.get_required_plugins() {
            plugins_to_initialize.insert(*plugin_id);
        }
        
        // Add peer's required plugins that we support
        if let Some(ref peer_requirements) = self.peer_requirements {
            for requirement in peer_requirements {
                if requirement.capability == PluginCapability::Required {
                    // Verify we support this plugin (should have been validated earlier)
                    if self.local_settings.get_required_plugins().contains(&requirement.plugin_id) ||
                       self.local_settings.get_optional_plugins().contains(&requirement.plugin_id) {
                        plugins_to_initialize.insert(requirement.plugin_id);
                    } else {
                        return Err(PluginHandshakeError::UnsupportedRequiredPlugin {
                            plugin_id: requirement.plugin_id,
                        });
                    }
                }
            }
        }

        // Initialize each required plugin
        for plugin_id in plugins_to_initialize {
            trace!("Initializing plugin: {}", plugin_id);
            // Per-plugin start time for duration telemetry
            #[cfg(feature = "telemetry")] let per_plugin_start = SystemTime::now();
            
            // Perform security validation
            if let Err(reason) = self.validate_plugin_security(plugin_id).await {
                #[cfg(feature = "telemetry")] { nyx_telemetry::inc_plugin_security_fail(); }
                return Err(PluginHandshakeError::SecurityValidationFailed {
                    plugin_id,
                    reason,
                });
            }

            // Initialize plugin (this would integrate with the actual plugin system)
            // For now, we'll simulate successful initialization
            match self.initialize_single_plugin(plugin_id).await {
                Ok(()) => {
                    active_plugins.insert(plugin_id);
                    debug!("Successfully initialized plugin: {}", plugin_id);
                    #[cfg(feature = "telemetry")] { nyx_telemetry::inc_plugin_init_success(); }
                    // Record per-plugin initialization (incl. security validation) duration
                    #[cfg(feature = "telemetry")] {
                        if let Ok(elapsed) = per_plugin_start.elapsed() { nyx_telemetry::observe_plugin_init_duration(elapsed.as_secs_f64()); }
                    }
                }
                Err(reason) => {
                    #[cfg(feature = "telemetry")] { nyx_telemetry::inc_plugin_init_failure(); }
                    return Err(PluginHandshakeError::PluginInitializationFailed {
                        plugin_id,
                        reason,
                    });
                }
            }
        }

        info!("Plugin initialization completed: {} plugins active", active_plugins.len());
        Ok(active_plugins)
    }

    /// Validate security permissions for a plugin
    async fn validate_plugin_security(&self, plugin_id: u32) -> Result<(), String> {
        use once_cell::sync::Lazy;
        use ed25519_dalek::{Verifier, Signature, VerifyingKey};
        #[derive(Clone)]
        struct RegistryEntry { min_version:(u16,u16), max_version:(u16,u16), pubkey: VerifyingKey, signature: Signature, caps:&'static [&'static str] }

        // Hard-coded demo keys (base64): in production load from signed manifest file.
        static REGISTRY: Lazy<HashMap<u32, RegistryEntry>> = Lazy::new(|| {
            use base64::{engine::general_purpose::STANDARD, Engine};
            let mut m = HashMap::new();
            // For reproducibility we embed a deterministic keypair generated offline; signature is over "plugin:<id>:v1".
            // Public keys (base64) & signatures (base64) are placeholders for concept validation.
            let entries: Vec<(u32,&str,&str,(u16,u16),(u16,u16),&[&str])> = vec![
                (1001, "WqHsyQ1+Jgdo8W7oVdZk90un0nLBKBPXn1HULICwhf8=", "mJ0K63eFUsVTNff7kwh28ykVfoCENKz7LxyzKDn5XMgLwHxZ34rnOG0r8QwMCKaRZ3eLaxhUJW6Ka7O5Kb/6BA==", (1,0),(1,5), &["metrics","basic"]),
                (2002, "WqHsyQ1+Jgdo8W7oVdZk90un0nLBKBPXn1HULICwhf8=", "mJ0K63eFUsVTNff7kwh28ykVfoCENKz7LxyzKDn5XMgLwHxZ34rnOG0r8QwMCKaRZ3eLaxhUJW6Ka7O5Kb/6BA==", (0,9),(2,0), &["advanced"]),
            ];
            for (pid, pk_b64, sig_b64, min_v, max_v, caps) in entries {
                let pk_bytes = STANDARD.decode(pk_b64).expect("pubkey b64");
                let sig_bytes = STANDARD.decode(sig_b64).expect("sig b64");
                let pubkey = VerifyingKey::from_bytes(&pk_bytes.try_into().expect("32" )).expect("vk");
                let signature = Signature::from_bytes(&sig_bytes.try_into().expect("64"));
                m.insert(pid, RegistryEntry { min_version:min_v, max_version:max_v, pubkey: pubkey, signature, caps });
            }
            m
        });

        if plugin_id == 0 { return Err("Plugin ID 0 is reserved".into()); }
        if plugin_id >= 0xFFFF0000 { return Err("Plugin ID is in reserved system range".into()); }
        let entry = REGISTRY.get(&plugin_id).ok_or_else(|| "Plugin not found in registry".to_string())?;

        if let Some(requested) = self.local_settings.get_version_requirement(plugin_id) {
            if requested < entry.min_version || requested > entry.max_version {
                return Err(format!("Version {:?} outside allowed range {:?}-{:?}", requested, entry.min_version, entry.max_version));
            }
        }

        // Verify signature over canonical context string.
        let message = format!("plugin:{}:v1", plugin_id);
        if entry.pubkey.verify(message.as_bytes(), &entry.signature).is_err() {
            return Err("Signature verification failed".into());
        }

        if let Some(req_caps) = self.local_settings.get_required_capabilities(plugin_id) {
            for cap in req_caps { if !entry.caps.contains(&cap.as_str()) { return Err(format!("Capability '{}' missing", cap)); } }
        }
        trace!("Security validation passed (ed25519) for plugin: {}", plugin_id);
    #[cfg(feature = "telemetry")] { nyx_telemetry::inc_plugin_security_pass(); }
        Ok(())
    }

    /// Initialize a single plugin
    async fn initialize_single_plugin(&self, plugin_id: u32) -> Result<(), String> {
    // シミュレーション遅延
    tokio::time::sleep(Duration::from_millis(10)).await;
    if plugin_id == 0xDEADBEEF { return Err("Simulated initialization failure".into()); }
    // 実際のロード/IPC は initializer へ委譲
    self.initializer.load(plugin_id)?;
    self.initializer.establish_ipc(plugin_id)?;
    trace!("Plugin {} initialized via {}", plugin_id, self.initializer.name());
    Ok(())
    }

    /// Transition to a new handshake state with validation
    fn transition_state(&mut self, new_state: HandshakeState) -> Result<(), PluginHandshakeError> {
        let valid_transition = match (self.state, new_state) {
            (HandshakeState::Initial, HandshakeState::SendingCapabilities) => true,
            (HandshakeState::Initial, HandshakeState::WaitingForPeerSettings) => true,
            (HandshakeState::SendingCapabilities, HandshakeState::WaitingForPeerSettings) => true,
            (HandshakeState::WaitingForPeerSettings, HandshakeState::ValidatingRequirements) => true,
            (HandshakeState::ValidatingRequirements, HandshakeState::SendingCapabilities) => true,
            (HandshakeState::ValidatingRequirements, HandshakeState::InitializingPlugins) => true,
            (HandshakeState::SendingCapabilities, HandshakeState::InitializingPlugins) => true,
            (HandshakeState::InitializingPlugins, HandshakeState::Completed) => true,
            (_, HandshakeState::Failed) => true,
            (_, HandshakeState::Aborted) => true,
            _ => false,
        };

        if !valid_transition {
            return Err(PluginHandshakeError::InvalidStateTransition {
                from: self.state,
                to: new_state,
            });
        }

        trace!("Plugin handshake state transition: {:?} -> {:?}", self.state, new_state);
        self.state = new_state;
        Ok(())
    }

    /// Get current handshake state
    pub fn current_state(&self) -> HandshakeState {
        self.state
    }

    /// Get set of active plugins after successful handshake
    pub fn active_plugins(&self) -> &HashSet<u32> {
        &self.active_plugins
    }

    /// Check if handshake is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.state, HandshakeState::Completed)
    }

    /// Check if handshake has failed
    pub fn has_failed(&self) -> bool {
        matches!(self.state, HandshakeState::Failed | HandshakeState::Aborted)
    }

    /// Get handshake duration if started
    pub fn handshake_duration(&self) -> Option<Duration> {
        self.handshake_start_time
            .and_then(|start| start.elapsed().ok())
    }

    /// Reset handshake state for retry
    pub fn reset_for_retry(&mut self) -> Result<(), PluginHandshakeError> {
        if self.retry_count >= MAX_HANDSHAKE_RETRIES {
            return Err(PluginHandshakeError::ProtocolError(
                format!("Maximum handshake retries ({}) exceeded", MAX_HANDSHAKE_RETRIES)
            ));
        }

        self.retry_count += 1;
        self.state = HandshakeState::Initial;
        self.handshake_start_time = None;
        self.peer_requirements = None;
        self.active_plugins.clear();

        debug!("Reset plugin handshake for retry attempt {}", self.retry_count);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_coordinator(is_initiator: bool) -> PluginHandshakeCoordinator {
        let mut settings = PluginSettingsManager::new();
        settings.add_required_plugin(1001, (1, 0), vec![]).expect("Add test plugin");
        PluginHandshakeCoordinator::new(settings, is_initiator)
    }

    #[tokio::test]
    async fn test_initiator_handshake_success() {
        let mut coordinator = create_test_coordinator(true);
        
        // Start handshake as initiator
        let settings_data = coordinator.initiate_handshake().await.expect("Initiate handshake");
        assert!(settings_data.is_some());
        assert_eq!(coordinator.current_state(), HandshakeState::WaitingForPeerSettings);
        
        // Process peer response (empty for test)
        let peer_settings = vec![0x00, 0x00]; // No requirements
        let response = coordinator.process_peer_settings(&peer_settings).await.expect("Process peer settings");
        assert!(response.is_none()); // Initiator doesn't send second response
        assert_eq!(coordinator.current_state(), HandshakeState::InitializingPlugins);
        
        // Complete initialization
        let result = coordinator.complete_plugin_initialization().await.expect("Complete initialization");
        assert!(matches!(result, HandshakeResult::Success { .. }));
        assert!(coordinator.is_complete());
    }

    #[tokio::test]
    async fn test_responder_handshake_success() {
        let mut coordinator = create_test_coordinator(false);
        
        // Start handshake as responder
        let settings_data = coordinator.initiate_handshake().await.expect("Initiate handshake");
        assert!(settings_data.is_none());
        assert_eq!(coordinator.current_state(), HandshakeState::WaitingForPeerSettings);
        
        // Process peer settings
        let peer_settings = vec![0x00, 0x00]; // No requirements
        let response = coordinator.process_peer_settings(&peer_settings).await.expect("Process peer settings");
        assert!(response.is_some()); // Responder sends its requirements
        assert_eq!(coordinator.current_state(), HandshakeState::InitializingPlugins);
        
        // Complete initialization
        let result = coordinator.complete_plugin_initialization().await.expect("Complete initialization");
        assert!(matches!(result, HandshakeResult::Success { .. }));
        assert!(coordinator.is_complete());
    }

    #[tokio::test]
    async fn test_handshake_state_transitions() {
        let mut coordinator = create_test_coordinator(true);
        
        // Test invalid state transition
        let result = coordinator.transition_state(HandshakeState::Completed);
        assert!(matches!(result, Err(PluginHandshakeError::InvalidStateTransition { .. })));
        
        // Test valid state transitions
        assert!(coordinator.transition_state(HandshakeState::SendingCapabilities).is_ok());
        assert!(coordinator.transition_state(HandshakeState::WaitingForPeerSettings).is_ok());
        assert!(coordinator.transition_state(HandshakeState::ValidatingRequirements).is_ok());
        assert!(coordinator.transition_state(HandshakeState::InitializingPlugins).is_ok());
        assert!(coordinator.transition_state(HandshakeState::Completed).is_ok());
    }

    #[tokio::test]
    async fn test_handshake_retry_limit() {
        let mut coordinator = create_test_coordinator(true);
        
        // Exhaust retry attempts
        for _ in 0..MAX_HANDSHAKE_RETRIES {
            coordinator.reset_for_retry().expect("Reset for retry");
        }
        
        // Next retry should fail
        let result = coordinator.reset_for_retry();
        assert!(matches!(result, Err(PluginHandshakeError::ProtocolError(_))));
    }

    #[tokio::test]
    async fn test_security_validation() {
        let coordinator = create_test_coordinator(true);
        // Test valid plugin ID (one that exists in the embedded registry: 1001)
        let result = coordinator.validate_plugin_security(1001).await;
        assert!(result.is_ok(), "expected registry plugin 1001 to validate");
        
        // Test reserved plugin ID
        let result = coordinator.validate_plugin_security(0xFFFF0001).await;
        assert!(result.is_err());
        
        // Test zero plugin ID
        let result = coordinator.validate_plugin_security(0).await;
        assert!(result.is_err());

        // Test unknown (non‑registered) plugin ID
        let result = coordinator.validate_plugin_security(424242).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unsupported_required_plugin_handshake_failure() {
        // Local only knows 1001
        let mut coordinator = create_test_coordinator(true);

        // Start as initiator
        let _settings = coordinator.initiate_handshake().await.expect("initiate");
        assert_eq!(coordinator.current_state(), HandshakeState::WaitingForPeerSettings);

        // Construct peer settings requiring unsupported plugin 424242
        let mut remote = PluginSettingsManager::new();
        remote.add_required_plugin(424242, (1,0), vec![]).expect("add remote required");
        let peer_frame = remote.generate_settings_frame_data().expect("gen frame");

        let err = coordinator.process_peer_settings(&peer_frame).await.expect_err("should fail");
    assert!(matches!(err, PluginHandshakeError::SettingsError(_)), "expected SettingsError for unsupported required plugin");
        assert!(coordinator.has_failed(), "handshake should be marked failed");
    }

    #[tokio::test]
    async fn test_plugin_initialization_failure_maps_to_incompatible_requirements() {
        // Create coordinator with one good plugin (1001) and one that will simulate failure (0xDEADBEEF)
        let mut settings = PluginSettingsManager::new();
        settings.add_required_plugin(1001, (1,0), vec![]).expect("add good plugin");
        settings.add_required_plugin(0xDEADBEEF, (1,0), vec![]).expect("add failing plugin");
        let mut coordinator = PluginHandshakeCoordinator::new(settings, true);

        // Initiate handshake
        let _ = coordinator.initiate_handshake().await.expect("init");

        // Peer returns empty requirements (count=0)
        let peer_settings = vec![0x00, 0x00];
        coordinator.process_peer_settings(&peer_settings).await.expect("process peer");

        // Complete initialization -> expect IncompatibleRequirements with failing plugin id
        let result = coordinator.complete_plugin_initialization().await.expect("complete");
        match result {
            HandshakeResult::IncompatibleRequirements { conflicting_plugin_id, .. } => {
                assert_eq!(conflicting_plugin_id, 0xDEADBEEF);
            }
            other => panic!("expected IncompatibleRequirements, got {:?}", other)
        }
    }
}
