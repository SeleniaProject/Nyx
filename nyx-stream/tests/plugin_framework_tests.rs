#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::needless_collect,
    clippy::explicit_into_iter_loop,
    clippy::uninlined_format_args,
    clippy::unreachable
)]

//! Plugin Framework Integration Tests
//!
//! Tests for the Protocol Combinator (Plugin Framework) implementation
//! in Nyx Protocol v1.0, including capability negotiation, plugin
//! lifecycle management, and cross-plugin communication.

use nyx_stream::frame::{Frame, FrameType};
use nyx_stream::plugin_framework::{
    Plugin, PluginCapability, PluginError, PluginFrameType, PluginHeader, PluginManager,
    PluginManagerConfig, PluginMetadata, PluginState,
};
use std::collections::HashMap;
use tokio::test;
use tracing_test::traced_test;

/// Example test plugin for compression
#[derive(Debug)]
struct TestCompressionPlugin {
    metadata: PluginMetadata,
    state: PluginState,
    stats: HashMap<String, u64>,
}

impl TestCompressionPlugin {
    pub fn new() -> Self {
        let metadata = PluginMetadata {
            id: 0x10000001,
            name: "TestCompression".to_string(),
            version: "1.0.0".to_string(),
            author: "Nyx Test Suite".to_string(),
            description: "Test compression plugin".to_string(),
            capabilities: vec![PluginCapability {
                name: "compression.test".to_string(),
                version: "1.0".to_string(),
                required: false,
                parameters: HashMap::new(),
            }],
            min_protocol_version: "1.0.0".to_string(),
            priority: 100,
            config_schema: None,
        };

        Self {
            metadata,
            state: PluginState::Unloaded,
            stats: HashMap::new(),
        }
    }
}

#[async_trait::async_trait]
impl Plugin for TestCompressionPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    async fn initialize(&mut self, _config: serde_cbor::Value) -> Result<(), PluginError> {
        self.state = PluginState::Ready;
        Ok(())
    }

    async fn process_frame(
        &mut self,
        _header: &PluginHeader,
        frame: &Frame,
    ) -> Result<Vec<Frame>, PluginError> {
        // Simple passthrough for testing
        *self
            .stats
            .entry("frames_processed".to_string())
            .or_insert(0) += 1;
        Ok(vec![frame.clone()])
    }

    async fn handle_control(
        &mut self,
        _message: serde_cbor::Value,
    ) -> Result<serde_cbor::Value, PluginError> {
        Ok(serde_cbor::Value::Text("OK".to_string()))
    }

    async fn heartbeat(&mut self) -> Result<(), PluginError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.state = PluginState::ShuttingDown;
        Ok(())
    }

    fn state(&self) -> PluginState {
        self.state.clone()
    }

    fn statistics(&self) -> HashMap<String, u64> {
        self.stats.clone()
    }
}

#[test]
#[traced_test]
async fn test_plugin_manager_basic_functionality() {
    let config = PluginManagerConfig::default();
    let manager = PluginManager::new(config);

    // Register compression plugin
    let plugin = Box::new(TestCompressionPlugin::new());
    let plugin_id = manager.register_plugin(plugin).await.unwrap();

    assert_eq!(plugin_id, 0x10000001);

    // Check capabilities are registered
    let capabilities = manager.get_capabilities();
    assert_eq!(capabilities.len(), 1);
    assert_eq!(capabilities[0].name, "compression.test");

    // Unregister plugin
    manager.unregister_plugin(plugin_id).await.unwrap();
}

#[test]
#[traced_test]
async fn test_plugin_frame_processing() {
    let config = PluginManagerConfig::default();
    let manager = PluginManager::new(config);

    // Register plugin
    let plugin = Box::new(TestCompressionPlugin::new());
    let plugin_id = manager.register_plugin(plugin).await.unwrap();

    // Create test frame with plugin header
    let header = PluginHeader {
        id: plugin_id,
        flags: 0,
        data: b"test payload".to_vec(),
    };

    let frame = manager
        .create_plugin_frame(1, 1, PluginFrameType::Data, &header)
        .unwrap();

    assert!(matches!(frame.header.ty, FrameType::Custom(_)));

    // Process the frame
    let result = manager.process_plugin_frame(&frame).await.unwrap();
    assert_eq!(result.len(), 1);

    // Clean up
    manager.unregister_plugin(plugin_id).await.unwrap();
}
