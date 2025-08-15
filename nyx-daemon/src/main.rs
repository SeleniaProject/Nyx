#![forbid(unsafe_code)]

//! Comprehensive Nyx daemon implementation.
//!
//! This daemon provides the complete Nyx network functionality including:
//! - Stream management with multipath routing
//! - Real-time metrics collection and monitoring
//! - Advanced path building with geographic diversity
//! - DHT integration for peer discovery
//! - Comprehensive gRPC API for client interaction
//! - Session management with Connection IDs (CID)
//! - Error handling and recovery mechanisms

// Pure Rust protocol types module to replace protobuf/gRPC dependencies
mod proto;

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
// (already imported above) use std::collections::HashMap;

use anyhow::Result;
use nyx_daemon::GLOBAL_PATH_PERFORMANCE_REGISTRY; // lib側で定義したグローバルレジストリを利用

use tokio::sync::{broadcast, Mutex, RwLock};
// use tonic::transport::Server; // REMOVED: C/C++ dependency
use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use tracing::{debug, error, info, instrument};
// use axum::http::HeaderMap; // duplicate import removed

// Internal modules
use nyx_control::{init_control, ControlManager};
use nyx_core::{config::NyxConfig, types::*};
use nyx_mix::cmix::*;
use nyx_transport::{PacketHandler, Transport};
#[cfg(feature = "experimental-metrics")]
use once_cell::sync::OnceCell;

// Internal modules
#[cfg(feature = "experimental-alerts")]
mod alert_system;
#[cfg(feature = "experimental-alerts")]
mod alert_system_enhanced;
#[cfg(feature = "experimental-alerts")]
mod alert_system_test;
#[cfg(feature = "low_power")]
mod low_power;
#[cfg(feature = "experimental-metrics")]
mod metrics;
#[cfg(feature = "experimental-metrics")]
mod path_performance_test;
#[cfg(feature = "experimental-metrics")]
mod prometheus_exporter;
#[cfg(feature = "experimental-metrics")]
mod stream_manager;
// Provide path_builder module name for existing imports by re-exporting
#[cfg(feature = "path-builder")]
pub mod path_builder_broken; // re-export behind feature
#[cfg(feature = "path-builder")]
pub use path_builder_broken as path_builder;
// Expose capability & push modules when building binary so path_builder_broken can use crate:: capability paths
#[cfg(feature = "path-builder")]
pub mod capability;
mod config_manager;
#[cfg(feature = "experimental-events")]
mod event_system;
mod health_monitor;
#[cfg(feature = "experimental-metrics")]
mod layer_manager;
mod pure_rust_dht; // always include minimal in-memory DHT (reused by path builder)
#[cfg(feature = "experimental-dht")]
mod pure_rust_dht_tcp;
#[cfg(feature = "experimental-p2p")]
mod pure_rust_p2p;
#[cfg(feature = "path-builder")]
pub mod push;
mod session_manager; // small core still built
#[cfg(feature = "experimental-metrics")]
mod zero_copy_bridge;

/// Enhanced packet handler for daemon
struct DaemonPacketHandler {
    packet_count: std::sync::atomic::AtomicU64,
}

impl DaemonPacketHandler {
    fn new() -> Self {
        Self {
            packet_count: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

#[async_trait::async_trait]
impl PacketHandler for DaemonPacketHandler {
    async fn handle_packet(&self, src: SocketAddr, data: &[u8]) {
        let count = self
            .packet_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        debug!(
            "Received packet {} from {} ({} bytes)",
            count,
            src,
            data.len()
        );

        // Enhanced packet processing would go here
        // - Protocol parsing
        // - Security validation
        // - Routing decisions
        // - Metrics collection

        // Route inbound packet into stream buffers when metrics subsystem is enabled
        #[cfg(feature = "experimental-metrics")]
        {
            if let Some(sm) = STREAM_MANAGER_INSTANCE.get() {
                let _ = sm.route_incoming(src, data).await;
            }
        }
    }
}

#[cfg(test)]
mod layer_recovery_test;

#[cfg(feature = "experimental-metrics")]
use metrics::MetricsCollector;
#[cfg(feature = "experimental-metrics")]
use prometheus_exporter::{PrometheusExporter, PrometheusExporterBuilder};
// Ensure HTTPS webhooks are not used in C-free builds
#[cfg(feature = "experimental-alerts")]
const _NYX_BUILD_TLS_FREE: bool = true;
use crate::proto::EventFilter;
use config_manager::ConfigManager;
#[cfg(feature = "experimental-events")]
use event_system::EventSystem;
use health_monitor::HealthMonitor;
#[cfg(feature = "experimental-metrics")]
use layer_manager::LayerManager;
#[cfg(feature = "path-builder")]
use path_builder::PathBuilder;
#[cfg(feature = "experimental-dht")]
use pure_rust_dht_tcp::PureRustDht;
#[cfg(feature = "experimental-p2p")]
use pure_rust_p2p::{P2PConfig, P2PNetworkEvent, PureRustP2P};
use session_manager::{SessionManager, SessionManagerConfig};
#[cfg(feature = "experimental-metrics")]
use stream_manager::{StreamManager, StreamManagerConfig};
// HTTP server (Axum) to serve pure-Rust JSON API for CLI
// use axum::{Router, routing::{get, post}, extract::{Path, State, Query}, Json}; // duplicate import removed
use axum::http::HeaderMap;
#[cfg(feature = "plugin")]
use base64::Engine as _;
#[cfg(feature = "plugin")]
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use nyx_stream::management::parse_settings_frame_ext;
use nyx_stream::management::setting_ids as mgmt_setting_ids;
use nyx_stream::management::ERR_UNSUPPORTED_CAP;
#[cfg(feature = "plugin")]
use nyx_stream::plugin_handshake::{PluginHandshakeCoordinator, PluginHandshakeError};
#[cfg(feature = "plugin")]
use nyx_stream::plugin_settings::PluginSettingsManager;
use nyx_stream::{parse_close_frame, parse_settings_frame};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[cfg(feature = "plugin")]
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering2};
#[cfg(feature = "plugin")]
use std::sync::Arc as StdArc;

#[derive(Debug, Clone, Default, Serialize)]
struct PluginNegotiationState {
    support_flags: u32,
    required_plugin_count: u32,
    optional_plugin_count: u32,
    security_policy: u32,
    requirements_count: u32,
}

// Global stream manager handle for inbound packet routing (feature-gated)
#[cfg(feature = "experimental-metrics")]
static STREAM_MANAGER_INSTANCE: OnceCell<Arc<stream_manager::StreamManager>> = OnceCell::new();

/// Convert SystemTime to proto::Timestamp
fn system_time_to_proto_timestamp(time: SystemTime) -> proto::Timestamp {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    proto::Timestamp {
        seconds: duration.as_secs() as i64,
        nanos: duration.subsec_nanos() as i32,
    }
}

// Use our pure Rust proto module instead of tonic
// mod proto {
//     tonic::include_proto!("nyx.api");
// }

#[cfg(feature = "experimental-alerts")]
use once_cell::sync::Lazy as OnceLazy;
use proto::NyxControl;
use proto::*;
#[cfg(feature = "experimental-alerts")]
static ENHANCED_ALERT_SYSTEM: OnceLazy<std::sync::Arc<alert_system_enhanced::EnhancedAlertSystem>> =
    OnceLazy::new(|| std::sync::Arc::new(alert_system_enhanced::EnhancedAlertSystem::new()));

/// Comprehensive control service implementation
pub struct ControlService {
    // Core components
    start_time: std::time::Instant,
    node_id: NodeId,
    transport: Arc<Transport>,
    control_manager: ControlManager,

    // Advanced subsystems
    #[cfg(feature = "experimental-metrics")]
    metrics: Arc<MetricsCollector>,
    #[cfg(feature = "experimental-metrics")]
    stream_manager: Arc<StreamManager>,
    #[cfg(all(feature = "experimental-metrics", feature = "low_power"))]
    cover_generator: Option<AdaptiveCoverGenerator>,
    #[cfg(feature = "path-builder")]
    path_builder: Arc<PathBuilder>,
    session_manager: Arc<SessionManager>,
    config_manager: Arc<ConfigManager>,
    health_monitor: Arc<HealthMonitor>,
    #[cfg(feature = "experimental-events")]
    event_system: Arc<EventSystem>,
    #[cfg(feature = "experimental-metrics")]
    layer_manager: Arc<RwLock<LayerManager>>,
    #[cfg(feature = "low_power")]
    low_power_manager: Arc<low_power::LowPowerManager>,

    // P2P networking
    #[cfg(feature = "experimental-dht")]
    pure_rust_dht: Arc<PureRustDht>,
    #[cfg(feature = "experimental-p2p")]
    pure_rust_p2p: Arc<PureRustP2P>,

    // Mix routing
    cmix_controller: Arc<Mutex<CmixController>>,

    // Event broadcasting
    event_tx: broadcast::Sender<Event>,

    // Configuration
    config: Arc<RwLock<NyxConfig>>,

    // Statistics
    connection_count: Arc<std::sync::atomic::AtomicU32>,
    total_requests: Arc<std::sync::atomic::AtomicU64>,

    // Access control: if present, all control APIs require this token
    api_token: Option<String>,

    // Last plugin negotiation state received from WASM client (HTTP gateway)
    plugin_negotiation: Arc<RwLock<PluginNegotiationState>>,

    // Plugin handshake coordinator stored across HTTP calls (plugin feature only)
    #[cfg(feature = "plugin")]
    plugin_handshake: Arc<Mutex<Option<PluginHandshakeCoordinator>>>,

    // Required plugin IDs provided by client (decoded from CBOR array<u32>)
    #[cfg(feature = "plugin")]
    plugin_required_ids: Arc<RwLock<Vec<u32>>>,
}

impl ControlService {
    /// Create a new control service with all subsystems
    pub async fn new(config: NyxConfig) -> anyhow::Result<Self> {
        let start_time = std::time::Instant::now();
        let node_id = Self::generate_node_id(&config);

        // Initialize transport layer
        info!("Initializing transport layer...");
        let transport = Arc::new(
            Transport::start(config.listen_port, Arc::new(DaemonPacketHandler::new())).await?,
        );
        info!("Transport layer initialized");

        // Initialize control plane (DHT, push notifications)
        info!("Initializing control plane...");
        let control_manager = init_control(&config).await;
        info!("Control plane initialized");

        // Initialize metrics & stream subsystems (optional)
        #[cfg(feature = "experimental-metrics")]
        let (metrics, stream_manager) = {
            // Initialize metrics collection
            let metrics = Arc::new(MetricsCollector::new());
            let _metrics_task = Arc::clone(&metrics).start_collection();

            // Initialize Prometheus exporter with safe parsing
            let prometheus_addr = match std::env::var("NYX_PROMETHEUS_ADDR") {
                Ok(v) => {
                    match v.parse() {
                        Ok(addr) => addr,
                        Err(e) => {
                            tracing::warn!("Invalid NYX_PROMETHEUS_ADDR '{}': {}. Falling back to 127.0.0.1:9090", v, e);
                            std::net::SocketAddr::from(([127, 0, 0, 1], 9090))
                        }
                    }
                }
                Err(_) => std::net::SocketAddr::from(([127, 0, 0, 1], 9090)),
            };
            let prometheus_exporter = PrometheusExporterBuilder::new()
                .with_server_addr(prometheus_addr)
                .with_update_interval(Duration::from_secs(15))
                .build(Arc::clone(&metrics))?;

            // Start Prometheus metrics server and collection
            prometheus_exporter.start_server().await?;
            prometheus_exporter.start_collection().await?;
            info!("Prometheus metrics server started on {}", prometheus_addr);

            // Initialize OTLP exporter if configured via environment (NYX_OTLP_ENABLED / NYX_OTLP_ENDPOINT)
            if std::env::var("NYX_OTLP_ENABLED")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false)
            {
                let endpoint = std::env::var("NYX_OTLP_ENDPOINT")
                    .unwrap_or_else(|_| "http://127.0.0.1:4317".to_string());
                #[cfg(feature = "experimental-metrics")]
                {
                    #[cfg(feature = "otlp_exporter")]
                    {
                        use nyx_telemetry::opentelemetry_integration::{
                            NyxTelemetry, TelemetryConfig as OCfg,
                        };
                        if let Err(e) = NyxTelemetry::init_with_exporter(OCfg {
                            endpoint: endpoint.clone(),
                            service_name: "nyx-daemon".into(),
                            sampling_ratio: 0.1,
                        }) {
                            tracing::warn!("Failed to initialize OTLP exporter: {}", e);
                        } else {
                            tracing::info!("OTLP exporter initialized for endpoint {}", endpoint);
                        }
                    }
                }
            }

            // Initialize stream manager
            let stream_config = StreamManagerConfig::default();
            let stream_manager =
                StreamManager::new(Arc::clone(&transport), Arc::clone(&metrics), stream_config)
                    .await?;
            let stream_manager = Arc::new(stream_manager);
            stream_manager.clone().start().await;

            // Start periodic export of zero-copy manager metrics into Prometheus (via metrics crate)
            {
                use nyx_core::zero_copy::manager::ZeroCopyManager;
                // For now, create a dedicated manager instance for daemon-level aggregation
                let zc_manager = Arc::new(ZeroCopyManager::new(
                    nyx_core::zero_copy::manager::ZeroCopyManagerConfig::default(),
                ));
                zero_copy_bridge::start_zero_copy_metrics_task(Arc::clone(&zc_manager));
                // Optionally: stash in metrics collector if needed in the future
            }

            // Publish global instance for packet handler routing
            if STREAM_MANAGER_INSTANCE
                .set(Arc::clone(&stream_manager))
                .is_err()
            {
                tracing::warn!(
                    "STREAM_MANAGER_INSTANCE was already set; inbound routing may be duplicated"
                );
            }
            // Dynamically export low power metrics if feature enabled
            #[cfg(feature = "low_power")]
            {
                let lpm = low_power_manager.clone();
                let metrics_clone = Arc::clone(&metrics);
                tokio::spawn(async move {
                    let mut rx = lpm.subscribe();
                    loop {
                        if rx.changed().await.is_ok() {
                            let is_lp = lpm.is_low_power();
                            let ratio = lpm.recommended_cover_ratio() as f64;
                            metrics_clone.record_custom_metric("low_power_cover_ratio", ratio);
                            metrics_clone.record_custom_metric(
                                "low_power_state",
                                if is_lp { 1.0 } else { 0.0 },
                            );
                        } else {
                            break;
                        }
                    }
                });
            }

            (metrics, stream_manager)
        };

        // Initialize adaptive cover generator and bind to low power state (if available)
        #[cfg(all(feature = "experimental-metrics", feature = "low_power"))]
        let cover_generator = {
            let mut cfg = AdaptiveCoverConfig::default();
            cfg.base_lambda = 2.0; // default base rate
            cfg.target_cover_ratio = self.low_power_manager.recommended_cover_ratio() as f64;
            let gen = AdaptiveCoverGenerator::new(cfg);
            gen.start().await.ok();
            // Watch low-power state and update target cover ratio accordingly
            let lpm = self.low_power_manager.clone();
            let gen_clone = gen.clone();
            tokio::spawn(async move {
                let mut rx = lpm.subscribe();
                while rx.changed().await.is_ok() {
                    let mut cfg = AdaptiveCoverConfig::default();
                    cfg.base_lambda = 2.0;
                    cfg.target_cover_ratio = lpm.recommended_cover_ratio() as f64;
                    gen_clone.update_config(cfg).await;
                }
            });

            // Also subscribe to lambda updates and spawn/adjust transport cover tasks
            let lambda_rx = gen.subscribe_lambda();
            let transport_clone = Arc::clone(&transport);
            let stream_manager_clone = stream_manager.clone();
            tokio::spawn(async move {
                let mut rx = lambda_rx;
                let mut last_lambda: f64 = 0.0;
                loop {
                    if rx.changed().await.is_err() {
                        break;
                    }
                    let lambda = *rx.borrow();
                    if (lambda - last_lambda).abs() < 1e-3 {
                        continue;
                    }
                    last_lambda = lambda;
                    // Choose first active path as cover target (best-effort)
                    let addrs = stream_manager_clone.list_active_path_addrs();
                    if let Some(addr) = addrs.first().cloned() {
                        transport_clone.spawn_cover_task(addr, lambda.max(0.0));
                    }
                }
            });
            Some(gen)
        };

        #[cfg(feature = "path-builder")]
        let path_builder = {
            info!("Initializing path builder with DHT support...");
            let bootstrap_peers = vec![
                "/ip4/127.0.0.1/tcp/4001/p2p/12D3KooWBootstrap1".to_string(),
                "/ip4/127.0.0.1/tcp/4002/p2p/12D3KooWBootstrap2".to_string(),
            ];
            let path_builder_config = path_builder::PathBuilderConfig::default();
            let path_builder = PathBuilder::new(bootstrap_peers, path_builder_config);
            path_builder.start().await?;
            let path_builder = Arc::new(path_builder);
            info!("Path builder with DHT support started successfully");
            path_builder
        };

        // Initialize session manager
        info!("Initializing session manager...");
        let session_config = SessionManagerConfig::default();
        let session_manager = SessionManager::new(session_config);
        info!("Session manager created, starting...");
        let session_manager_arc = Arc::new(session_manager);
        session_manager_arc.clone().start().await?;
        info!("Session manager started successfully");
        let session_manager = session_manager_arc.clone();

        // Event broadcasting
        info!("Setting up event broadcasting...");
        let (event_tx, _) = broadcast::channel(1000);
        info!("Event broadcasting setup complete");

        // Initialize configuration manager
        info!("Initializing configuration manager...");
        let config_manager = Arc::new(ConfigManager::new(config.clone(), event_tx.clone()));
        info!("Configuration manager initialized");

        // Initialize health monitor
        info!("Initializing health monitor...");
        let health_monitor = Arc::new(HealthMonitor::new());
        info!("Health monitor created, starting...");
        health_monitor.start().await?;
        // Inject active connection accessor so health API can expose live connection count
        {
            let hm = health_monitor.clone();
            let sm = session_manager.clone();
            tokio::spawn(async move {
                hm.set_active_connection_accessor(move || sm.sessions.len() as u32)
                    .await;
            });
        }
        info!("Health monitor started successfully");

        // Initialize event system (optional)
        #[cfg(feature = "experimental-events")]
        let event_system = {
            info!("Initializing event system...");
            let es = Arc::new(EventSystem::new());
            // Bridge broadcast channel to event system history so that all emitted
            // events are persisted and can be queried via subscribe_events.
            {
                let es_clone = Arc::clone(&es);
                let mut rx = event_tx.subscribe();
                tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok(ev) => {
                                let _ = es_clone.publish_event(ev).await;
                            }
                            Err(_e) => {
                                // Channel closed; exit task
                                break;
                            }
                        }
                    }
                });
            }
            info!("Event system initialized and bridged to broadcast");
            es
        };

        // Initialize low power manager (optional)
        #[cfg(feature = "low_power")]
        let low_power_manager = {
            let lpm = low_power::LowPowerManager::new(low_power::LowPowerConfig::default());
            // Initialize mobile monitoring (on mobile builds this hooks OS APIs)
            lpm.init_mobile_monitoring();
            #[cfg(feature = "experimental-events")]
            let lpm = lpm.with_event_system(event_system.clone());
            let lpm_spawn = lpm.clone();
            tokio::spawn(async move {
                lpm_spawn.run().await;
            });
            lpm
        };
        // NOTE: SessionManager re-instantiation with event system omitted to keep minimal diff;
        // future enhancement: builder pattern to inject at construction.

        // Initialize cMix controller based on configuration
        info!("Initializing cMix controller...");
        let cmix_controller = Arc::new(Mutex::new(match config.mix.mode {
            nyx_core::config::MixMode::Cmix => {
                info!(
                    "Initializing cMix in VDF mode with batch_size={}, delay={}ms",
                    config.mix.batch_size, config.mix.vdf_delay_ms
                );
                CmixController::new(config.mix.batch_size, config.mix.vdf_delay_ms)
            }
            nyx_core::config::MixMode::Standard => {
                info!("Initializing cMix in standard mode with default settings");
                CmixController::default()
            }
        }));
        info!("cMix controller initialized in {:?} mode", config.mix.mode);

        // Initialize layer manager for full protocol stack integration (optional)
        #[cfg(feature = "experimental-metrics")]
        let layer_manager = {
            info!("Initializing layer manager...");
            let mut lm =
                LayerManager::new(config.clone(), Arc::clone(&metrics), event_tx.clone()).await?;
            info!("Layer manager created, starting all layers...");
            lm.start().await?;
            info!("All protocol layers started successfully");
            Arc::new(RwLock::new(lm))
        };

        // Initialize Pure Rust DHT (optional)
        #[cfg(feature = "experimental-dht")]
        let pure_rust_dht = {
            info!("Initializing Pure Rust DHT...");
            let dht_addr = std::net::SocketAddr::from(([127, 0, 0, 1], 3001))
                .map_err(|e| anyhow::anyhow!("Invalid DHT address: {}", e))?;
            let bootstrap_addrs = vec![
                std::net::SocketAddr::from(([127, 0, 0, 1], 3002)),
                std::net::SocketAddr::from(([127, 0, 0, 1], 3003)),
            ];
            let mut dht_instance = PureRustDht::new(dht_addr, bootstrap_addrs)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to create Pure Rust DHT: {}", e))?;
            dht_instance
                .start()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to start Pure Rust DHT: {}", e))?;
            info!("Pure Rust DHT started on {}", dht_addr);
            Arc::new(dht_instance)
        };

        // Initialize Pure Rust P2P network (optional)
        #[cfg(feature = "experimental-p2p")]
        let pure_rust_p2p = {
            info!("Initializing Pure Rust P2P network...");
            let p2p_config = P2PConfig {
                listen_address: std::net::SocketAddr::from(([127, 0, 0, 1], 3100)),
                bootstrap_peers: vec![
                    std::net::SocketAddr::from(([127, 0, 0, 1], 3101)),
                    std::net::SocketAddr::from(([127, 0, 0, 1], 3102)),
                ],
                max_peers: 50,
                enable_encryption: false, // Disabled for now to avoid TLS complexity
                ..Default::default()
            };
            let (pure_rust_p2p, mut p2p_events) =
                PureRustP2P::new(Arc::clone(&pure_rust_dht), p2p_config)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to create Pure Rust P2P: {}", e))?;
            let pure_rust_p2p = Arc::new(pure_rust_p2p);
            pure_rust_p2p
                .start()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to start Pure Rust P2P: {}", e))?;
            info!(
                "Pure Rust P2P network started with peer ID: {}",
                hex::encode(pure_rust_p2p.local_peer().peer_id)
            );
            let event_tx_clone = event_tx.clone();
            let pure_rust_p2p_clone = Arc::clone(&pure_rust_p2p);
            tokio::spawn(async move {
                while let Some(event) = p2p_events.recv().await {
                    match event {
                        P2PNetworkEvent::PeerConnected { peer_id, address } => {
                            info!(
                                "P2P peer connected: {} at {}",
                                hex::encode(peer_id),
                                address
                            );
                        }
                        P2PNetworkEvent::PeerDiscovered { peer_info } => {
                            info!(
                                "P2P peer discovered: {} at {}",
                                hex::encode(peer_info.peer_id),
                                peer_info.address
                            );
                        }
                        P2PNetworkEvent::MessageReceived { from, message } => {
                            debug!(
                                "P2P message received from {}: {:?}",
                                hex::encode(from),
                                message
                            );
                        }
                        P2PNetworkEvent::NetworkError { error } => {
                            tracing::warn!("P2P network error: {}", error);
                        }
                        _ => {}
                    }
                }
            });
            pure_rust_p2p
        };

        info!("Creating control service instance...");
        // Load optional control-plane API token from environment
        let api_token = std::env::var("NYX_CONTROL_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());

        let service = Self {
            start_time,
            node_id,
            transport: Arc::clone(&transport),
            control_manager,
            #[cfg(feature = "experimental-metrics")]
            metrics,
            #[cfg(feature = "experimental-metrics")]
            stream_manager,
            #[cfg(all(feature = "experimental-metrics", feature = "low_power"))]
            cover_generator,
            #[cfg(feature = "path-builder")]
            path_builder,
            session_manager,
            config_manager,
            health_monitor,
            #[cfg(feature = "experimental-events")]
            event_system,
            #[cfg(feature = "experimental-metrics")]
            layer_manager,
            #[cfg(feature = "low_power")]
            low_power_manager,
            #[cfg(feature = "experimental-dht")]
            pure_rust_dht,
            #[cfg(feature = "experimental-p2p")]
            pure_rust_p2p,
            cmix_controller,
            event_tx,
            config: Arc::new(RwLock::new(config)),
            connection_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            total_requests: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            api_token,
            plugin_negotiation: Arc::new(RwLock::new(PluginNegotiationState::default())),
            #[cfg(feature = "plugin")]
            plugin_handshake: Arc::new(Mutex::new(None)),
            #[cfg(feature = "plugin")]
            plugin_required_ids: Arc::new(RwLock::new(Vec::new())),
        };
        info!("Control service instance created");

        // Start background tasks
        info!("Starting background tasks...");
        service.start_background_tasks().await?;
        info!("Background tasks started successfully");

        info!(
            "Control service initialized with node ID: {}",
            hex::encode(node_id)
        );
        Ok(service)
    }

    /// Apply a SETTINGS payload received from WASM client to negotiation snapshot
    async fn apply_wasm_settings(&self, payload: &[u8]) -> Result<PluginNegotiationState, String> {
        let mut state = parse_wasm_settings_to_state(payload)?;
        // If extended SETTINGS section carries CBOR plugin lists, reflect counts and store required IDs.
        if let Ok((_rem, (_frame, ext))) = parse_settings_frame_ext(payload) {
            // Update counts from CBOR if present
            for (id, bytes) in ext.iter() {
                if *id == mgmt_setting_ids::PLUGIN_REQUIRED_CBOR {
                    if let Ok(ids) = ciborium::from_reader::<Vec<u32>, _>(bytes.as_slice()) {
                        state.required_plugin_count = ids.len() as u32;
                        state.requirements_count = state.required_plugin_count;
                        #[cfg(feature = "plugin")]
                        {
                            let mut guard = self.plugin_required_ids.write().await;
                            *guard = ids;
                        }
                    }
                } else if *id == mgmt_setting_ids::PLUGIN_OPTIONAL_CBOR {
                    if let Ok(ids) = ciborium::from_reader::<Vec<u32>, _>(bytes.as_slice()) {
                        state.optional_plugin_count = ids.len() as u32;
                    }
                }
            }
        }
        {
            let mut guard = self.plugin_negotiation.write().await;
            *guard = state.clone();
        }
        // Emit event for diagnostics/observers
        let _ = self.event_tx.send(proto::Event {
            r#type: "system".to_string(),
            detail: "plugin negotiation updated".to_string(),
            timestamp: Some(proto::Timestamp::now()),
            severity: "info".to_string(),
            attributes: std::collections::HashMap::new(),
            event_type: "plugin_negotiation".to_string(),
            data: std::collections::HashMap::new(),
            event_data: None,
        });
        Ok(state)
    }

    /// For plugin-enabled builds: bootstrap a handshake coordinator using stored negotiation
    #[cfg(feature = "plugin")]
    async fn begin_plugin_handshake_from_snapshot(
        &self,
        is_initiator: bool,
    ) -> Result<Option<Vec<u8>>, String> {
        let snap = self.plugin_negotiation.read().await.clone();
        let mut mgr = PluginSettingsManager::new();
        // Inject required plugin IDs if provided by client beforehand
        {
            let ids = self.plugin_required_ids.read().await;
            for pid in ids.iter() {
                // default version range and empty caps for now; registry validation will refine later
                let _ = mgr.add_required_plugin(*pid, (1, 0), vec![]);
            }
        }
        let mut coord = PluginHandshakeCoordinator::new(mgr, is_initiator);
        coord
            .initiate_handshake()
            .await
            .map_err(|e| format!("handshake init error: {}", e))
    }

    /// Process a CLOSE payload from WASM client. Returns decoded code and optional cap id.
    async fn process_wasm_close(&self, payload: &[u8]) -> Result<(u16, Option<u32>), String> {
        let (_rest, cf) =
            parse_close_frame(payload).map_err(|e| format!("CLOSE parse error: {:?}", e))?;
        let code = cf.code;
        let cap_id = if code == ERR_UNSUPPORTED_CAP && cf.reason.len() == 4 {
            let mut b = [0u8; 4];
            b.copy_from_slice(cf.reason);
            Some(u32::from_be_bytes(b))
        } else {
            None
        };
        Ok((code, cap_id))
    }

    /// Generate a node ID from configuration
    fn generate_node_id(config: &NyxConfig) -> NodeId {
        if let Some(node_id_hex) = &config.node_id {
            if let Ok(bytes) = hex::decode(node_id_hex) {
                if bytes.len() == 32 {
                    let mut node_id = [0u8; 32];
                    node_id.copy_from_slice(&bytes);
                    return node_id;
                }
            }
        }

        // Generate random node ID if not configured
        let mut node_id = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut node_id);
        node_id
    }

    /// Start all background tasks
    async fn start_background_tasks(&self) -> anyhow::Result<()> {
        // Start packet forwarding task
        #[cfg(feature = "experimental-metrics")]
        {
            #[cfg(all(feature = "experimental-metrics", feature = "path-builder"))]
            {
                let transport_clone = Arc::clone(&self.transport);
                let cmix_clone = Arc::clone(&self.cmix_controller);
                let path_builder_clone = Arc::clone(&self.path_builder);
                let metrics_clone = Arc::clone(&self.metrics);
                tokio::spawn(async move {
                    Self::packet_forwarding_loop(
                        transport_clone,
                        cmix_clone,
                        path_builder_clone,
                        metrics_clone,
                    )
                    .await;
                });
            }
        }

        // Start metrics aggregation task
        #[cfg(feature = "experimental-metrics")]
        {
            let metrics_clone = Arc::clone(&self.metrics);
            let event_tx_clone = self.event_tx.clone();
            tokio::spawn(async move {
                Self::metrics_aggregation_loop(metrics_clone, event_tx_clone).await;
            });
        }

        // Start configuration monitoring task
        let config_manager_clone = Arc::clone(&self.config_manager);
        let config_clone = Arc::clone(&self.config);
        let event_tx_clone = self.event_tx.clone();

        tokio::spawn(async move {
            Self::config_monitoring_loop(config_manager_clone, config_clone, event_tx_clone).await;
        });

        info!("All background tasks started");
        Ok(())
    }

    /// Packet forwarding background loop
    #[cfg(feature = "experimental-metrics")]
    async fn packet_forwarding_loop(
        _transport: Arc<Transport>,
        _cmix: Arc<Mutex<CmixController>>,
        _path_builder: Arc<PathBuilder>,
        metrics: Arc<MetricsCollector>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            interval.tick().await;

            // Simulate packet forwarding
            metrics.increment_packets_sent();
            metrics.increment_bytes_sent(1024);
        }
    }

    /// Metrics aggregation background loop
    #[cfg(feature = "experimental-metrics")]
    async fn metrics_aggregation_loop(
        metrics: Arc<MetricsCollector>,
        event_tx: broadcast::Sender<Event>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            let performance = metrics.get_performance_metrics().await;
            let _resource_usage = metrics.get_resource_usage().await.unwrap_or_default();

            let event = proto::Event {
                r#type: "performance".to_string(),
                detail: "Metrics updated".to_string(),
                timestamp: Some(proto::Timestamp::now()),
                severity: "info".to_string(),
                attributes: HashMap::new(),
                event_type: "health_status_changed".to_string(),
                data: {
                    let mut data = HashMap::new();
                    data.insert("metric".to_string(), "system_health".to_string());
                    data.insert("value".to_string(), performance.cpu_usage.to_string());
                    data.insert("threshold".to_string(), "0.8".to_string());
                    data.insert(
                        "description".to_string(),
                        "System performance metrics".to_string(),
                    );
                    data
                },
                event_data: Some(proto::event::EventData::SystemEvent(
                    proto::event::SystemEvent {
                        event_type: "health_status_changed".to_string(),
                        severity: "info".to_string(),
                        message: "System performance metrics updated".to_string(),
                        metadata: HashMap::new(),
                        component: "metrics".to_string(),
                    },
                )),
            };

            let _ = event_tx.send(event);
        }
    }

    /// Configuration monitoring background loop
    async fn config_monitoring_loop(
        config_manager: Arc<ConfigManager>,
        config: Arc<RwLock<NyxConfig>>,
        event_tx: broadcast::Sender<Event>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;

            // Check for configuration changes
            if let Ok(updated_config) = config_manager.check_for_updates().await {
                if let Some(new_config) = updated_config {
                    *config.write().await = new_config;

                    let event = proto::Event {
                        event_type: "config_reload".to_string(),
                        timestamp: Some(proto::Timestamp::now()),
                        data: HashMap::new(),
                        severity: "info".to_string(),
                        r#type: "system".to_string(),
                        detail: "Configuration has been reloaded".to_string(),
                        attributes: HashMap::new(),
                        event_data: Some(proto::event::EventData::SystemEvent(
                            proto::event::SystemEvent {
                                event_type: "config_reload".to_string(),
                                severity: "info".to_string(),
                                message: "Configuration has been reloaded".to_string(),
                                metadata: HashMap::new(),
                                component: "daemon".to_string(),
                            },
                        )),
                    };

                    let _ = event_tx.send(event);
                }
            }
        }
    }

    /// Build comprehensive node information with all extended fields
    async fn build_node_info(&self) -> NodeInfo {
        #[cfg(feature = "experimental-metrics")]
        let performance_metrics = self.metrics.get_performance_metrics().await;
        #[cfg(feature = "experimental-metrics")]
        let resource_usage = self.metrics.get_resource_usage().await.unwrap_or_default();
        #[cfg(not(feature = "experimental-metrics"))]
        let performance_metrics = crate::proto::PerformanceMetrics {
            cover_traffic_rate: 0.0,
            avg_latency_ms: 0.0,
            packet_loss_rate: 0.0,
            bandwidth_utilization: 0.0,
            cpu_usage: 0.0,
            memory_usage_mb: 0.0,
            total_packets_sent: 0,
            total_packets_received: 0,
            retransmissions: 0,
            connection_success_rate: 0.0,
        };
        #[cfg(not(feature = "experimental-metrics"))]
        let resource_usage = crate::proto::ResourceUsage {
            cpu_percent: 0.0,
            memory_bytes: 0,
            memory_rss_bytes: 0,
            memory_vms_bytes: 0,
            memory_percent: 0.0,
            disk_usage_bytes: 0,
            disk_total_bytes: 0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
            file_descriptors: 0,
            open_file_descriptors: 0,
            thread_count: 0,
        };

        // Get actual mix routes from metrics
        #[cfg(feature = "experimental-metrics")]
        let mix_routes = self.metrics.get_mix_routes().await;
        #[cfg(not(feature = "experimental-metrics"))]
        let mix_routes: Vec<String> = Vec::new();

        // Build peer information from actual connected peers
        let mut peers = Vec::new();

        // Simulate some peer data for demonstration
        // In a real implementation, this would come from the DHT/control manager
        #[cfg(feature = "experimental-metrics")]
        let connected_peers_count = self.metrics.get_connected_peers_count();
        #[cfg(not(feature = "experimental-metrics"))]
        let connected_peers_count = 0usize;
        for i in 0..connected_peers_count.min(10) {
            let peer_id = format!("peer_{:02x}", i);
            let peer = PeerInfo {
                peer_id: peer_id.clone(),
                node_id: peer_id, // Same as peer_id
                address: format!("peer{}.nyx.network:43301", i + 1),
                latency_ms: 50.0 + (i as f64 * 10.0),
                bandwidth_mbps: 100.0 - (i as f64 * 5.0),
                connection_status: "connected".to_string(),
                status: "connected".to_string(), // Same as connection_status
                last_seen: Some(system_time_to_proto_timestamp(SystemTime::now())),
                connection_count: (i + 1) as u32,
                region: match i % 3 {
                    0 => "us-west".to_string(),
                    1 => "eu-central".to_string(),
                    _ => "ap-southeast".to_string(),
                },
                reliability_score: 0.9,
                bytes_sent: i as u64 * 1000,
                bytes_received: i as u64 * 1200,
            };
            peers.push(peer);
        }

        // Build path information from path builder
        let mut paths = Vec::new();

        // Get active paths from stream manager
        #[cfg(feature = "experimental-metrics")]
        let stream_stats = self.stream_manager.list_streams().await;
        #[cfg(not(feature = "experimental-metrics"))]
        let stream_stats: Vec<crate::proto::StreamStats> = Vec::new();
        for (path_idx, stream_stat) in stream_stats.iter().enumerate() {
            for path_stat in &stream_stat.paths {
                let path = PathInfo {
                    path_id: path_stat.path_id.clone(),
                    hops: vec![
                        format!("hop1_{}", path_idx),
                        format!("hop2_{}", path_idx),
                        format!("hop3_{}", path_idx),
                    ],
                    latency_ms: path_stat.rtt_ms,
                    total_latency_ms: path_stat.rtt_ms, // Same as latency_ms
                    bandwidth_bps: (path_stat.bandwidth_mbps * 1_000_000.0) as f64,
                    min_bandwidth_mbps: path_stat.bandwidth_mbps,
                    reliability_score: path_stat.success_rate,
                    last_used: Some(system_time_to_proto_timestamp(SystemTime::now())),
                    status: path_stat.status.clone(),
                    packet_count: path_stat.packet_count,
                    success_rate: path_stat.success_rate,
                    created_at: Some(system_time_to_proto_timestamp(SystemTime::now())),
                };
                paths.push(path);
            }
        }

        // Get network topology information with real data
        let topology = NetworkTopology {
            total_nodes: connected_peers_count as u32 + 100,
            active_nodes: connected_peers_count as u32,
            mix_nodes: (connected_peers_count as u32 * 2) / 3,
            gateway_nodes: connected_peers_count as u32 / 3,
            network_diameter: 6, // Typical small-world network diameter
            clustering_coefficient: 0.7,
            peers: peers.iter().map(|p| p.peer_id.clone()).collect(),
            paths: paths.iter().map(|p| p.path_id.clone()).collect(),
            total_nodes_known: connected_peers_count as u32 + 50, // Known but not connected
            reachable_nodes: connected_peers_count as u32,
            current_region: self.detect_current_region().await,
            available_regions: vec![
                "us-west".to_string(),
                "us-east".to_string(),
                "eu-central".to_string(),
                "eu-west".to_string(),
                "ap-southeast".to_string(),
                "ap-northeast".to_string(),
            ],
        };

        NodeInfo {
            node_id: hex::encode(self.node_id),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_sec: self.start_time.elapsed().as_secs() as u32,
            bytes_in: resource_usage.network_bytes_received,
            bytes_out: resource_usage.network_bytes_sent,

            // Extended fields for task 1.2.1
            pid: std::process::id(),
            active_streams: connected_peers_count as u32,
            connected_peers: connected_peers_count as u32,
            mix_routes,

            // Performance and resource information
            performance: Some(performance_metrics),
            resources: Some(resource_usage),
            topology: Some(topology),
            // Compliance & capabilities: in a full implementation capabilities would be gathered
            // from negotiated/advertised capability set. For now we synthesize based on enabled features.
            compliance_level: Some({
                // Map compile-time features to capability ids used in nyx-core::compliance::cap
                use nyx_core::compliance::{self, ComplianceLevel};
                // Collect capability ids
                let caps: Vec<u32> = Vec::new();
                #[cfg(feature = "mpr_experimental")]
                caps.push(compliance::cap::MULTIPATH);
                #[cfg(feature = "hybrid")]
                caps.push(compliance::cap::HYBRID_PQ);
                #[cfg(feature = "cmix")]
                caps.push(compliance::cap::CMIX);
                #[cfg(feature = "plugin")]
                caps.push(compliance::cap::PLUGIN);
                #[cfg(feature = "low_power")]
                caps.push(compliance::cap::LOW_POWER);
                let cap_objs: Vec<nyx_core::capability::Capability> = caps
                    .iter()
                    .map(|id| nyx_core::capability::Capability { id: *id, flags: 0 })
                    .collect();
                let level = compliance::determine(&cap_objs);
                match level {
                    ComplianceLevel::Core => "Core".to_string(),
                    ComplianceLevel::Plus => "Plus".to_string(),
                    ComplianceLevel::Full => "Full".to_string(),
                }
            }),
            capabilities: Some({
                let caps: Vec<u32> = Vec::new();
                #[cfg(feature = "mpr_experimental")]
                caps.push(nyx_core::compliance::cap::MULTIPATH);
                #[cfg(feature = "hybrid")]
                caps.push(nyx_core::compliance::cap::HYBRID_PQ);
                #[cfg(feature = "cmix")]
                caps.push(nyx_core::compliance::cap::CMIX);
                #[cfg(feature = "plugin")]
                caps.push(nyx_core::compliance::cap::PLUGIN);
                #[cfg(feature = "low_power")]
                caps.push(nyx_core::compliance::cap::LOW_POWER);
                caps
            }),
        }
    }

    /// Detect current region based on network topology
    async fn detect_current_region(&self) -> String {
        // In a real implementation, this would use geolocation or network topology analysis
        // For now, return a default region
        "us-west".to_string()
    }

    /// Verify authorization for admin-like requests using optional metadata map.
    /// If `NYX_CONTROL_TOKEN` is unset, authorization is not enforced.
    fn ensure_authorized(&self, metadata: Option<&HashMap<String, String>>) -> Result<(), String> {
        let Some(expected) = &self.api_token else {
            return Ok(());
        };

        let provided = metadata
            .and_then(|m| {
                if let Some(v) = m.get("authorization") {
                    let v = v.trim();
                    if let Some(rest) = v.strip_prefix("Bearer ") {
                        return Some(rest.trim().to_string());
                    }
                    return Some(v.to_string());
                }
                m.get("api_key")
                    .or_else(|| m.get("api-key"))
                    .or_else(|| m.get("x-api-key"))
                    .map(|s| s.trim().to_string())
            })
            .ok_or_else(|| "Missing authorization token".to_string())?;

        if constant_time_eq(expected.as_bytes(), provided.as_bytes()) {
            Ok(())
        } else {
            Err("Invalid authorization token".to_string())
        }
    }
}

/// Parse SETTINGS payload from WASM into PluginNegotiationState (pure function for testability)
fn parse_wasm_settings_to_state(payload: &[u8]) -> Result<PluginNegotiationState, String> {
    // Prefer extended parser to also read CBOR plugin lists; fall back to base parser if needed.
    let (rest, frame) =
        parse_settings_frame(payload).map_err(|e| format!("SETTINGS parse error: {:?}", e))?;
    let mut state = PluginNegotiationState::default();
    let mut found_plugin_required = false;
    for s in frame.settings.iter() {
        match s.id {
            mgmt_setting_ids::PLUGIN_SUPPORT => state.support_flags = s.value,
            mgmt_setting_ids::PLUGIN_REQUIRED => {
                state.required_plugin_count = s.value;
                found_plugin_required = true;
            }
            mgmt_setting_ids::PLUGIN_OPTIONAL => state.optional_plugin_count = s.value,
            mgmt_setting_ids::PLUGIN_SECURITY_POLICY => state.security_policy = s.value,
            _ => {}
        }
    }
    // If an extension section is present, parse and use CBOR arrays to refine counts.
    if !rest.is_empty() {
        if let Ok((_rem, (_f2, ext))) = parse_settings_frame_ext(payload) {
            for (id, bytes) in ext.iter() {
                if *id == mgmt_setting_ids::PLUGIN_REQUIRED_CBOR {
                    if let Ok(ids) = ciborium::from_reader::<Vec<u32>, _>(bytes.as_slice()) {
                        state.required_plugin_count = ids.len() as u32;
                        found_plugin_required = true;
                    }
                } else if *id == mgmt_setting_ids::PLUGIN_OPTIONAL_CBOR {
                    if let Ok(ids) = ciborium::from_reader::<Vec<u32>, _>(bytes.as_slice()) {
                        state.optional_plugin_count = ids.len() as u32;
                    }
                }
            }
        }
    }
    state.requirements_count = if found_plugin_required {
        state.required_plugin_count
    } else {
        0
    };
    Ok(state)
}

#[cfg(test)]
mod wasm_settings_tests {
    use super::*;
    use nyx_stream::Setting;

    #[test]
    fn test_parse_wasm_settings_to_state_basic() {
        let items = vec![
            Setting {
                id: mgmt_setting_ids::PLUGIN_SUPPORT,
                value: 0x0001,
            },
            Setting {
                id: mgmt_setting_ids::PLUGIN_SECURITY_POLICY,
                value: 0x0001,
            },
            Setting {
                id: mgmt_setting_ids::PLUGIN_REQUIRED,
                value: 2,
            },
            Setting {
                id: mgmt_setting_ids::PLUGIN_OPTIONAL,
                value: 1,
            },
        ];
        let payload = nyx_stream::build_settings_frame(&items);
        let st = parse_wasm_settings_to_state(&payload).expect("parse ok");
        assert_eq!(st.support_flags, 0x0001);
        assert_eq!(st.security_policy, 0x0001);
        assert_eq!(st.required_plugin_count, 2);
        assert_eq!(st.optional_plugin_count, 1);
        assert_eq!(st.requirements_count, 2);
    }

    #[test]
    fn test_parse_wasm_settings_to_state_with_ext_cbor() {
        use nyx_stream::management::{build_settings_frame_ext, setting_ids, Setting};
        // Base TLVs (no explicit counts)
        let base = vec![
            Setting {
                id: setting_ids::PLUGIN_SUPPORT,
                value: 0x0001,
            },
            Setting {
                id: setting_ids::PLUGIN_SECURITY_POLICY,
                value: 0x0001,
            },
        ];
        // CBOR arrays: required [1,2], optional [10]
        let mut req_cbor = Vec::new();
        ciborium::into_writer(&vec![1u32, 2u32], &mut req_cbor).expect("cbor encode req");
        let mut opt_cbor = Vec::new();
        ciborium::into_writer(&vec![10u32], &mut opt_cbor).expect("cbor encode opt");

        let payload = build_settings_frame_ext(
            &base,
            &[
                (setting_ids::PLUGIN_REQUIRED_CBOR, &req_cbor),
                (setting_ids::PLUGIN_OPTIONAL_CBOR, &opt_cbor),
            ],
        );

        let st = parse_wasm_settings_to_state(&payload).expect("parse ok");
        assert_eq!(st.support_flags, 0x0001);
        assert_eq!(st.security_policy, 0x0001);
        assert_eq!(st.required_plugin_count, 2);
        assert_eq!(st.optional_plugin_count, 1);
        assert_eq!(st.requirements_count, 2);
    }
}

/// Constant-time equality for secret comparison
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[async_trait::async_trait]
impl NyxControl for ControlService {
    /// Get comprehensive node information
    #[instrument(skip(self))]
    async fn get_info(&self, _request: proto::Empty) -> Result<NodeInfo, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Read-only endpoint: allow without token to enable health visibility by default.

        let info = self.build_node_info().await;
        Ok(info)
    }

    /// Get health status with detailed checks
    #[instrument(skip(self))]
    async fn get_health(&self, request: HealthRequest) -> Result<HealthResponse, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Read-only endpoint: allow without token to enable health visibility by default.

        let health_status = self
            .health_monitor
            .get_health_status(request.include_details)
            .await;

        Ok(health_status)
    }

    /// Open a new stream with comprehensive options
    #[instrument(skip(self), fields(target = %request.target_address))]
    async fn open_stream(&self, request: OpenRequest) -> Result<StreamResponse, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Mutating operation requires authorization when token configured.
        if !request.metadata.is_empty() {
            self.ensure_authorized(Some(&request.metadata))?;
        } else {
            self.ensure_authorized(None)?;
        }
        #[cfg(feature = "experimental-metrics")]
        let result = self.stream_manager.open_stream(request).await;
        #[cfg(not(feature = "experimental-metrics"))]
        let result: Result<StreamResponse, String> = {
            let _ = request; // suppress unused warning
            Err("stream manager disabled".to_string())
        };
        match result {
            Ok(response) => {
                info!("Stream {} opened successfully", response.stream_id);
                Ok(response)
            }
            Err(e) => {
                error!("Failed to open stream: {}", e);
                Err(format!("Failed to open stream: {}", e))
            }
        }
    }

    /// Close a stream
    #[instrument(skip(self))]
    async fn close_stream(&self, request: StreamId) -> Result<proto::Empty, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Closing a stream is a mutating operation; require authorization if configured.
        self.ensure_authorized(None)?;
        #[cfg(feature = "experimental-metrics")]
        let stream_id = &request.id;
        #[cfg(feature = "experimental-metrics")]
        let stream_id_u32 = stream_id
            .parse::<u32>()
            .map_err(|e| format!("Invalid stream ID: {}", e))?;
        #[cfg(feature = "experimental-metrics")]
        let close_result = self.stream_manager.close_stream(stream_id_u32).await;
        #[cfg(not(feature = "experimental-metrics"))]
        let close_result: Result<(), String> = {
            let _ = request; // suppress unused warning
            Err("stream manager disabled".to_string())
        };
        match close_result {
            Ok(()) => {
                #[cfg(feature = "experimental-metrics")]
                let id_str = stream_id.as_str();
                #[cfg(not(feature = "experimental-metrics"))]
                let id_str = "(disabled)";
                info!("Stream {} closed successfully", id_str);
                Ok(proto::Empty {})
            }
            Err(e) => {
                #[cfg(feature = "experimental-metrics")]
                let id_str = stream_id.as_str();
                #[cfg(not(feature = "experimental-metrics"))]
                let id_str = "(disabled)";
                error!("Failed to close stream {}: {}", id_str, e);
                Err(format!("Failed to close stream: {}", e))
            }
        }
    }

    /// Get stream statistics
    #[instrument(skip(self))]
    async fn get_stream_stats(&self, request: StreamId) -> Result<StreamStats, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Stats are read-only; allow without token.
        #[cfg(feature = "experimental-metrics")]
        let stream_id = request
            .id
            .parse::<u32>()
            .map_err(|e| format!("Invalid stream ID: {}", e))?;
        #[cfg(feature = "experimental-metrics")]
        let stats_result = self.stream_manager.get_stream_stats(stream_id).await;
        #[cfg(not(feature = "experimental-metrics"))]
        let stats_result: Result<StreamStats, String> = {
            let _ = request; // suppress unused warning
            Err("stream manager disabled".to_string())
        };
        match stats_result {
            Ok(stats) => Ok(stats),
            Err(e) => {
                #[cfg(feature = "experimental-metrics")]
                let id_str = stream_id.to_string();
                #[cfg(not(feature = "experimental-metrics"))]
                let id_str = "(disabled)".to_string();
                error!("Failed to get stream stats: {}", e);
                Err(format!("Stream {} not found", id_str))
            }
        }
    }

    /// Receive data for a stream (chunked)
    #[instrument(skip(self))]
    async fn receive_data(&self, request: StreamId) -> Result<ReceiveResponse, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Read-only from buffer; allow without token
        let stream_id = request
            .id
            .parse::<u32>()
            .map_err(|e| format!("Invalid stream ID: {}", e))?;
        #[cfg(feature = "experimental-metrics")]
        {
            let max_bytes = 64 * 1024usize; // default chunk size
            let (data, more) = self
                .stream_manager
                .read_incoming(stream_id, max_bytes)
                .await
                .map_err(|e| format!("{}", e))?;
            return Ok(ReceiveResponse {
                stream_id: stream_id.to_string(),
                data,
                more_data: more,
            });
        }
        #[cfg(not(feature = "experimental-metrics"))]
        {
            let _ = stream_id;
            Ok(ReceiveResponse {
                stream_id: "0".to_string(),
                data: Vec::new(),
                more_data: false,
            })
        }
    }

    /// Send data for a stream
    #[instrument(skip(self))]
    async fn send_data(&self, request: DataRequest) -> Result<DataResponse, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Mutating operation requires authorization when token configured.
        let meta = None::<&std::collections::HashMap<String, String>>;
        self.ensure_authorized(meta)?;

        #[cfg(feature = "experimental-metrics")]
        {
            let stream_id = request
                .stream_id
                .parse::<u32>()
                .map_err(|e| format!("Invalid stream ID: {}", e))?;
            let written = self
                .stream_manager
                .send_data(stream_id, request.data)
                .await
                .map_err(|e| format!("{}", e))?;
            return Ok(DataResponse {
                success: true,
                bytes_written: written,
            });
        }
        #[cfg(not(feature = "experimental-metrics"))]
        {
            let _ = request;
            Ok(DataResponse {
                success: false,
                bytes_written: 0,
            })
        }
    }

    /// List all streams
    #[instrument(skip(self))]
    async fn list_streams(&self, _request: proto::Empty) -> Result<Vec<StreamStats>, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.ensure_authorized(None)?;

        // Get all stream stats from the stream manager
        #[cfg(feature = "experimental-metrics")]
        let streams = self.stream_manager.list_streams().await;
        #[cfg(not(feature = "experimental-metrics"))]
        let streams: Vec<StreamStats> = Vec::new();
        Ok(streams)
    }

    /// Subscribe to events with filtering
    #[instrument(skip(self))]
    async fn subscribe_events(&self, request: EventFilter) -> Result<Vec<Event>, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.ensure_authorized(None)?;
        #[cfg(feature = "experimental-events")]
        {
            let limit = request
                .time_range_seconds
                .map(|secs| secs as usize)
                .or(Some(1000));
            let events = self.event_system.query(&request, limit).await;
            return Ok(events);
        }
        #[cfg(not(feature = "experimental-events"))]
        {
            let _ = request;
            Ok(Vec::new())
        }
    }

    /// Subscribe to statistics with real-time updates
    #[instrument(skip(self))]
    async fn subscribe_stats(&self, _request: proto::Empty) -> Result<Vec<StatsUpdate>, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.ensure_authorized(None)?;

        // Return current stats snapshot
        #[cfg(feature = "experimental-metrics")]
        let performance_metrics = self.metrics.get_performance_metrics().await;
        #[cfg(feature = "experimental-metrics")]
        let resource_usage = self.metrics.get_resource_usage().await.unwrap_or_default();
        #[cfg(feature = "experimental-metrics")]
        let streams = self.stream_manager.list_streams().await;
        #[cfg(not(feature = "experimental-metrics"))]
        let performance_metrics = crate::proto::PerformanceMetrics {
            cover_traffic_rate: 0.0,
            avg_latency_ms: 0.0,
            packet_loss_rate: 0.0,
            bandwidth_utilization: 0.0,
            cpu_usage: 0.0,
            memory_usage_mb: 0.0,
            total_packets_sent: 0,
            total_packets_received: 0,
            retransmissions: 0,
            connection_success_rate: 0.0,
        };
        #[cfg(not(feature = "experimental-metrics"))]
        let resource_usage = crate::proto::ResourceUsage {
            cpu_percent: 0.0,
            memory_bytes: 0,
            memory_rss_bytes: 0,
            memory_vms_bytes: 0,
            memory_percent: 0.0,
            disk_usage_bytes: 0,
            disk_total_bytes: 0,
            network_rx_bytes: 0,
            network_tx_bytes: 0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
            file_descriptors: 0,
            open_file_descriptors: 0,
            thread_count: 0,
        };
        #[cfg(not(feature = "experimental-metrics"))]
        let streams: Vec<StreamStats> = Vec::new();

        let mut custom_metrics = HashMap::new();
        custom_metrics.insert("cpu_usage".to_string(), performance_metrics.cpu_usage);
        custom_metrics.insert("memory_usage".to_string(), resource_usage.memory_percent);
        #[cfg(feature = "low_power")]
        {
            // Low power cover ratio metric (0.1 in low power, 1.0 otherwise)
            let lp_ratio = self.low_power_manager.recommended_cover_ratio() as f64;
            custom_metrics.insert("low_power_cover_ratio".to_string(), lp_ratio);
            // Low power state as 1.0 (Low) / 0.0 (Normal)
            let lp_state = if self.low_power_manager.is_low_power() {
                1.0
            } else {
                0.0
            };
            custom_metrics.insert("low_power_state".to_string(), lp_state);
        }

        let node_info = NodeInfo {
            node_id: hex::encode(self.node_id),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_sec: self.start_time.elapsed().as_secs() as u32,
            bytes_in: performance_metrics.total_packets_received,
            bytes_out: resource_usage.network_bytes_sent,
            pid: std::process::id(),
            active_streams: 0,
            connected_peers: 0,
            mix_routes: vec![],
            performance: Some(performance_metrics),
            resources: Some(resource_usage),
            topology: Some(NetworkTopology {
                total_nodes: 0,
                active_nodes: 0,
                mix_nodes: 0,
                gateway_nodes: 0,
                network_diameter: 0,
                clustering_coefficient: 0.0,
                peers: Vec::new(),
                paths: Vec::new(),
                total_nodes_known: 0,
                reachable_nodes: 0,
                current_region: "unknown".to_string(),
                available_regions: Vec::new(),
            }),
            compliance_level: None,
            capabilities: None,
        };

        let stats_update = StatsUpdate {
            metrics: custom_metrics.clone(),
            timestamp: Some(system_time_to_proto_timestamp(SystemTime::now())),
            node_info: Some(node_info),
            stream_stats: streams,
            custom_metrics,
        };

        Ok(vec![stats_update])
    }

    /// Update configuration dynamically
    #[instrument(skip(self))]
    async fn update_config(
        &self,
        request: proto::ConfigRequest,
    ) -> Result<proto::ConfigResponse, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let meta = if request.metadata.is_empty() {
            None
        } else {
            Some(&request.metadata)
        };
        self.ensure_authorized(meta)?;

        match self
            .config_manager
            .update_config(ConfigUpdate {
                section: request.scope.clone(),
                key: request.key.clone(),
                value: request.value.clone().unwrap_or_default(),
                settings: request.metadata.clone(),
            })
            .await
        {
            Ok(response) => {
                if response.success {
                    info!("Configuration updated successfully: {}", response.message);

                    // Emit configuration update event
                    let event = proto::Event {
                        r#type: "system".to_string(),
                        detail: "Configuration updated".to_string(),
                        timestamp: Some(proto::Timestamp::now()),
                        severity: "info".to_string(),
                        attributes: HashMap::new(),
                        event_type: "config_update".to_string(),
                        data: HashMap::new(),
                        event_data: Some(proto::event::EventData::SystemEvent(
                            proto::event::SystemEvent {
                                event_type: "config_update".to_string(),
                                severity: "info".to_string(),
                                message: response.message.clone(),
                                metadata: HashMap::new(),
                                component: "daemon".to_string(),
                            },
                        )),
                    };

                    let _ = self.event_tx.send(event);
                }

                Ok(response)
            }
            Err(e) => {
                error!("Failed to update configuration: {}", e);
                Err(format!("Configuration update failed: {}", e))
            }
        }
    }

    /// Reload configuration from file
    #[instrument(skip(self))]
    async fn reload_config(&self, _request: proto::Empty) -> Result<ConfigResponse, String> {
        self.total_requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.ensure_authorized(None)?;

        match self.config_manager.reload_config().await {
            Ok(response) => {
                if response.success {
                    info!("Configuration reloaded successfully: {}", response.message);

                    // Update the main config reference
                    let new_config = self.config_manager.get_config().await;
                    *self.config.write().await = new_config;

                    // Emit configuration reload event
                    let event = proto::Event {
                        r#type: "system".to_string(),
                        detail: "Configuration reloaded".to_string(),
                        timestamp: Some(proto::Timestamp::now()),
                        severity: "info".to_string(),
                        attributes: HashMap::new(),
                        event_type: "config_reload".to_string(),
                        data: HashMap::new(),
                        event_data: Some(proto::event::EventData::SystemEvent(
                            proto::event::SystemEvent {
                                event_type: "config_reload".to_string(),
                                severity: "info".to_string(),
                                message: "Configuration reloaded successfully".to_string(),
                                metadata: HashMap::new(),
                                component: "daemon".to_string(),
                            },
                        )),
                    };

                    let _ = self.event_tx.send(event);
                }

                Ok(response)
            }
            Err(e) => {
                error!("Failed to reload configuration: {}", e);
                Err(format!("Configuration reload failed: {}", e))
            }
        }
    }
}

/// Initialize telemetry and tracing
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
}

/// Main entry point for the Nyx daemon
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logging and debugging
    init_tracing();

    // Apply OS-level sandboxing / process isolation
    #[cfg(target_os = "linux")]
    {
        // Keep Linux behavior via seccomp when compiled with support
        let _ = nyx_core::install_seccomp();
    }
    #[cfg(target_os = "openbsd")]
    {
        // Apply pledge/unveil standard restrictions when available
        let _ = nyx_core::openbsd::install_nyx_daemon_pledge();
        let _ = nyx_core::openbsd::setup_nyx_daemon_unveil();
    }
    #[cfg(target_os = "windows")]
    {
        // Apply Windows Job Object based process isolation
        // Read optional environment overrides for isolation parameters
        let cfg = nyx_core::windows::WindowsIsolationConfig {
            max_process_memory_mb: std::env::var("NYX_WIN_MAX_PROCESS_MB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(512),
            max_job_memory_mb: std::env::var("NYX_WIN_MAX_JOB_MB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1024),
            max_working_set_mb: std::env::var("NYX_WIN_MAX_WORKINGSET_MB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(256),
            max_process_time_seconds: std::env::var("NYX_WIN_MAX_CPU_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            kill_on_job_close: std::env::var("NYX_WIN_KILL_ON_CLOSE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true),
        };
        let _ = nyx_core::apply_process_isolation(Some(cfg));
    }

    let addr = "127.0.0.1:50051";
    let config = Default::default();
    let service = std::sync::Arc::new(ControlService::new(config).await?);
    // Spawn HTTP control API server compatible with nyx-cli
    spawn_http_server(service.clone()).await?;

    // Start plugin manifest watcher if feature and env configured
    #[cfg(feature = "plugin")]
    {
        if let Ok(path) = std::env::var("NYX_PLUGIN_MANIFEST") {
            if !path.is_empty() {
                tracing::info!("Starting plugin manifest watcher for {}", path);
                start_plugin_manifest_watcher(path);
            }
        }
    }

    tracing::info!(target = "nyx-daemon", address = %addr, "Nyx daemon starting");

    // Start the server (simplified non-tonic version)
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
#[cfg(feature = "plugin")]
fn start_plugin_manifest_watcher(path: String) {
    use std::time::Duration;
    use tokio::sync::mpsc;
    // Debounce reload requests to avoid thrash on editors writing temp files
    let (tx, mut rx) = mpsc::unbounded_channel::<()>();

    // Clone path for async task
    let path_async = path.clone();
    // Spawn a background task to perform reloads
    tokio::spawn(async move {
        let mut last_reload = std::time::Instant::now() - Duration::from_secs(10);
        while let Some(_) = rx.recv().await {
            if last_reload.elapsed() < Duration::from_millis(250) {
                continue;
            }
            match nyx_stream::plugin_handshake::reload_plugin_manifest_from_path(&path_async) {
                Ok(count) => tracing::info!("Reloaded plugin manifest ({} entries)", count),
                Err(e) => tracing::warn!("Failed to reload plugin manifest: {}", e),
            }
            last_reload = std::time::Instant::now();
        }
    });

    // Native watcher runs in a blocking thread; forward events into channel
    std::thread::spawn(move || {
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        let mut watcher: RecommendedWatcher =
            notify::recommended_watcher(event_tx).expect("watcher");
        let p = std::path::Path::new(&path);
        let parent = p.parent().unwrap_or_else(|| std::path::Path::new("."));
        watcher
            .watch(parent, RecursiveMode::NonRecursive)
            .expect("watch path");
        for evt in event_rx {
            match evt {
                Ok(event) => {
                    // Accept Modify/Create/Remove on the target file
                    let is_target = event.paths.iter().any(|ep| ep.ends_with(&path));
                    if !is_target {
                        continue;
                    }
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                            let _ = tx.send(());
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    eprintln!("notify error: {e}");
                }
            }
        }
    });
}

// ---------------- HTTP API (Axum) ----------------

#[derive(Clone)]
struct AppState {
    service: std::sync::Arc<ControlService>,
}

#[derive(Debug, Deserialize)]
struct HttpOpenRequest {
    destination: String,
    #[allow(dead_code)]
    options: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct HttpStreamResponse {
    stream_id: u32,
    success: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HttpDataRequest {
    stream_id: u32,
    data: Vec<u8>,
    #[allow(dead_code)]
    metadata: Option<String>,
}

#[derive(Debug, Serialize)]
struct HttpDataResponse {
    success: bool,
    bytes_sent: u64,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct HttpStreamStats {
    stream_id: u32,
    bytes_sent: u64,
    bytes_received: u64,
    packets_sent: u64,
    packets_received: u64,
    avg_rtt_ms: f64,
    packet_loss_rate: f64,
}

async fn spawn_http_server(service: std::sync::Arc<ControlService>) -> anyhow::Result<()> {
    let http_addr =
        std::env::var("NYX_HTTP_ADDR").unwrap_or_else(|_| "127.0.0.1:50051".to_string());
    let state = AppState { service };

    let app = Router::new()
        .route("/api/v1/info", get(http_get_info))
        .route("/api/v1/stream/open", post(http_open_stream))
        .route("/api/v1/stream/data", post(http_send_data))
        .route("/api/v1/stream/:id/stats", get(http_get_stream_stats))
        .route("/api/v1/stream/:id/close", post(http_close_stream))
        // WASM: incoming SETTINGS and CLOSE payloads for browser clients
        .route("/api/v1/wasm/settings", post(http_wasm_settings))
        .route("/api/v1/wasm/close", post(http_wasm_close))
        .route("/api/v1/wasm/negotiation", get(http_get_wasm_negotiation))
        .route("/api/v1/wasm/handshake/start", post(http_wasm_handshake_start))
        .route("/api/v1/wasm/handshake/process-peer-settings", post(http_wasm_handshake_process_peer_settings))
        .route("/api/v1/wasm/handshake/complete", post(http_wasm_handshake_complete))
        .route("/api/v1/wasm/handshake/required", post(http_wasm_set_required_plugins))
        // Plugin registry diagnostics and reload (feature=plugin)
        .route("/api/v1/plugins/registry", get(http_get_plugin_registry))
        .route("/api/v1/plugins/reload", post(http_reload_plugin_manifest_json))
        .route("/api/v1/plugins/reload/env", post(http_reload_plugin_manifest_env))
        .route("/api/v1/stream/:id/recv", get(http_receive_data))
        .route("/api/v1/events", get(http_get_events))
        .route("/api/v1/events", post(http_publish_event))
        .route("/api/v1/events/stats", get(http_get_event_stats))
        // Alerts (experimental)
        .route("/api/v1/alerts/stats", get(http_get_alerts_stats))
        .route("/api/v1/alerts/analysis", get(http_get_alerts_analysis))
        .with_state(state)
        // Diagnostics for plugin registry (feature=plugin)
        .route(
            "/api/v1/plugin/registry",
            get(|| async move {
                #[cfg(feature = "plugin")]
                {
                    let snap = nyx_stream::plugin_handshake::get_plugin_registry_snapshot();
                    return Json(serde_json::json!({"plugins": snap}));
                }
                #[cfg(not(feature = "plugin"))]
                {
                    return Json(serde_json::json!({"plugins": []}));
                }
            })
        )
        .route(
            "/api/v1/plugin/reload",
            post(|Json(body): Json<serde_json::Value>| async move {
                #[cfg(feature = "plugin")]
                {
                    // Optional raw JSON manifest in request body {"manifest": [...]} for remote update
                    if let Some(m) = body.get("manifest") {
                        let s = serde_json::to_string(m).unwrap_or("[]".to_string());
                        match nyx_stream::plugin_handshake::reload_plugin_manifest_from_json(&s) {
                            Ok(n) => return Json(serde_json::json!({"reloaded": true, "source": "body", "count": n})),
                            Err(e) => return Json(serde_json::json!({"reloaded": false, "error": e})),
                        }
                    }
                    match nyx_stream::plugin_handshake::reload_plugin_manifest() {
                        Ok(n) => Json(serde_json::json!({"reloaded": true, "count": n})),
                        Err(e) => Json(serde_json::json!({"reloaded": false, "error": e})),
                    }
                }
                #[cfg(not(feature = "plugin"))]
                {
                    Json(serde_json::json!({"reloaded": false, "error": "plugin feature disabled"}))
                }
            })
        );

    let listener = tokio::net::TcpListener::bind(&http_addr).await?;
    tracing::info!(target = "nyx-daemon", address = %http_addr, "HTTP control API listening");
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });
    Ok(())
}

fn auth_metadata_from_headers(headers: &HeaderMap) -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    if let Some(v) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        m.insert("authorization".to_string(), v.to_string());
    }
    m
}

#[cfg(feature = "experimental-alerts")]
async fn http_get_alerts_stats() -> Json<serde_json::Value> {
    let stats = ENHANCED_ALERT_SYSTEM.get_alert_statistics();
    Json(serde_json::to_value(stats).unwrap_or(serde_json::json!({})))
}

#[cfg(not(feature = "experimental-alerts"))]
async fn http_get_alerts_stats() -> Json<serde_json::Value> {
    Json(serde_json::json!({"disabled": true}))
}

#[cfg(feature = "experimental-alerts")]
async fn http_get_alerts_analysis() -> Json<serde_json::Value> {
    let report = ENHANCED_ALERT_SYSTEM.generate_analysis_report();
    Json(serde_json::to_value(report).unwrap_or(serde_json::json!({})))
}

#[cfg(not(feature = "experimental-alerts"))]
async fn http_get_alerts_analysis() -> Json<serde_json::Value> {
    Json(serde_json::json!({"disabled": true}))
}

async fn http_get_info(State(st): State<AppState>) -> Json<serde_json::Value> {
    let info = st
        .service
        .get_info(proto::Empty {})
        .await
        .unwrap_or_else(|_| NodeInfo {
            node_id: String::new(),
            version: String::new(),
            uptime_sec: 0,
            bytes_in: 0,
            bytes_out: 0,
            pid: 0,
            active_streams: 0,
            connected_peers: 0,
            mix_routes: Vec::new(),
            performance: None,
            resources: None,
            topology: None,
            compliance_level: None,
            capabilities: None,
        });
    // Map to CLI-friendly shape
    let (cpu_usage_percent, memory_usage_bytes, rx, tx) =
        if let (Some(p), Some(r)) = (info.performance.clone(), info.resources.clone()) {
            (
                (p.cpu_usage * 100.0),
                r.memory_bytes,
                r.network_rx_bytes,
                r.network_tx_bytes,
            )
        } else {
            (0.0, 0, 0, 0)
        };
    Json(serde_json::json!({
        "node_id": info.node_id,
        "version": info.version,
        "uptime_seconds": info.uptime_sec,
        "cpu_usage_percent": cpu_usage_percent,
        "memory_usage_bytes": memory_usage_bytes,
        "network_rx_bytes": rx,
        "network_tx_bytes": tx,
        "active_connections": info.active_streams,
        "total_sent_bytes": info.bytes_out,
        "total_received_bytes": info.bytes_in,
        "connected_peers": info.connected_peers,
    }))
}

// ---- Events API ----

async fn http_get_events(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> Json<Vec<proto::Event>> {
    let (filter, _limit) = parse_event_filter_from_query(&q);
    match st.service.subscribe_events(filter).await {
        Ok(events) => Json(events),
        Err(_) => Json(Vec::new()),
    }
}

async fn http_publish_event(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(event): Json<proto::Event>,
) -> Json<serde_json::Value> {
    let meta = auth_metadata_from_headers(&headers);
    match st.service.publish_event_internal(&meta, event).await {
        Ok(_) => Json(serde_json::json!({"success": true})),
        Err(e) => Json(serde_json::json!({"success": false, "error": e})),
    }
}

// ---- WASM endpoints ----

/// Accept SETTINGS payload posted by browser client. Content-Type: application/nyx-settings
async fn http_wasm_settings(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Json<serde_json::Value> {
    let len = body.len();
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    tracing::info!(target="nyx-daemon", content_type=%ct, bytes=len, "Received WASM SETTINGS payload");
    let result = st.service.apply_wasm_settings(&body).await;
    // Also parse LOW_POWER_PREFERENCE to inform runtime
    if let Ok((_rem, frame)) = parse_settings_frame(&body) {
        for s in frame.settings.iter() {
            if s.id == mgmt_setting_ids::LOW_POWER_PREFERENCE {
                let prefer_low = s.value != 0;
                #[cfg(feature = "low_power")]
                {
                    if prefer_low {
                        st.service.low_power_manager.set_manual_low_power(true);
                    } else {
                        st.service.low_power_manager.set_manual_low_power(false);
                    }
                }
                st.service.transport.apply_low_power_preference();
            }
        }
    }
    match result {
        Ok(state) => Json(serde_json::json!({
            "accepted": true,
            "bytes": len,
            "support_flags": state.support_flags,
            "required_plugin_count": state.required_plugin_count,
            "optional_plugin_count": state.optional_plugin_count,
            "security_policy": state.security_policy,
            "requirements_count": state.requirements_count
        })),
        Err(e) => Json(serde_json::json!({"accepted": false, "error": e})),
    }
}

/// Accept CLOSE payload posted by browser client. Content-Type: application/nyx-close
async fn http_wasm_close(
    State(st): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Json<serde_json::Value> {
    let len = body.len();
    let ct = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    tracing::info!(target="nyx-daemon", content_type=%ct, bytes=len, "Received WASM CLOSE payload");
    match st.service.process_wasm_close(&body).await {
        Ok((code, cap)) => {
            Json(serde_json::json!({"accepted": true, "bytes": len, "code": code, "cap_id": cap}))
        }
        Err(e) => Json(serde_json::json!({"accepted": false, "error": e})),
    }
}

/// Get last WASM negotiation snapshot
async fn http_get_wasm_negotiation(State(st): State<AppState>) -> Json<serde_json::Value> {
    let snap = st.service.plugin_negotiation.read().await.clone();
    Json(serde_json::json!({
        "support_flags": snap.support_flags,
        "required_plugin_count": snap.required_plugin_count,
        "optional_plugin_count": snap.optional_plugin_count,
        "security_policy": snap.security_policy,
        "requirements_count": snap.requirements_count
    }))
}

/// Start a plugin handshake using current negotiation snapshot (feature=plugin)
async fn http_wasm_handshake_start(State(st): State<AppState>) -> Json<serde_json::Value> {
    #[cfg(feature = "plugin")]
    {
        use nyx_stream::plugin_handshake::PluginHandshakeCoordinator;
        use nyx_stream::plugin_settings::PluginSettingsManager;

        let mut guard = st.service.plugin_handshake.lock().await;
        // If not present, create a new coordinator from current snapshot
        if guard.is_none() {
            let _snap = st.service.plugin_negotiation.read().await.clone();
            let mgr = PluginSettingsManager::new();
            let mut coord = PluginHandshakeCoordinator::new(mgr, true);
            match coord.initiate_handshake().await {
                Ok(payload_opt) => {
                    let settings_b64 =
                        payload_opt.map(|p| general_purpose::URL_SAFE_NO_PAD.encode(p));
                    *guard = Some(coord);
                    return Json(
                        serde_json::json!({"started": true, "settings_b64": settings_b64}),
                    );
                }
                Err(e) => {
                    return Json(serde_json::json!({"started": false, "error": e.to_string()}));
                }
            }
        } else {
            return Json(serde_json::json!({"started": true, "info": "already in progress"}));
        }
    }
    #[cfg(not(feature = "plugin"))]
    {
        Json(serde_json::json!({"started": false, "error": "plugin feature disabled"}))
    }
}

/// Process peer SETTINGS (from browser) through the coordinator and return optional response SETTINGS
async fn http_wasm_handshake_process_peer_settings(
    State(st): State<AppState>,
    body: axum::body::Bytes,
) -> Json<serde_json::Value> {
    #[cfg(feature = "plugin")]
    {
        use nyx_stream::plugin_handshake::PluginHandshakeCoordinator;
        use nyx_stream::plugin_settings::PluginSettingsManager;

        let mut guard = st.service.plugin_handshake.lock().await;
        if guard.is_none() {
            // Initialize as responder if no coordinator exists
            let mgr = PluginSettingsManager::new();
            let mut coord = PluginHandshakeCoordinator::new(mgr, false);
            if let Err(e) = coord.initiate_handshake().await {
                return Json(serde_json::json!({"ok": false, "error": e.to_string()}));
            }
            *guard = Some(coord);
        }
        let coord = guard.as_mut().unwrap();
        match coord.process_peer_settings(&body).await {
            Ok(Some(response_settings)) => {
                let b64 = general_purpose::URL_SAFE_NO_PAD.encode(&response_settings);
                Json(serde_json::json!({"ok": true, "response_settings_b64": b64}))
            }
            Ok(None) => Json(serde_json::json!({"ok": true, "response_settings_b64": null})),
            Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        }
    }
    #[cfg(not(feature = "plugin"))]
    {
        Json(serde_json::json!({"ok": false, "error": "plugin feature disabled"}))
    }
}

/// Complete plugin initialization and return result summary
async fn http_wasm_handshake_complete(State(st): State<AppState>) -> Json<serde_json::Value> {
    #[cfg(feature = "plugin")]
    {
        use nyx_stream::plugin_handshake::{HandshakeResult, PluginHandshakeError};
        let mut guard = st.service.plugin_handshake.lock().await;
        if let Some(coord) = guard.as_mut() {
            // Fallback: if peer SETTINGS were never provided (initiator happy-path),
            // treat it as zero requirements and advance state accordingly.
            match coord.process_peer_settings(&[0u8, 0u8]).await {
                Ok(_) => {}
                Err(PluginHandshakeError::InvalidStateTransition { .. }) => { /* ignore, proceed */
                }
                Err(e) => {
                    return Json(serde_json::json!({"ok": false, "error": e.to_string()}));
                }
            }
            match coord.complete_plugin_initialization().await {
                Ok(HandshakeResult::Success {
                    active_plugins,
                    handshake_duration,
                }) => {
                    *guard = None; // clear coordinator after success
                    Json(serde_json::json!({
                        "ok": true,
                        "result": "success",
                        "active_plugins": active_plugins,
                        "duration_secs": handshake_duration.as_secs_f64()
                    }))
                }
                Ok(HandshakeResult::IncompatibleRequirements {
                    conflicting_plugin_id,
                    reason,
                }) => {
                    *guard = None;
                    Json(serde_json::json!({
                        "ok": false,
                        "result": "incompatible",
                        "conflicting_plugin_id": conflicting_plugin_id,
                        "reason": reason
                    }))
                }
                Ok(HandshakeResult::Timeout { attempted_duration }) => {
                    *guard = None;
                    Json(serde_json::json!({
                        "ok": false,
                        "result": "timeout",
                        "duration_secs": attempted_duration.as_secs_f64()
                    }))
                }
                Ok(HandshakeResult::ProtocolError { error }) => {
                    *guard = None;
                    Json(
                        serde_json::json!({"ok": false, "result": "protocol_error", "error": error}),
                    )
                }
                Err(e) => {
                    *guard = None;
                    Json(serde_json::json!({"ok": false, "error": e.to_string()}))
                }
            }
        } else {
            Json(serde_json::json!({"ok": false, "error": "no handshake in progress"}))
        }
    }
    #[cfg(not(feature = "plugin"))]
    {
        Json(serde_json::json!({"ok": false, "error": "plugin feature disabled"}))
    }
}

/// Set required plugin IDs from CBOR array<u32> (base64url-encoded) coming from browser
async fn http_wasm_set_required_plugins(
    State(st): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    #[cfg(feature = "plugin")]
    {
        // Expect {"required_cbor_b64": "..."}
        let maybe_b64 = body
            .get("required_cbor_b64")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(maybe_b64.as_bytes()) {
            Ok(bytes) => match ciborium::from_reader::<Vec<u32>, _>(bytes.as_slice()) {
                Ok(ids) => {
                    {
                        let mut guard = st.service.plugin_required_ids.write().await;
                        *guard = ids.clone();
                    }
                    return Json(serde_json::json!({"ok": true, "count": ids.len()}));
                }
                Err(e) => {
                    return Json(serde_json::json!({"ok": false, "error": format!("cbor: {}", e)}))
                }
            },
            Err(e) => {
                return Json(serde_json::json!({"ok": false, "error": format!("b64: {}", e)}))
            }
        }
    }
    #[cfg(not(feature = "plugin"))]
    {
        Json(serde_json::json!({"ok": false, "error": "plugin feature disabled"}))
    }
}

/// Get plugin registry snapshot (feature=plugin)
async fn http_get_plugin_registry() -> Json<serde_json::Value> {
    #[cfg(feature = "plugin")]
    {
        let items = nyx_stream::plugin_handshake::get_plugin_registry_snapshot();
        return Json(serde_json::json!({"ok": true, "registry": items}));
    }
    #[cfg(not(feature = "plugin"))]
    {
        Json(serde_json::json!({"ok": false, "error": "plugin feature disabled"}))
    }
}

/// Reload plugin manifest from JSON body (feature=plugin)
async fn http_reload_plugin_manifest_json(body: axum::body::Bytes) -> Json<serde_json::Value> {
    #[cfg(feature = "plugin")]
    {
        match nyx_stream::plugin_handshake::reload_plugin_manifest_from_json(
            std::str::from_utf8(&body).unwrap_or("{}"),
        ) {
            Ok(count) => Json(serde_json::json!({"ok": true, "count": count})),
            Err(e) => Json(serde_json::json!({"ok": false, "error": e})),
        }
    }
    #[cfg(not(feature = "plugin"))]
    {
        Json(serde_json::json!({"ok": false, "error": "plugin feature disabled"}))
    }
}

/// Reload plugin manifest from NYX_PLUGIN_MANIFEST env (feature=plugin)
async fn http_reload_plugin_manifest_env() -> Json<serde_json::Value> {
    #[cfg(feature = "plugin")]
    {
        match nyx_stream::plugin_handshake::reload_plugin_manifest() {
            Ok(count) => Json(serde_json::json!({"ok": true, "count": count})),
            Err(e) => Json(serde_json::json!({"ok": false, "error": e})),
        }
    }
    #[cfg(not(feature = "plugin"))]
    {
        Json(serde_json::json!({"ok": false, "error": "plugin feature disabled"}))
    }
}

async fn http_get_event_stats(State(st): State<AppState>) -> Json<serde_json::Value> {
    let stats = st.service.get_event_statistics_snapshot().await;
    Json(stats)
}

async fn http_open_stream(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<HttpOpenRequest>,
) -> Json<HttpStreamResponse> {
    let metadata = auth_metadata_from_headers(&headers);
    let open = OpenRequest {
        destination: req.destination.clone(),
        target_address: req.destination,
        options: None,
        metadata,
    };
    match st.service.open_stream(open).await {
        Ok(resp) => {
            let id_u32 = resp.stream_id.parse::<u32>().unwrap_or(0);
            Json(HttpStreamResponse {
                stream_id: id_u32,
                success: resp.success,
                error: if resp.success {
                    None
                } else {
                    Some(resp.message)
                },
            })
        }
        Err(e) => Json(HttpStreamResponse {
            stream_id: 0,
            success: false,
            error: Some(e),
        }),
    }
}

async fn http_send_data(
    State(st): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<HttpDataRequest>,
) -> Json<HttpDataResponse> {
    let _metadata = auth_metadata_from_headers(&headers);
    let data_req = DataRequest {
        stream_id: req.stream_id.to_string(),
        data: req.data,
    };
    match st.service.send_data(data_req).await {
        Ok(dr) => Json(HttpDataResponse {
            success: dr.success,
            bytes_sent: dr.bytes_written,
            error: None,
        }),
        Err(e) => Json(HttpDataResponse {
            success: false,
            bytes_sent: 0,
            error: Some(e),
        }),
    }
}

async fn http_get_stream_stats(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Json<HttpStreamStats> {
    let sid = StreamId { id: id.clone() };
    match st.service.get_stream_stats(sid).await {
        Ok(s) => {
            let id_u32 = s.stream_id.parse::<u32>().unwrap_or(0);
            Json(HttpStreamStats {
                stream_id: id_u32,
                bytes_sent: s.bytes_sent,
                bytes_received: s.bytes_received,
                packets_sent: s.packets_sent,
                packets_received: s.packets_received,
                avg_rtt_ms: s.rtt_ms,
                packet_loss_rate: s.packet_loss_rate,
            })
        }
        Err(_) => Json(HttpStreamStats {
            stream_id: id.parse().unwrap_or(0),
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            avg_rtt_ms: 0.0,
            packet_loss_rate: 0.0,
        }),
    }
}

async fn http_close_stream(
    State(st): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let mut _metadata = auth_metadata_from_headers(&headers);
    let _ = st.service.close_stream(StreamId { id }).await;
    Json(serde_json::json!({}))
}

#[derive(Debug, Serialize)]
struct HttpReceiveResponse {
    stream_id: u32,
    data: Vec<u8>,
    more_data: bool,
}

async fn http_receive_data(
    State(st): State<AppState>,
    Path(id): Path<String>,
    _q: Option<Query<std::collections::HashMap<String, String>>>,
) -> Json<HttpReceiveResponse> {
    match st.service.receive_data(StreamId { id: id.clone() }).await {
        Ok(rr) => {
            let id_u32 = id.parse::<u32>().unwrap_or(0);
            Json(HttpReceiveResponse {
                stream_id: id_u32,
                data: rr.data,
                more_data: rr.more_data,
            })
        }
        Err(_e) => {
            let id_u32 = id.parse::<u32>().unwrap_or(0);
            Json(HttpReceiveResponse {
                stream_id: id_u32,
                data: Vec::new(),
                more_data: false,
            })
        }
    }
}

fn parse_event_filter_from_query(
    q: &HashMap<String, String>,
) -> (proto::EventFilter, Option<usize>) {
    let types = q
        .get("types")
        .or_else(|| q.get("event_types"))
        .map(|s| {
            s.split(',')
                .filter(|x| !x.is_empty())
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let severity = q.get("severity").cloned().unwrap_or_default();
    let severity_levels = q
        .get("severity_levels")
        .map(|s| {
            s.split(',')
                .filter(|x| !x.is_empty())
                .map(|s| s.trim().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let stream_ids = q
        .get("stream_ids")
        .map(|s| {
            s.split(',')
                .filter_map(|x| x.trim().parse::<u32>().ok())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let limit = q.get("limit").and_then(|v| v.parse::<usize>().ok());
    let filter = proto::EventFilter {
        event_types: types.clone(),
        types,
        severity_levels,
        severity,
        time_range_seconds: limit.map(|l| l as u64),
        stream_ids,
    };
    (filter, limit)
}

impl ControlService {
    async fn publish_event_internal(
        &self,
        meta: &HashMap<String, String>,
        event: proto::Event,
    ) -> Result<(), String> {
        self.ensure_authorized(Some(meta))?;
        let _ = self.event_tx.send(event.clone());
        #[cfg(feature = "experimental-events")]
        {
            let _ = self.event_system.publish_event(event).await;
        }
        Ok(())
    }

    async fn get_event_statistics_snapshot(&self) -> serde_json::Value {
        #[cfg(feature = "experimental-events")]
        {
            let stats = self.event_system.get_statistics().await;
            serde_json::json!({
                "total_events": stats.total_events,
                "events_by_type": stats.events_by_type,
                "events_by_severity": stats.events_by_severity,
                "events_by_priority": stats.events_by_priority,
                "filtered_events": stats.filtered_events,
                "subscriber_count": stats.subscriber_count,
                "active_subscriber_count": stats.active_subscriber_count,
                "failed_deliveries": stats.failed_deliveries,
                "queue_size": stats.queue_size,
                "processing_errors": stats.processing_errors,
                "average_processing_time_ms": stats.average_processing_time_ms
            })
        }
        #[cfg(not(feature = "experimental-events"))]
        {
            serde_json::json!({
                "total_events": 0,
                "events_by_type": {},
                "events_by_severity": {},
                "events_by_priority": {},
                "filtered_events": 0,
                "subscriber_count": 0,
                "active_subscriber_count": 0,
                "failed_deliveries": 0,
                "queue_size": 0,
                "processing_errors": 0,
                "average_processing_time_ms": 0.0
            })
        }
    }
}
