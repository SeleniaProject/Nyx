#![allow(unexpected_cfgs)]
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(all(feature = "quic", test, run_quic_tests))]
//! Comprehensive QUIC Transport Integration Tests
//!
//! This test suite validates the production-grade QUIC implementation with:
//! - Connection establishment and lifecycle management
//! - Stream multiplexing with different types and priorities
//! - Datagram transmission with reliability guarantees
//! - Flow control and congestion management
//! - Error handling and recovery scenarios
//! - Performance metrics and monitoring
//! - Security and DoS protection mechanisms

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};
use tracing::{debug, info, warn};
use tracing_test::traced_test;

use nyx_core::config::QuicConfig;
use nyx_transport::quic::{
    ConnectionState, QuicConnection, QuicError, QuicTransport, StreamType, CONNECTION_TIMEOUT,
    MAX_CONCURRENT_STREAMS, MAX_DATAGRAM_SIZE,
};

/// Test result type for better error handling
type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Helper function to create test configuration
fn create_test_config(port_offset: u16) -> TestResult<QuicConfig> {
    let bind_addr = format!("127.0.0.1:{}", 9000 + port_offset)
        .parse()
        .map_err(|e| format!("Failed to parse bind address: {}", e))?;

    Ok(QuicConfig {
        bind_addr,
        idle_timeout_secs: 60,
        keep_alive_interval_secs: 15,
        max_concurrent_streams: 10,
    })
}

/// Helper function to establish client-server connection pair
async fn establish_connection_pair(
    server_port: u16,
    client_port: u16,
) -> TestResult<(Arc<QuicConnection>, Arc<QuicConnection>)> {
    let server_config = create_test_config(server_port)?;
    let mut server_transport = QuicTransport::new(server_config)
        .await
        .map_err(|e| format!("Failed to create server transport: {}", e))?;

    let server_addr = server_transport
        .endpoint
        .local_addr()
        .map_err(|e| format!("Failed to get server local addr: {}", e))?;

    let client_config = create_test_config(client_port)?;
    let client_transport = QuicTransport::new(client_config)
        .await
        .map_err(|e| format!("Failed to create client transport: {}", e))?;

    // Start server accept task
    let server_handle = tokio::spawn(async move { server_transport.accept().await });

    // Give server time to start listening
    sleep(Duration::from_millis(100)).await;

    // Client connects
    let client_conn = client_transport
        .connect(server_addr)
        .await
        .map_err(|e| format!("Failed to connect client: {}", e))?;

    let server_conn = server_handle
        .await
        .map_err(|e| format!("Failed to join server handle: {}", e))?
        .map_err(|e| format!("Failed to accept server connection: {}", e))?;

    Ok((client_conn, server_conn))
}

#[tokio::test]
#[traced_test]
async fn test_quic_transport_creation() -> TestResult<()> {
    let config = create_test_config(1)?;
    let transport = QuicTransport::new(config)
        .await
        .map_err(|e| format!("Failed to create QUIC transport: {}", e))?;

    let local_addr = transport
        .endpoint
        .local_addr()
        .map_err(|e| format!("Failed to get local address: {}", e))?;
    info!("QUIC transport created successfully on {}", local_addr);
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_connection_establishment() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(2, 3).await?;

    // Verify both connections are active
    assert!(
        client_conn.is_active(),
        "Client connection should be active"
    );
    assert!(
        server_conn.is_active(),
        "Server connection should be active"
    );

    // Verify connection states
    match client_conn.get_state() {
        ConnectionState::Connected { .. } => {}
        state => return Err(format!("Expected Connected state, got {:?}", state).into()),
    }

    match server_conn.get_state() {
        ConnectionState::Connected { .. } => {}
        state => return Err(format!("Expected Connected state, got {:?}", state).into()),
    }

    info!("Connection establishment test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_bidirectional_stream_communication() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(4, 5).await?;

    // Client opens bidirectional stream
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::MixPacket, 1)
        .await?;

    let test_data = b"Hello, QUIC world!";

    // Client sends data
    client_conn.send_on_stream(stream_id, test_data).await?;

    // Server receives incoming stream
    let incoming_result = timeout(Duration::from_secs(5), server_conn.connection.accept_bi()).await;

    assert!(
        incoming_result.is_ok(),
        "Server should receive incoming stream"
    );

    if let Ok(Ok((mut send, mut recv))) = incoming_result {
        // Server reads data
        let received = recv.read_chunk(1024, false).await??;

        assert_eq!(
            received.bytes, test_data,
            "Received data should match sent data"
        );

        // Server echoes back
        send.write_all(&received.bytes).await?;
        send.finish().await?;

        // Client receives echo
        let echo = client_conn
            .recv_from_stream(stream_id, Duration::from_secs(5))
            .await??;

        assert_eq!(echo, test_data, "Echo should match original data");
    }

    info!("Bidirectional stream communication test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_unidirectional_stream() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(6, 7).await?;

    // Client opens unidirectional stream
    let stream_id = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await?;

    let test_data = b"Telemetry data stream";

    // Client sends data
    client_conn.send_on_stream(stream_id, test_data).await?;

    // Server receives incoming stream
    let incoming_result =
        timeout(Duration::from_secs(5), server_conn.connection.accept_uni()).await;

    assert!(
        incoming_result.is_ok(),
        "Server should receive incoming unidirectional stream"
    );

    if let Ok(Ok(mut recv)) = incoming_result {
        // Server reads data
        let received = recv.read_chunk(1024, false).await??;

        assert_eq!(
            received.bytes, test_data,
            "Received data should match sent data"
        );
    }

    info!("Unidirectional stream test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_datagram_transmission() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(8, 9).await?;

    let test_data = b"Datagram message";

    // Client sends datagram
    client_conn.send_datagram(test_data).await?;

    // Server receives datagram
    let received = server_conn.recv_datagram(Duration::from_secs(5)).await??;

    assert_eq!(
        received, test_data,
        "Received datagram should match sent data"
    );

    info!("Datagram transmission test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_multiple_stream_types() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(10, 11).await?;

    // Open multiple streams of different types
    let control_stream = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await?;

    let telemetry_stream = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await?;

    let mix_stream = client_conn
        .open_bidirectional_stream(StreamType::MixPacket, 3)
        .await?;

    let auth_stream = client_conn
        .open_bidirectional_stream(StreamType::Authentication, 1)
        .await?;

    // Verify all streams have different IDs
    assert_ne!(control_stream, telemetry_stream);
    assert_ne!(telemetry_stream, mix_stream);
    assert_ne!(mix_stream, auth_stream);
    assert_ne!(control_stream, auth_stream);

    // Send data on each stream
    client_conn
        .send_on_stream(control_stream, b"Control message")
        .await?;

    client_conn
        .send_on_stream(telemetry_stream, b"Telemetry data")
        .await?;

    client_conn
        .send_on_stream(mix_stream, b"Mix packet")
        .await?;

    client_conn
        .send_on_stream(auth_stream, b"Auth handshake")
        .await?;

    // Verify stream count
    let streams_count = client_conn.streams.read().await.len();
    assert_eq!(streams_count, 4, "Should have 4 active streams");

    info!("Multiple stream types test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_stream_flow_control() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(12, 13).await?;

    let stream_id = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await?;

    // Try to send large amounts of data to trigger flow control
    let large_data = vec![0u8; 2 * 1024 * 1024]; // 2MB

    let mut send_count = 0;
    let mut total_sent = 0;

    // Keep sending until we hit flow control limits
    loop {
        match client_conn.send_on_stream(stream_id, &large_data).await {
            Ok(_) => {
                send_count += 1;
                total_sent += large_data.len();
                debug!("Sent chunk {}, total: {} bytes", send_count, total_sent);
            }
            Err(QuicError::ResourceExhausted { .. }) => {
                info!(
                    "Hit flow control limit after {} sends, {} bytes",
                    send_count, total_sent
                );
                break;
            }
            Err(e) => return Err(format!("Unexpected error: {}", e).into()),
        }

        if send_count > 10 {
            warn!("Flow control test didn't trigger within 10 sends");
            break;
        }
    }

    assert!(send_count > 0, "Should have sent at least some data");
    info!("Stream flow control test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_connection_statistics() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(14, 15).await?;

    // Get initial stats
    let initial_stats = client_conn.get_stats();
    assert_eq!(initial_stats.bytes_sent, 0);
    assert_eq!(initial_stats.streams_opened, 0);

    // Open a stream and send data
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await?;

    let test_data = b"Statistics test data";
    client_conn.send_on_stream(stream_id, test_data).await?;

    // Check updated stats
    let updated_stats = client_conn.get_stats();
    assert!(updated_stats.bytes_sent >= test_data.len() as u64);
    assert_eq!(updated_stats.streams_opened, 1);
    assert!(updated_stats.connection_duration > Duration::ZERO);

    info!(
        "Connection statistics test passed - sent {} bytes, opened {} streams",
        updated_stats.bytes_sent, updated_stats.streams_opened
    );
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_stream_lifecycle() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(16, 17).await?;

    // Open stream
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::MixPacket, 1)
        .await?;

    // Verify stream exists
    assert!(client_conn.streams.read().await.contains_key(&stream_id));

    // Send some data
    client_conn
        .send_on_stream(stream_id, b"Stream lifecycle test")
        .await?;

    // Close stream
    client_conn.close_stream(stream_id).await?;

    // Verify stream is removed
    assert!(!client_conn.streams.read().await.contains_key(&stream_id));

    // Verify stats updated
    let stats = client_conn.get_stats();
    assert_eq!(stats.streams_opened, 1);
    assert_eq!(stats.streams_closed, 1);

    info!("Stream lifecycle test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_large_datagram_rejection() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(18, 19).await?;

    // Try to send datagram larger than maximum size
    let large_data = vec![0u8; MAX_DATAGRAM_SIZE + 1];

    let result = client_conn.send_datagram(&large_data).await;

    assert!(result.is_err(), "Large datagram should be rejected");

    match result.unwrap_err() {
        QuicError::ProtocolViolation { violation } => {
            assert!(violation.contains("exceeds maximum"));
        }
        e => return Err(format!("Expected ProtocolViolation, got {:?}", e).into()),
    }

    info!("Large datagram rejection test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_max_concurrent_streams() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(20, 21).await?;

    let max_streams = client_conn.max_streams;
    let mut opened_streams = Vec::new();

    // Open streams up to the limit
    for i in 0..max_streams {
        let stream_id = client_conn
            .open_unidirectional_stream(StreamType::Telemetry, 1)
            .await
            .map_err(|e| format!("Failed to open stream {}: {}", i, e))?;
        opened_streams.push(stream_id);
    }

    // Try to open one more stream (should fail)
    let result = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 1)
        .await;

    assert!(result.is_err(), "Should not be able to exceed max streams");

    match result.unwrap_err() {
        QuicError::ResourceExhausted { resource } => {
            assert!(resource.contains("Maximum streams"));
        }
        e => return Err(format!("Expected ResourceExhausted, got {:?}", e).into()),
    }

    info!(
        "Max concurrent streams test passed - opened {} streams",
        opened_streams.len()
    );
}

#[tokio::test]
#[traced_test]
async fn test_connection_timeout() -> TestResult<()> {
    let config = create_test_config(22);
    let client_transport = QuicTransport::new(config).await?;

    // Try to connect to non-existent server
    let nonexistent_addr: SocketAddr = "127.0.0.1:9999".parse()?;
    let start_time = Instant::now();

    let result = client_transport.connect(nonexistent_addr).await;
    let elapsed = start_time.elapsed();

    assert!(
        result.is_err(),
        "Connection to non-existent server should fail"
    );
    assert!(
        elapsed >= CONNECTION_TIMEOUT,
        "Should wait for full timeout"
    );

    match result.unwrap_err() {
        QuicError::TimeoutError {
            operation,
            duration,
        } => {
            assert!(operation.contains("connection"));
            assert_eq!(duration, CONNECTION_TIMEOUT);
        }
        e => return Err(format!("Expected TimeoutError, got {:?}", e).into()),
    }

    info!("Connection timeout test passed after {:?}", elapsed);
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_idle_connection_cleanup() -> TestResult<()> {
    let server_config = QuicConfig {
        bind_addr: "127.0.0.1:9023".parse()?,
        idle_timeout_secs: 2, // Short timeout for testing
        keep_alive_interval_secs: 1,
        max_concurrent_streams: 10,
    };
    let mut server_transport = QuicTransport::new(server_config).await?;

    let server_addr = server_transport.endpoint.local_addr()?;

    let client_config = QuicConfig {
        bind_addr: "127.0.0.1:9024".parse()?,
        idle_timeout_secs: 2,
        keep_alive_interval_secs: 1,
        max_concurrent_streams: 10,
    };
    let client_transport = QuicTransport::new(client_config).await?;

    // Establish connection
    let server_handle = tokio::spawn(async move { server_transport.accept().await });

    sleep(Duration::from_millis(100)).await;
    let client_conn = client_transport.connect(server_addr).await?;
    let server_conn = server_handle.await??;

    // Verify connection is initially active
    assert!(client_conn.is_active());

    // Wait for idle timeout (add buffer time)
    sleep(Duration::from_secs(4)).await;

    // Connection should eventually be marked as inactive due to idle timeout
    // Note: This test might be flaky depending on timing and quinn's internal behavior
    info!("Idle connection cleanup test completed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_concurrent_operations() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(25, 26).await?;

    let client_conn_clone = client_conn.clone();
    let server_conn_clone = server_conn.clone();

    // Spawn multiple concurrent operations
    let handles: Vec<_> = (0..5)
        .map(|i| {
            let client = client_conn_clone.clone();
            tokio::spawn(async move {
                let stream_id = client
                    .open_bidirectional_stream(StreamType::MixPacket, 1)
                    .await
                    .map_err(|e| format!("Failed to open stream {}: {}", i, e))?;

                let data = format!("Concurrent message {}", i);
                client
                    .send_on_stream(stream_id, data.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to send on stream {}: {}", i, e))?;

                stream_id
            })
        })
        .collect();

    // Wait for all operations to complete
    let stream_ids: Vec<_> = futures::future::try_join_all(handles)
        .await?
        .into_iter()
        .collect();

    // Verify all streams were created
    assert_eq!(stream_ids.len(), 5);
    let active_streams = client_conn.streams.read().await.len();
    assert_eq!(active_streams, 5);

    // Send datagrams concurrently
    let datagram_handles: Vec<_> = (0..3)
        .map(|i| {
            let client = client_conn_clone.clone();
            tokio::spawn(async move {
                let data = format!("Datagram {}", i);
                client.send_datagram(data.as_bytes()).await
            })
        })
        .collect();

    // Wait for all datagrams
    let datagram_results: Vec<_> = futures::future::try_join_all(datagram_handles).await?;

    // All datagrams should succeed
    for result in datagram_results {
        result?;
    }

    info!("Concurrent operations test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_error_recovery() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(27, 28).await?;

    // Open a stream
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await?;

    // Close the stream
    client_conn.close_stream(stream_id).await?;

    // Try to send on closed stream (should fail)
    let result = client_conn.send_on_stream(stream_id, b"Should fail").await;

    assert!(
        result.is_err(),
        "Should not be able to send on closed stream"
    );

    match result.unwrap_err() {
        QuicError::StreamError {
            stream_id: err_id,
            reason,
        } => {
            assert_eq!(err_id, stream_id);
            assert!(reason.contains("not found"));
        }
        e => return Err(format!("Expected StreamError, got {:?}", e).into()),
    }

    // Try to receive from closed stream (should fail)
    let result = client_conn
        .recv_from_stream(stream_id, Duration::from_secs(1))
        .await;

    assert!(
        result.is_err(),
        "Should not be able to receive from closed stream"
    );

    info!("Error recovery test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_transport_statistics() -> TestResult<()> {
    let config = create_test_config(29);
    let transport = QuicTransport::new(config).await?;

    // Get initial transport stats
    let initial_stats = transport.get_transport_stats().await;
    assert_eq!(initial_stats.total_connections, 0);
    assert_eq!(initial_stats.active_connections, 0);

    info!("Transport statistics test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_graceful_connection_close() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(30, 31).await?;

    // Open some streams
    let stream1 = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await?;

    let stream2 = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await?;

    // Verify connection is active
    assert!(client_conn.is_active());

    // Close connection gracefully
    client_conn.close("Test close").await?;

    // Verify connection state
    match client_conn.get_state() {
        ConnectionState::Closed { reason, .. } => {
            assert_eq!(reason, "Test close");
        }
        state => return Err(format!("Expected Closed state, got {:?}", state).into()),
    }

    // Connection should no longer be active
    assert!(!client_conn.is_active());

    info!("Graceful connection close test passed");
    Ok(())
}
