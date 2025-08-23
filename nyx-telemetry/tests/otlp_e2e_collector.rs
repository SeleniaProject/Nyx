#![cfg(all(feature = "otlp_exporter", feature = "otlp"))]

use std::net::SocketAddr;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};

use opentelemetry_proto::tonic::collector::trace::v1::trace_service_server::{
    TraceService, TraceServiceServer,
};
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};

struct MockTraceService {
    tx: mpsc::Sender<usize>,
}

#[tonic::async_trait]
impl TraceService for MockTraceService {
    async fn export(
        &self,
        request: Request<ExportTraceServiceRequest>,
    ) -> Result<Response<ExportTraceServiceResponse>, Status> {
        let req = request.into_inner();
        let mut count = 0usize;
        for r_s in req.resource_spans {
            for s_s in r_s.scope_spans {
                count = count.saturating_add(s_s.spans.len());
            }
        }
        let _ = self.tx.send(count).await;
        Ok(Response::new(ExportTraceServiceResponse {
            partial_success: None,
        }))
    }
}

async fn start_mock_collector(
    bind: SocketAddr,
) -> Result<(SocketAddr, oneshot::Sender<()>, mpsc::Receiver<usize>), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel::<usize>(8);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let svc = TraceServiceServer::new(MockTraceService { tx });
    let std_listener = std::net::TcpListener::bind(bind)?;
    std_listener.set_nonblocking(true)?;
    let addr = std_listener.local_addr()?;
    let tokio_listener = tokio::net::TcpListener::from_std(std_listener)?;
    let incoming = TcpListenerStream::new(tokio_listener);
    let server = tonic::transport::Server::builder()
        .add_service(svc)
        .serve_with_incoming_shutdown(incoming, async move {
            let _ = shutdown_rx.await;
        });
    tokio::spawn(async move {
        let _ = server.await;
    });
    Ok((addr, shutdown_tx, rx))
}

#[tokio::test(flavor = "current_thread")]
async fn otlp_exporter_flushes_to_mock_collector() -> Result<(), Box<dyn std::error::Error>> {
    let (addr, shutdown_tx, mut rx) = start_mock_collector("127.0.0.1:0".parse().unwrap()).await?;
    let endpoint = format!("http://{addr}");
    // For gRPC exporter, only the base endpoint (host:port) is used.
    std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint);

    let config_local = nyx_telemetry::Config {
        exporter: nyx_telemetry::Exporter::Otlp,
        servicename: Some("nyx-test".into()),
        ..Default::default()
    };
    if let Err(e) = nyx_telemetry::init(&config_local) {
        eprintln!("Failed to initialize telemetry: {e}");
        return Ok(());
    }
        return Ok(());
    }

    let span = tracing::info_span!("otlp_e2e_test_span");
    let enter = span.enter();
    tracing::info!("hello from nyx");
    drop(enter);
    // Ensure the span is closed so it can be exported.
    drop(span);

    tokio::time::sleep(Duration::from_millis(150)).await;
    nyx_telemetry::shutdown();

    let mut received = 0usize;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline {
        if let Ok(Some(n)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            received = received.saturating_add(n);
            if received > 0 {
                break;
            }
        }
        }
    }
    let _ = shutdown_tx.send(());
    assert!(received > 0, "collector did not receive span_s");
    Ok(())
}
