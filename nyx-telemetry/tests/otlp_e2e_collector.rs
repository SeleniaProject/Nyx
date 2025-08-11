#![cfg(all(feature = "otlp_exporter", feature = "otlp"))]
// E2E test: NyxTelemetry exporter sends span to mock OTLP collector; attributes verified.
use nyx_telemetry::opentelemetry_integration::{TelemetryConfig as OCfg, NyxTelemetry};
use std::sync::{Arc, Mutex};
use opentelemetry_proto::tonic::collector::trace::v1::{trace_service_server::{TraceService, TraceServiceServer}, ExportTraceServiceRequest, ExportTraceServiceResponse};
use opentelemetry_proto::tonic::common::v1::any_value::Value;
use tonic::{Request, Response, Status};

#[derive(Clone, Default)]
struct MockTrace { store: Arc<Mutex<Vec<ExportTraceServiceRequest>>> }
#[tonic::async_trait]
impl TraceService for MockTrace {
    async fn export(&self, request: Request<ExportTraceServiceRequest>) -> Result<Response<ExportTraceServiceResponse>, Status> {
        self.store.lock().unwrap().push(request.into_inner());
        Ok(Response::new(ExportTraceServiceResponse { partial_success: None }))
    }
}

#[tokio::test(flavor="multi_thread", worker_threads=2)]
async fn otlp_exporter_sends_span_and_attributes_preserved() {
    let mock = MockTrace::default();
    let addr = ([127,0,0,1], 50097); // test port
    let server = mock.clone();
    tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(TraceServiceServer::new(server))
            .serve(addr.into())
            .await
            .unwrap();
    });
    // Allow server to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    NyxTelemetry::init_with_exporter(OCfg { endpoint: format!("http://{}.{}.{}.{}:{}", addr.0[0], addr.0[1], addr.0[2], addr.0[3], addr.1), service_name: "nyx-test".into(), sampling_ratio: 1.0 }).unwrap();
    {
        let span = tracing::span!(tracing::Level::INFO, "nyx.stream.send", path_id = 42u8, cid = "cid-e2e");
        let _e = span.enter();
    }
    // Give exporter time to send
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let data = mock.store.lock().unwrap();
    assert!(!data.is_empty(), "no export requests received");
    let mut found = false;
    for req in data.iter() {
        for rs in &req.resource_spans {
            for ss in &rs.scope_spans {
                for sp in &ss.spans {
                    if sp.name == "nyx.stream.send" {
                        let mut pid: Option<i64> = None; let mut cid: Option<String> = None;
                        for kv in &sp.attributes {
                            if kv.key == "path_id" {
                                if let Some(anyv) = &kv.value { if let Some(inner) = &anyv.value { match inner { Value::IntValue(i) => pid = Some(*i), Value::StringValue(s) => { if let Ok(parsed) = s.parse::<i64>() { pid = Some(parsed); } }, _ => {} } } }
                            }
                            if kv.key == "cid" {
                                if let Some(anyv) = &kv.value { if let Some(inner) = &anyv.value { if let Value::StringValue(s) = inner { cid = Some(s.clone()); } } }
                            }
                        }
                        assert_eq!(pid, Some(42));
                        assert_eq!(cid.as_deref(), Some("cid-e2e"));
                        found = true;
                    }
                }
            }
        }
    }
    assert!(found, "expected span not found in exported data");
}
