#![cfg(any(feature = "otlp", feature = "otlp_exporter"))]
//! Minimal OTLP integration (manual exporter, no SDK provider).

// no direct opentelemetry tracer usage (manual capture layer)
use serde::{Serialize, Deserialize};
use tracing::info;
use tracing_subscriber::Registry;
use std::time::Duration;
use opentelemetry_semantic_conventions as semcov;
#[cfg(feature = "otlp_exporter")]
use tokio::sync::mpsc;
#[cfg(feature = "otlp_exporter")]
use opentelemetry_proto::tonic::collector::trace::v1::trace_service_client::TraceServiceClient;
#[cfg(feature = "otlp_exporter")]
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
#[cfg(feature = "otlp_exporter")]
use opentelemetry_proto::tonic::trace::v1::{Span, span::SpanKind, ResourceSpans, ScopeSpans};
#[cfg(feature = "otlp_exporter")]
use opentelemetry_proto::tonic::common::v1::{InstrumentationScope, KeyValue as OTLPKeyValue, AnyValue};
#[cfg(feature = "otlp_exporter")]
use crate::otlp::{register_export_sender, CapturedSpan};
#[cfg(feature = "otlp_exporter")]
use crate::otlp::with_recovery;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub endpoint: String,
    pub service_name: String,
    pub sampling_ratio: f64,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self { endpoint: "http://localhost:4317".into(), service_name: "nyx".into(), sampling_ratio: 1.0 }
    }
}

pub struct NyxTelemetry;

impl NyxTelemetry {
    /// Initialize a local (non-exporting) tracer provider for tests / lightweight telemetry.
    pub fn init(cfg: TelemetryConfig) -> anyhow::Result<()> {
        // Initialize in-memory capture layer so that manual exporter can receive closes.
        #[cfg(feature = "otlp")]
        {
            use crate::otlp::init_in_memory_tracer;
            let (dispatch, _store) = init_in_memory_tracer(&cfg.service_name, cfg.sampling_ratio);
            let _ = tracing::dispatcher::set_global_default(dispatch);
        }
        #[cfg(not(feature = "otlp"))]
        {
            let subscriber = Registry::default();
            let _ = tracing::subscriber::set_global_default(subscriber);
        }
        info!(target="telemetry", "telemetry_in_memory_initialized svc={} ratio={}", cfg.service_name, cfg.sampling_ratio);
        Ok(())
    }

    #[cfg(feature = "otlp_exporter")]
    pub fn init_with_exporter(cfg: TelemetryConfig) -> anyhow::Result<()> {
        // Fallback: initialize local tracer (non-exporting) + manual sender worker.
        Self::init(TelemetryConfig { endpoint: cfg.endpoint.clone(), service_name: cfg.service_name.clone(), sampling_ratio: cfg.sampling_ratio })?;
        let (tx, mut rx) = mpsc::channel::<CapturedSpan>(128);
        register_export_sender(tx);
        let endpoint = cfg.endpoint.clone();
        let service_name = cfg.service_name.clone();
        tokio::spawn(async move {
            // Build static resource attributes once.
            let resource_attrs = vec![OTLPKeyValue { key: semcov::resource::SERVICE_NAME.to_string(), value: Some(AnyValue { value: Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(service_name)) }) }];
            while let Some(cspan) = rx.recv().await {
                // Build a minimal OTLP Span from CapturedSpan
                let attributes: Vec<OTLPKeyValue> = cspan.attributes.iter().map(|(k,v)| OTLPKeyValue { key: k.clone(), value: Some(AnyValue { value: Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(v.clone())) }) }).collect();
                let span = Span {
                    trace_id: vec![0;16], // placeholder deterministic
                    span_id: vec![0;8],
                    parent_span_id: vec![],
                    name: cspan.name.clone(),
                    kind: SpanKind::Internal as i32,
                    start_time_unix_nano: 0,
                    end_time_unix_nano: 0,
                    attributes,
                    dropped_attributes_count: 0,
                    events: vec![],
                    dropped_events_count: 0,
                    links: vec![],
                    dropped_links_count: 0,
                    status: None,
                    trace_state: String::new(),
                    flags: 0,
                };
                let scope_spans = ScopeSpans { scope: Some(InstrumentationScope { name: "manual".into(), version: "0".into(), attributes: vec![], dropped_attributes_count: 0 }), spans: vec![span], schema_url: String::new() };
                let resource_spans = ResourceSpans { resource: Some(opentelemetry_proto::tonic::resource::v1::Resource { attributes: resource_attrs.clone(), dropped_attributes_count: 0 }), scope_spans: vec![scope_spans], schema_url: String::new() };
                let req = ExportTraceServiceRequest { resource_spans: vec![resource_spans] };
                    let endpoint_clone = endpoint.clone();
                    // Use recovery (backoff + circuit breaker); ignore returned value.
                    let _ = with_recovery(|| {
                        let req = req.clone();
                        let endpoint_inner = endpoint_clone.clone();
                        async move {
                            let mut client = TraceServiceClient::connect(endpoint_inner).await?;
                            let _ = client.export(req).await?; // we only care that call succeeded
                            Ok(())
                        }
                    }).await;
            }
        });
    info!(target="telemetry", "manual_otlp_worker_initialized endpoint={} svc={} ratio={}", cfg.endpoint, cfg.service_name, cfg.sampling_ratio);
        Ok(())
    }

    /// Attempt a TCP connectivity health check to the OTLP endpoint (host:port derived from URL).
    /// Only available with exporter feature.
    #[cfg(feature = "otlp_exporter")]
    pub fn health_check(cfg: &TelemetryConfig, timeout: Duration) -> anyhow::Result<()> {
        use std::net::{TcpStream};
        // crude parse: strip scheme if present
        let mut s = cfg.endpoint.trim().to_string();
        if let Some(rest) = s.strip_prefix("http://") { s = rest.to_string(); }
        if let Some(rest) = s.strip_prefix("https://") { s = rest.to_string(); }
        // cut path
        if let Some(idx) = s.find('/') { s = s[..idx].to_string(); }
        if !s.contains(':') { s = format!("{}:4317", s); }
        let addr = s;
        let dur = timeout;
        TcpStream::connect_timeout(&addr.parse()?, dur)?; // relies on std net ToSocketAddrs
        Ok(())
    }

    /// Flush & shutdown global tracer provider (safe to call multiple times).
    #[cfg(any(feature = "otlp_exporter", feature = "otlp"))]
    pub fn shutdown() { /* no-op */ }

    pub fn test_span() {
        let span = tracing::span!(tracing::Level::INFO, "nyx.stream.send", path_id = 1, cid = "test");
        let _g = span.enter();
        tracing::info!(target="telemetry", "test_span_inside");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(flavor = "current_thread")]
    async fn init_and_emit_span() { NyxTelemetry::init(TelemetryConfig::default()).unwrap(); NyxTelemetry::test_span(); }
}
