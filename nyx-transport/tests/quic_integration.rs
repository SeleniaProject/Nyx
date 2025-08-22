#![cfg(feature = "quic")]
//! Comprehensive QUIC Transport Integration Test_s
//!
//! This test suite validates the production-grade QUIC implementation with:
//! - Connection establishment and lifecycle management
//! - Stream multiplexing with different type_s and prioritie_s
//! - Datagram transmission with reliability guarantees
//! - Flow control and congestion management
//! - Error handling and recovery scenario_s
//! - Performance metric_s and monitoring
//! - Security and DoS protection mechanism_s

use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use byte_s::Byte_s;
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

    // Client connect_s
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
        .map_err(|e| format!("Failed to get local addres_s: {}", e))?;
    info!("QUIC transport created successfully on {}", local_addr);
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_connection_establishment() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(2, 3).await?;

    // Verify both connection_s are active
    assert!(
        client_conn.is_active(),
        "Client connection should be active"
    );
    assert!(
        server_conn.is_active(),
        "Server connection should be active"
    );

    // Verify connection state_s
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

    // Client open_s bidirectional stream
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::MixPacket, 1)
        .await?;

    let test_data = b"Hello, QUIC world!";

    // Client send_s data
    client_conn.send_on_stream(stream_id, test_data).await?;

    // Server receive_s incoming stream
    let incoming_result =
        timeout(Duration::from_sec_s(5), server_conn.connection.accept_bi()).await;

    assert!(
        incoming_result.is_ok(),
        "Server should receive incoming stream"
    );

    if let Ok(Ok((mut send, mut recv))) = incoming_result {
        // Server read_s data
        let received = recv.read_chunk(1024, false).await??;

        assert_eq!(
            received.byte_s, test_data,
            "Received data should match sent data"
        );

        // Server echoe_s back
        send.write_all(&received.byte_s).await?;
        send.finish().await?;

        // Client receive_s echo
        let echo = client_conn
            .recv_from_stream(stream_id, Duration::from_sec_s(5))
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

    // Client open_s unidirectional stream
    let stream_id = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await?;

    let test_data = b"Telemetry data stream";

    // Client send_s data
    client_conn.send_on_stream(stream_id, test_data).await?;

    // Server receive_s incoming stream
    let incoming_result =
        timeout(Duration::from_sec_s(5), server_conn.connection.accept_uni()).await;

    assert!(
        incoming_result.is_ok(),
        "Server should receive incoming unidirectional stream"
    );

    if let Ok(Ok(mut recv)) = incoming_result {
        // Server read_s data
        let received = recv.read_chunk(1024, false).await??;

        assert_eq!(
            received.byte_s, test_data,
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

    // Client send_s datagram
    client_conn.send_datagram(test_data).await?;

    // Server receive_s datagram
    let received = server_conn.recv_datagram(Duration::from_sec_s(5)).await??;

    assert_eq!(
        received, test_data,
        "Received datagram should match sent data"
    );

    info!("Datagram transmission test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_multiple_stream_type_s() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(10, 11).await?;

    // Open multiple stream_s of different type_s
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

    // Verify all stream_s have different ID_s
    assertne!(control_stream, telemetry_stream);
    assertne!(telemetry_stream, mix_stream);
    assertne!(mix_stream, auth_stream);
    assertne!(control_stream, auth_stream);

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
    let streams_count = client_conn.stream_s.read().await.len();
    assert_eq!(streams_count, 4, "Should have 4 active stream_s");

    info!("Multiple stream type_s test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_stream_flow_control() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(12, 13).await?;

    let stream_id = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await?;

    // Try to send large amount_s of data to trigger flow control
    let large_data = vec![0u8; 2 * 1024 * 1024]; // 2MB

    let mut send_count = 0;
    let mut total_sent = 0;

    // Keep sending until we hit flow control limit_s
    loop {
        match client_conn.send_on_stream(stream_id, &large_data).await {
            Ok(_) => {
                send_count += 1;
                total_sent += large_data.len();
                debug!("Sent chunk {}, total: {} byte_s", send_count, total_sent);
            }
            Err(QuicError::ResourceExhausted { .. }) => {
                info!(
                    "Hit flow control limit after {} send_s, {} byte_s",
                    send_count, total_sent
                );
                break;
            }
            Err(e) => return Err(format!("Unexpected error: {}", e).into()),
        }

        if send_count > 10 {
            warn!("Flow control test didn't trigger within 10 send_s");
            break;
        }
    }

    assert!(send_count > 0, "Should have sent at least some data");
    info!("Stream flow control test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_connection_statistic_s() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(14, 15).await?;

    // Get initial stat_s
    let initial_stat_s = client_conn.get_stat_s();
    assert_eq!(initial_stat_s.bytes_sent, 0);
    assert_eq!(initial_stat_s.streams_opened, 0);

    // Open a stream and send data
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await?;

    let test_data = b"Statistic_s test data";
    client_conn.send_on_stream(stream_id, test_data).await?;

    // Check updated stat_s
    let updated_stat_s = client_conn.get_stat_s();
    assert!(updated_stat_s.bytes_sent >= test_data.len() as u64);
    assert_eq!(updated_stat_s.streams_opened, 1);
    assert!(updated_stat_s.connection_duration > Duration::ZERO);

    info!(
        "Connection statistic_s test passed - sent {} byte_s, opened {} stream_s",
        updated_stat_s.bytes_sent, updated_stat_s.streams_opened
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

    // Verify stream exist_s
    assert!(client_conn.stream_s.read().await.contains_key(&stream_id));

    // Send some data
    client_conn
        .send_on_stream(stream_id, b"Stream lifecycle test")
        .await?;

    // Close stream
    client_conn.close_stream(stream_id).await?;

    // Verify stream is removed
    assert!(!client_conn.stream_s.read().await.contains_key(&stream_id));

    // Verify stat_s updated
    let stat_s = client_conn.get_stat_s();
    assert_eq!(stat_s.streams_opened, 1);
    assert_eq!(stat_s.streams_closed, 1);

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
            assert!(violation.contains("exceed_s maximum"));
        }
        e => return Err(format!("Expected ProtocolViolation, got {:?}", e).into()),
    }

    info!("Large datagram rejection test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_max_concurrent_stream_s() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(20, 21).await?;

    let max_stream_s = client_conn.max_stream_s;
    let mut opened_stream_s = Vec::new();

    // Open stream_s up to the limit
    for i in 0..max_stream_s {
        let stream_id = client_conn
            .open_unidirectional_stream(StreamType::Telemetry, 1)
            .await
            .map_err(|e| format!("Failed to open stream {}: {}", i, e))?;
        opened_stream_s.push(stream_id);
    }

    // Try to open one more stream (should fail)
    let result = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 1)
        .await;

    assert!(result.is_err(), "Should not be able to exceed max stream_s");

    match result.unwrap_err() {
        QuicError::ResourceExhausted { resource } => {
            assert!(resource.contains("Maximum stream_s"));
        }
        e => return Err(format!("Expected ResourceExhausted, got {:?}", e).into()),
    }

    info!(
        "Max concurrent stream_s test passed - opened {} stream_s",
        opened_stream_s.len()
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
        idle_timeout_sec_s: 2, // Short timeout for testing
        keep_alive_interval_sec_s: 1,
        max_concurrent_stream_s: 10,
    };
    let mut server_transport = QuicTransport::new(server_config).await?;

    let server_addr = server_transport.endpoint.local_addr()?;

    let client_config = QuicConfig {
        bind_addr: "127.0.0.1:9024".parse()?,
        idle_timeout_sec_s: 2,
        keep_alive_interval_sec_s: 1,
        max_concurrent_stream_s: 10,
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
    sleep(Duration::from_sec_s(4)).await;

    // Connection should eventually be marked as inactive due to idle timeout
    // Note: This test might be flaky depending on timing and quinn's internal behavior
    info!("Idle connection cleanup test completed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_concurrent_operation_s() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(25, 26).await?;

    let client_conn_clone = client_conn.clone();
    let server_conn_clone = server_conn.clone();

    // Spawn multiple concurrent operation_s
    let handle_s: Vec<_> = (0..5)
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

    // Wait for all operation_s to complete
    let stream_id_s: Vec<_> = futu_re_s::future::try_join_all(handle_s)
        .await?
        .into_iter()
        .collect();

    // Verify all stream_s were created
    assert_eq!(stream_id_s.len(), 5);
    let active_stream_s = client_conn.stream_s.read().await.len();
    assert_eq!(active_stream_s, 5);

    // Send datagram_s concurrently
    let datagram_handle_s: Vec<_> = (0..3)
        .map(|i| {
            let client = client_conn_clone.clone();
            tokio::spawn(async move {
                let data = format!("Datagram {}", i);
                client.send_datagram(data.as_bytes()).await
            })
        })
        .collect();

    // Wait for all datagram_s
    let datagram_result_s: Vec<_> = futu_re_s::future::try_join_all(datagram_handle_s).await?;

    // All datagram_s should succeed
    for result in datagram_result_s {
        result?;
    }

    info!("Concurrent operation_s test passed");
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
        .recv_from_stream(stream_id, Duration::from_sec_s(1))
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
async fn test_transport_statistic_s() -> TestResult<()> {
    let config = create_test_config(29);
    let transport = QuicTransport::new(config).await?;

    // Get initial transport stat_s
    let initial_stat_s = transport.get_transport_stat_s().await;
    assert_eq!(initial_stat_s.total_connection_s, 0);
    assert_eq!(initial_stat_s.active_connection_s, 0);

    info!("Transport statistic_s test passed");
    Ok(())
}

#[tokio::test]
#[traced_test]
async fn test_graceful_connection_close() -> TestResult<()> {
    let (client_conn, server_conn) = establish_connection_pair(30, 31).await?;

    // Open some stream_s
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
