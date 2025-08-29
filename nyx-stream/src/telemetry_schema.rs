//! Telemetry schema module providing OpenTelemetry Protocol (OTLP) compliant instrumentation
//! for the Nyx stream processing system. This module implements telemetry collection,
//! span management, and observability features for debugging and monitoring purposes.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Connection identifier for telemetry tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub u64);

impl ConnectionId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn inner(&self) -> u64 {
        self.0
    }
}

/// Telemetry configuration structure for customizing instrumentation behavior
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Enable or disable telemetry instrumentation
    pub enabled: bool,
    /// Sampling rate for telemetry data (0.0 to 1.0)
    pub sampling_rate: f64,
    /// Maximum span depth to prevent unlimited recursion
    pub max_span_depth: u32,
    /// Service name for telemetry identification
    pub service_name: String,
    /// Environment designation (dev, staging, prod)
    pub environment: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sampling_rate: 1.0,
            max_span_depth: 10,
            service_name: "nyx-stream".to_string(),
            environment: "development".to_string(),
        }
    }
}

/// Telemetry sampler for controlling data collection
#[derive(Debug, Clone)]
pub enum TelemetrySampler {
    /// Always sample all telemetry data
    AlwaysOn,
    /// Never sample telemetry data
    AlwaysOff,
    /// Sample based on trace ID hash
    TraceIdRatioBased(f64),
    /// Custom sampling logic
    Custom(fn(u64) -> bool),
}

impl TelemetrySampler {
    pub fn should_sample(&self, trace_id: u64) -> bool {
        match self {
            Self::AlwaysOn => true,
            Self::AlwaysOff => false,
            Self::TraceIdRatioBased(ratio) => {
                let hash = trace_id.wrapping_mul(2654435761);
                (hash as f64 / u64::MAX as f64) < *ratio
            }
            Self::Custom(func) => func(trace_id),
        }
    }
}

/// Span data structure for telemetry instrumentation
#[derive(Debug, Clone)]
pub struct TelemetrySpan {
    /// Unique span identifier
    pub span_id: u64,
    /// Parent span identifier (if any)
    pub parent_span_id: Option<u64>,
    /// Trace identifier this span belongs to
    pub trace_id: u64,
    /// Human-readable span name
    pub name: String,
    /// Span start time
    pub start_time: SystemTime,
    /// Span end time (None if still active)
    pub end_time: Option<SystemTime>,
    /// Span attributes as key-value pairs
    pub attributes: HashMap<String, String>,
    /// Span status
    pub status: SpanStatus,
}

/// Status of a telemetry span
#[derive(Debug, Clone, PartialEq)]
pub enum SpanStatus {
    /// Span completed successfully
    Ok,
    /// Span completed with error
    Error,
    /// Span is still active
    Unset,
}

/// Stream-specific telemetry context for connection tracking
#[derive(Debug)]
pub struct StreamTelemetryContext {
    /// Active spans indexed by span ID
    spans: Arc<RwLock<HashMap<u64, TelemetrySpan>>>,
    /// Connection mappings
    connections: Arc<RwLock<HashMap<ConnectionId, Vec<u64>>>>,
    /// Configuration
    config: TelemetryConfig,
    /// Sampler
    sampler: TelemetrySampler,
}

impl StreamTelemetryContext {
    pub fn new(config: TelemetryConfig) -> Self {
        Self {
            spans: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            sampler: TelemetrySampler::AlwaysOn,
            config,
        }
    }

    pub fn with_sampler(mut self, sampler: TelemetrySampler) -> Self {
        self.sampler = sampler;
        self
    }

    /// Create a new span with the given name and parent
    pub async fn create_span(&self, name: &str, parent_span_id: Option<u64>) -> Option<u64> {
        if !self.config.enabled {
            return None;
        }

        let span_id = self.generate_span_id();
        let trace_id = parent_span_id.unwrap_or_else(|| self.generate_trace_id());

        if !self.sampler.should_sample(trace_id) {
            return None;
        }

        let span = TelemetrySpan {
            span_id,
            parent_span_id,
            trace_id,
            name: name.to_string(),
            start_time: SystemTime::now(),
            end_time: None,
            attributes: HashMap::new(),
            status: SpanStatus::Unset,
        };

        let mut spans = self.spans.write().await;
        spans.insert(span_id, span);
        Some(span_id)
    }

    /// End a span with the given status
    pub async fn end_span(&self, span_id: u64, status: SpanStatus) {
        let mut spans = self.spans.write().await;
        if let Some(span) = spans.get_mut(&span_id) {
            span.end_time = Some(SystemTime::now());
            span.status = status;
        }
    }

    /// Add attribute to span
    pub async fn add_span_attribute(&self, span_id: u64, key: &str, value: &str) {
        let mut spans = self.spans.write().await;
        if let Some(span) = spans.get_mut(&span_id) {
            span.attributes.insert(key.to_string(), value.to_string());
        }
    }

    /// Associate span with connection
    pub async fn associate_connection(&self, connection_id: ConnectionId, span_id: u64) {
        let mut connections = self.connections.write().await;
        connections.entry(connection_id).or_default().push(span_id);
    }

    /// Get spans for connection
    pub async fn get_connection_spans(&self, connection_id: ConnectionId) -> Vec<u64> {
        let connections = self.connections.read().await;
        connections.get(&connection_id).cloned().unwrap_or_default()
    }

    fn generate_span_id(&self) -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    fn generate_trace_id(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
    }
}

/// Main telemetry instrumentation interface
pub struct NyxTelemetryInstrumentation {
    context: StreamTelemetryContext,
}

impl NyxTelemetryInstrumentation {
    pub fn new(config: TelemetryConfig) -> Self {
        Self {
            context: StreamTelemetryContext::new(config),
        }
    }

    pub fn with_sampler(mut self, sampler: TelemetrySampler) -> Self {
        self.context = self.context.with_sampler(sampler);
        self
    }

    /// Start instrumentation for a connection
    pub async fn start_connection_instrumentation(
        &self,
        connection_id: ConnectionId,
    ) -> Option<u64> {
        let span_id = self.context.create_span("connection_start", None).await?;
        self.context
            .associate_connection(connection_id, span_id)
            .await;
        self.context
            .add_span_attribute(span_id, "connection.id", &connection_id.inner().to_string())
            .await;
        Some(span_id)
    }

    /// End connection instrumentation
    pub async fn end_connection_instrumentation(&self, connection_id: ConnectionId, success: bool) {
        let spans = self.context.get_connection_spans(connection_id).await;
        for span_id in spans {
            let status = if success {
                SpanStatus::Ok
            } else {
                SpanStatus::Error
            };
            self.context.end_span(span_id, status).await;
        }
    }

    /// Record packet processing
    pub async fn record_packet_processing(&self, connection_id: ConnectionId, packet_size: usize) {
        if let Some(span_id) = self.context.create_span("packet_processing", None).await {
            self.context
                .associate_connection(connection_id, span_id)
                .await;
            self.context
                .add_span_attribute(span_id, "packet.size", &packet_size.to_string())
                .await;
            self.context
                .add_span_attribute(span_id, "connection.id", &connection_id.inner().to_string())
                .await;
        }
    }

    /// Record bandwidth metrics
    pub async fn record_bandwidth_usage(
        &self,
        connection_id: ConnectionId,
        bytes: u64,
        duration: Duration,
    ) {
        if let Some(span_id) = self.context.create_span("bandwidth_usage", None).await {
            self.context
                .associate_connection(connection_id, span_id)
                .await;
            self.context
                .add_span_attribute(span_id, "bandwidth.bytes", &bytes.to_string())
                .await;
            self.context
                .add_span_attribute(
                    span_id,
                    "bandwidth.duration_ms",
                    &duration.as_millis().to_string(),
                )
                .await;
            self.context
                .add_span_attribute(span_id, "connection.id", &connection_id.inner().to_string())
                .await;
        }
    }

    /// Get telemetry context
    pub fn get_context(&self) -> &StreamTelemetryContext {
        &self.context
    }
}

/// Standard span names used throughout the system
pub mod span_names {
    pub const CONNECTION_START: &str = "connection_start";
    pub const CONNECTION_END: &str = "connection_end";
    pub const PACKET_PROCESSING: &str = "packet_processing";
    pub const RATE_LIMITING: &str = "rate_limiting";
    pub const MULTIPATH_ROUTING: &str = "multipath_routing";
    pub const BANDWIDTH_MONITORING: &str = "bandwidth_monitoring";
    pub const SECURITY_CHECK: &str = "security_check";
    pub const PROTOCOL_NEGOTIATION: &str = "protocol_negotiation";
}

/// Standard attribute names for consistent telemetry
pub mod attribute_names {
    pub const CONNECTION_ID: &str = "connection.id";
    pub const PACKET_SIZE: &str = "packet.size";
    pub const BANDWIDTH_BYTES: &str = "bandwidth.bytes";
    pub const DURATION_MS: &str = "duration.ms";
    pub const ERROR_CODE: &str = "error.code";
    pub const ERROR_MESSAGE: &str = "error.message";
    pub const RATE_LIMIT_EXCEEDED: &str = "rate_limit.exceeded";
    pub const SECURITY_VIOLATION: &str = "security.violation";
    pub const PROTOCOL_VERSION: &str = "protocol.version";
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[tokio::test]
    async fn test_telemetry_span_creation() {
        let config = TelemetryConfig::default();
        let context = StreamTelemetryContext::new(config);

        let span_id = context.create_span("test_span", None).await.unwrap();
        assert!(span_id > 0);

        let spans = context.spans.read().await;
        assert!(spans.contains_key(&span_id));
        let span = &spans[&span_id];
        assert_eq!(span.name, "test_span");
        assert_eq!(span.status, SpanStatus::Unset);
    }

    #[tokio::test]
    async fn test_span_attributes() {
        let config = TelemetryConfig::default();
        let context = StreamTelemetryContext::new(config);

        let span_id = context.create_span("test_span", None).await.unwrap();
        context
            .add_span_attribute(span_id, "test_key", "test_value")
            .await;

        let spans = context.spans.read().await;
        let span = &spans[&span_id];
        assert_eq!(
            span.attributes.get("test_key"),
            Some(&"test_value".to_string())
        );
    }

    #[tokio::test]
    async fn test_connection_association() {
        let config = TelemetryConfig::default();
        let context = StreamTelemetryContext::new(config);
        let connection_id = ConnectionId::new(123);

        let span_id = context.create_span("test_span", None).await.unwrap();
        context.associate_connection(connection_id, span_id).await;

        let spans = context.get_connection_spans(connection_id).await;
        assert_eq!(spans, vec![span_id]);
    }

    #[tokio::test]
    async fn test_sampler_always_on() {
        let sampler = TelemetrySampler::AlwaysOn;
        assert!(sampler.should_sample(12345));
        assert!(sampler.should_sample(67890));
    }

    #[tokio::test]
    async fn test_sampler_always_off() {
        let sampler = TelemetrySampler::AlwaysOff;
        assert!(!sampler.should_sample(12345));
        assert!(!sampler.should_sample(67890));
    }

    #[tokio::test]
    async fn test_instrumentation_connection_lifecycle() {
        let config = TelemetryConfig::default();
        let instrumentation = NyxTelemetryInstrumentation::new(config);
        let connection_id = ConnectionId::new(456);

        let span_id = instrumentation
            .start_connection_instrumentation(connection_id)
            .await
            .unwrap();
        assert!(span_id > 0);

        instrumentation
            .end_connection_instrumentation(connection_id, true)
            .await;

        // Verify span was ended
        let spans = instrumentation.context.spans.read().await;
        let span = &spans[&span_id];
        assert_eq!(span.status, SpanStatus::Ok);
        assert!(span.end_time.is_some());
    }

    #[tokio::test]
    async fn test_packet_processing_recording() {
        let config = TelemetryConfig::default();
        let instrumentation = NyxTelemetryInstrumentation::new(config);
        let connection_id = ConnectionId::new(789);

        instrumentation
            .record_packet_processing(connection_id, 1024)
            .await;

        let spans = instrumentation
            .context
            .get_connection_spans(connection_id)
            .await;
        assert!(!spans.is_empty());
    }

    #[tokio::test]
    async fn test_bandwidth_recording() {
        let config = TelemetryConfig::default();
        let instrumentation = NyxTelemetryInstrumentation::new(config);
        let connection_id = ConnectionId::new(999);

        instrumentation
            .record_bandwidth_usage(connection_id, 2048, Duration::from_millis(100))
            .await;

        let spans = instrumentation
            .context
            .get_connection_spans(connection_id)
            .await;
        assert!(!spans.is_empty());
    }
}
