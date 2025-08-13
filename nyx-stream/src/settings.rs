//! Runtime SETTINGS values and watcher.
#![forbid(unsafe_code)]

use tokio::sync::{watch, watch::Receiver};
#[cfg(feature = "plugin")]
use std::collections::HashSet;

use crate::management::{SettingsFrame, Setting};
#[cfg(feature = "plugin")]
use crate::plugin::PluginId;

/// SETTINGS frame IDs for different configuration parameters
pub mod setting_ids {
    pub const MAX_STREAMS: u16 = 0x01;
    pub const MAX_DATA: u16 = 0x02;
    pub const IDLE_TIMEOUT: u16 = 0x03;
    pub const PQ_SUPPORTED: u16 = 0x04;
    #[cfg(feature = "plugin")]
    pub const PLUGIN_REQUIRED: u16 = 0x05;  // New for v1.0 Plugin Framework
}

/// Mutable stream-layer configuration updated via SETTINGS frames.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamSettings {
    pub max_streams: u32,
    pub max_data: u32,
    pub idle_timeout: u16,
    pub pq_supported: bool,
    /// Required plugins that must be supported for connection
    #[cfg(feature = "plugin")]
    pub required_plugins: HashSet<PluginId>,
}

impl Default for StreamSettings {
    fn default() -> Self {
        Self {
            max_streams: 256,
            max_data: 1_048_576,
            idle_timeout: 30,
            pq_supported: false,
            #[cfg(feature = "plugin")]
            required_plugins: HashSet::new(),
        }
    }
}

impl StreamSettings {
    /// Apply values from a received SETTINGS frame.
    pub fn apply(&mut self, frame: &SettingsFrame) {
        for s in &frame.settings {
            match s.id {
                setting_ids::MAX_STREAMS => self.max_streams = s.value,
                setting_ids::MAX_DATA => self.max_data = s.value,
                setting_ids::IDLE_TIMEOUT => self.idle_timeout = s.value as u16,
                setting_ids::PQ_SUPPORTED => self.pq_supported = s.value != 0,
                #[cfg(feature = "plugin")]
                setting_ids::PLUGIN_REQUIRED => {
                    // Plugin ID is encoded in the value field
                    if s.value != 0 {
                        self.required_plugins.insert(s.value);
                    }
                }
                _ => {}
            }
        }
    }

    /// Build a SETTINGS frame representing current config.
    pub fn to_frame(&self) -> SettingsFrame {
        let mut settings = Vec::<Setting>::new();
        settings.push(Setting { id: setting_ids::MAX_STREAMS, value: self.max_streams });
        settings.push(Setting { id: setting_ids::MAX_DATA, value: self.max_data });
        settings.push(Setting { id: setting_ids::IDLE_TIMEOUT, value: self.idle_timeout as u32 });
        settings.push(Setting { id: setting_ids::PQ_SUPPORTED, value: if self.pq_supported { 1 } else { 0 } });
        
        // Add required plugins as separate settings
        #[cfg(feature = "plugin")]
        for &plugin_id in &self.required_plugins {
            settings.push(Setting { id: setting_ids::PLUGIN_REQUIRED, value: plugin_id });
        }
        
        SettingsFrame { settings }
    }

    /// Add a required plugin
    #[cfg(feature = "plugin")]
    pub fn add_required_plugin(&mut self, plugin_id: PluginId) {
        self.required_plugins.insert(plugin_id);
    }

    /// Remove a required plugin
    #[cfg(feature = "plugin")]
    pub fn remove_required_plugin(&mut self, plugin_id: PluginId) -> bool {
        self.required_plugins.remove(&plugin_id)
    }

    /// Check if a plugin is required
    #[cfg(feature = "plugin")]
    pub fn is_plugin_required(&self, plugin_id: PluginId) -> bool {
        self.required_plugins.contains(&plugin_id)
    }

    /// Get all required plugins
    #[cfg(feature = "plugin")]
    pub fn get_required_plugins(&self) -> Vec<PluginId> {
        self.required_plugins.iter().copied().collect()
    }
}

/// Create a watch channel seeded with default settings.
pub fn settings_watch() -> (watch::Sender<StreamSettings>, Receiver<StreamSettings>) {
    watch::channel(StreamSettings::default())
} 