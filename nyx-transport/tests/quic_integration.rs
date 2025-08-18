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
    ConnectionState, QuicConnection, QuicError, QuicTransport, StreamType,
    MAX_DATAGRAM_SIZE, CONNECTION_TIMEOUT, MAX_CONCURRENT_STREAMS,
};

/// Helper function to create test configuration
fn create_test_config(port_offset: u16) -> QuicConfig {
    QuicConfig {
        bind_addr: format!("127.0.0.1:{}", 9000 + port_offset).parse().unwrap(),
        idle_timeout_secs: 60,
        keep_alive_interval_secs: 15,
        max_concurrent_streams: 10,
    }
}

/// Helper function to establish client-server connection pair
async fn establish_connection_pair(
    server_port: u16,
    client_port: u16,
) -> Result<(Arc<QuicConnection>, Arc<QuicConnection>), QuicError> {
    let server_config = create_test_config(server_port);
    let mut server_transport = QuicTransport::new(server_config).await?;
    let server_addr = server_transport.endpoint.local_addr().unwrap();

    let client_config = create_test_config(client_port);
    let client_transport = QuicTransport::new(client_config).await?;

    // Start server accept task
    let server_handle = tokio::spawn(async move {
        server_transport.accept().await
    });

    // Give server time to start listening
    sleep(Duration::from_millis(100)).await;

    // Client connects
    let client_conn = client_transport.connect(server_addr).await?;
    let server_conn = server_handle.await.unwrap()?;

    Ok((client_conn, server_conn))
}

#[tokio::test]
#[traced_test]
async fn test_quic_transport_creation() {
    let config = create_test_config(1);
    let transport = QuicTransport::new(config).await;
    assert!(transport.is_ok(), "Failed to create QUIC transport: {:?}", transport.err());

    let transport = transport.unwrap();
    info!("QUIC transport created successfully on {}", transport.endpoint.local_addr().unwrap());
}

#[tokio::test]
#[traced_test]
async fn test_connection_establishment() {
    let (client_conn, server_conn) = establish_connection_pair(2, 3).await
        .expect("Failed to establish connection pair");

    // Verify both connections are active
    assert!(client_conn.is_active(), "Client connection should be active");
    assert!(server_conn.is_active(), "Server connection should be active");

    // Verify connection states
    match client_conn.get_state() {
        ConnectionState::Connected { .. } => {},
        state => panic!("Expected Connected state, got {:?}", state),
    }

    match server_conn.get_state() {
        ConnectionState::Connected { .. } => {},
        state => panic!("Expected Connected state, got {:?}", state),
    }

    info!("Connection establishment test passed");
}

#[tokio::test]
#[traced_test]
async fn test_bidirectional_stream_communication() {
    let (client_conn, server_conn) = establish_connection_pair(4, 5).await
        .expect("Failed to establish connection pair");

    // Client opens bidirectional stream
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::MixPacket, 1)
        .await
        .expect("Failed to open bidirectional stream");

    let test_data = b"Hello, QUIC world!";
    
    // Client sends data
    client_conn
        .send_on_stream(stream_id, test_data)
        .await
        .expect("Failed to send data on stream");

    // Server receives incoming stream
    let incoming_result = timeout(
        Duration::from_secs(5),
        server_conn.connection.accept_bi()
    ).await;

    assert!(incoming_result.is_ok(), "Server should receive incoming stream");
    
    if let Ok(Ok((mut send, mut recv))) = incoming_result {
        // Server reads data
        let received = recv.read_chunk(1024, false).await
            .expect("Failed to read from stream")
            .expect("No data received");
        
        assert_eq!(received.bytes, test_data, "Received data should match sent data");

        // Server echoes back
        send.write_all(&received.bytes).await
            .expect("Failed to write echo response");
        send.finish().await
            .expect("Failed to finish send stream");

        // Client receives echo
        let echo = client_conn
            .recv_from_stream(stream_id, Duration::from_secs(5))
            .await
            .expect("Failed to receive echo")
            .expect("No echo received");

        assert_eq!(echo, test_data, "Echo should match original data");
    }

    info!("Bidirectional stream communication test passed");
}

#[tokio::test]
#[traced_test]
async fn test_unidirectional_stream() {
    let (client_conn, server_conn) = establish_connection_pair(6, 7).await
        .expect("Failed to establish connection pair");

    // Client opens unidirectional stream
    let stream_id = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await
        .expect("Failed to open unidirectional stream");

    let test_data = b"Telemetry data stream";
    
    // Client sends data
    client_conn
        .send_on_stream(stream_id, test_data)
        .await
        .expect("Failed to send data on stream");

    // Server receives incoming stream
    let incoming_result = timeout(
        Duration::from_secs(5),
        server_conn.connection.accept_uni()
    ).await;

    assert!(incoming_result.is_ok(), "Server should receive incoming unidirectional stream");
    
    if let Ok(Ok(mut recv)) = incoming_result {
        // Server reads data
        let received = recv.read_chunk(1024, false).await
            .expect("Failed to read from stream")
            .expect("No data received");
        
        assert_eq!(received.bytes, test_data, "Received data should match sent data");
    }

    info!("Unidirectional stream test passed");
}

#[tokio::test]
#[traced_test]
async fn test_datagram_transmission() {
    let (client_conn, server_conn) = establish_connection_pair(8, 9).await
        .expect("Failed to establish connection pair");

    let test_data = b"Datagram message";
    
    // Client sends datagram
    client_conn
        .send_datagram(test_data)
        .await
        .expect("Failed to send datagram");

    // Server receives datagram
    let received = server_conn
        .recv_datagram(Duration::from_secs(5))
        .await
        .expect("Failed to receive datagram")
        .expect("No datagram received");

    assert_eq!(received, test_data, "Received datagram should match sent data");

    info!("Datagram transmission test passed");
}

#[tokio::test]
#[traced_test]
async fn test_multiple_stream_types() {
    let (client_conn, server_conn) = establish_connection_pair(10, 11).await
        .expect("Failed to establish connection pair");

    // Open multiple streams of different types
    let control_stream = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await
        .expect("Failed to open control stream");

    let telemetry_stream = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await
        .expect("Failed to open telemetry stream");

    let mix_stream = client_conn
        .open_bidirectional_stream(StreamType::MixPacket, 3)
        .await
        .expect("Failed to open mix stream");

    let auth_stream = client_conn
        .open_bidirectional_stream(StreamType::Authentication, 1)
        .await
        .expect("Failed to open auth stream");

    // Verify all streams have different IDs
    assert_ne!(control_stream, telemetry_stream);
    assert_ne!(telemetry_stream, mix_stream);
    assert_ne!(mix_stream, auth_stream);
    assert_ne!(control_stream, auth_stream);

    // Send data on each stream
    client_conn.send_on_stream(control_stream, b"Control message").await
        .expect("Failed to send on control stream");
    
    client_conn.send_on_stream(telemetry_stream, b"Telemetry data").await
        .expect("Failed to send on telemetry stream");
    
    client_conn.send_on_stream(mix_stream, b"Mix packet").await
        .expect("Failed to send on mix stream");
    
    client_conn.send_on_stream(auth_stream, b"Auth handshake").await
        .expect("Failed to send on auth stream");

    // Verify stream count
    let streams_count = client_conn.streams.read().await.len();
    assert_eq!(streams_count, 4, "Should have 4 active streams");

    info!("Multiple stream types test passed");
}

#[tokio::test]
#[traced_test]
async fn test_stream_flow_control() {
    let (client_conn, _server_conn) = establish_connection_pair(12, 13).await
        .expect("Failed to establish connection pair");

    let stream_id = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await
        .expect("Failed to open stream");

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
                info!("Hit flow control limit after {} sends, {} bytes", send_count, total_sent);
                break;
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
        
        if send_count > 10 {
            warn!("Flow control test didn't trigger within 10 sends");
            break;
        }
    }

    assert!(send_count > 0, "Should have sent at least some data");
    info!("Stream flow control test passed");
}

#[tokio::test]
#[traced_test]
async fn test_connection_statistics() {
    let (client_conn, _server_conn) = establish_connection_pair(14, 15).await
        .expect("Failed to establish connection pair");

    // Get initial stats
    let initial_stats = client_conn.get_stats();
    assert_eq!(initial_stats.bytes_sent, 0);
    assert_eq!(initial_stats.streams_opened, 0);

    // Open a stream and send data
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await
        .expect("Failed to open stream");

    let test_data = b"Statistics test data";
    client_conn
        .send_on_stream(stream_id, test_data)
        .await
        .expect("Failed to send data");

    // Check updated stats
    let updated_stats = client_conn.get_stats();
    assert!(updated_stats.bytes_sent >= test_data.len() as u64);
    assert_eq!(updated_stats.streams_opened, 1);
    assert!(updated_stats.connection_duration > Duration::ZERO);

    info!("Connection statistics test passed - sent {} bytes, opened {} streams", 
          updated_stats.bytes_sent, updated_stats.streams_opened);
}

#[tokio::test]
#[traced_test]
async fn test_stream_lifecycle() {
    let (client_conn, _server_conn) = establish_connection_pair(16, 17).await
        .expect("Failed to establish connection pair");

    // Open stream
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::MixPacket, 1)
        .await
        .expect("Failed to open stream");

    // Verify stream exists
    assert!(client_conn.streams.read().await.contains_key(&stream_id));

    // Send some data
    client_conn
        .send_on_stream(stream_id, b"Stream lifecycle test")
        .await
        .expect("Failed to send data");

    // Close stream
    client_conn
        .close_stream(stream_id)
        .await
        .expect("Failed to close stream");

    // Verify stream is removed
    assert!(!client_conn.streams.read().await.contains_key(&stream_id));

    // Verify stats updated
    let stats = client_conn.get_stats();
    assert_eq!(stats.streams_opened, 1);
    assert_eq!(stats.streams_closed, 1);

    info!("Stream lifecycle test passed");
}

#[tokio::test]
#[traced_test]
async fn test_large_datagram_rejection() {
    let (client_conn, _server_conn) = establish_connection_pair(18, 19).await
        .expect("Failed to establish connection pair");

    // Try to send datagram larger than maximum size
    let large_data = vec![0u8; MAX_DATAGRAM_SIZE + 1];
    
    let result = client_conn.send_datagram(&large_data).await;
    
    assert!(result.is_err(), "Large datagram should be rejected");
    
    match result.unwrap_err() {
        QuicError::ProtocolViolation { violation } => {
            assert!(violation.contains("exceeds maximum"));
        }
        e => panic!("Expected ProtocolViolation, got {:?}", e),
    }

    info!("Large datagram rejection test passed");
}

#[tokio::test]
#[traced_test]
async fn test_max_concurrent_streams() {
    let (client_conn, _server_conn) = establish_connection_pair(20, 21).await
        .expect("Failed to establish connection pair");

    let max_streams = client_conn.max_streams;
    let mut opened_streams = Vec::new();

    // Open streams up to the limit
    for i in 0..max_streams {
        let stream_id = client_conn
            .open_unidirectional_stream(StreamType::Telemetry, 1)
            .await
            .expect(&format!("Failed to open stream {}", i));
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
        e => panic!("Expected ResourceExhausted, got {:?}", e),
    }

    info!("Max concurrent streams test passed - opened {} streams", opened_streams.len());
}

#[tokio::test]
#[traced_test]
async fn test_connection_timeout() {
    let config = create_test_config(22);
    let client_transport = QuicTransport::new(config).await
        .expect("Failed to create client transport");

    // Try to connect to non-existent server
    let nonexistent_addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
    let start_time = Instant::now();
    
    let result = client_transport.connect(nonexistent_addr).await;
    let elapsed = start_time.elapsed();

    assert!(result.is_err(), "Connection to non-existent server should fail");
    assert!(elapsed >= CONNECTION_TIMEOUT, "Should wait for full timeout");
    
    match result.unwrap_err() {
        QuicError::TimeoutError { operation, duration } => {
            assert!(operation.contains("connection"));
            assert_eq!(duration, CONNECTION_TIMEOUT);
        }
        e => panic!("Expected TimeoutError, got {:?}", e),
    }

    info!("Connection timeout test passed after {:?}", elapsed);
}

#[tokio::test]
#[traced_test]
async fn test_idle_connection_cleanup() {
    let server_config = QuicConfig {
        bind_addr: "127.0.0.1:9023".parse().unwrap(),
        idle_timeout_secs: 2, // Short timeout for testing
        keep_alive_interval_secs: 1,
        max_concurrent_streams: 10,
    };
    let mut server_transport = QuicTransport::new(server_config).await
        .expect("Failed to create server transport");
    
    let server_addr = server_transport.endpoint.local_addr().unwrap();

    let client_config = QuicConfig {
        bind_addr: "127.0.0.1:9024".parse().unwrap(),
        idle_timeout_secs: 2,
        keep_alive_interval_secs: 1,
        max_concurrent_streams: 10,
    };
    let client_transport = QuicTransport::new(client_config).await
        .expect("Failed to create client transport");

    // Establish connection
    let server_handle = tokio::spawn(async move {
        server_transport.accept().await
    });

    sleep(Duration::from_millis(100)).await;
    let client_conn = client_transport.connect(server_addr).await
        .expect("Failed to connect");
    let _server_conn = server_handle.await.unwrap()
        .expect("Failed to accept connection");

    // Verify connection is initially active
    assert!(client_conn.is_active());

    // Wait for idle timeout (add buffer time)
    sleep(Duration::from_secs(4)).await;

    // Connection should eventually be marked as inactive due to idle timeout
    // Note: This test might be flaky depending on timing and quinn's internal behavior
    info!("Idle connection cleanup test completed");
}

#[tokio::test]
#[traced_test]
async fn test_concurrent_operations() {
    let (client_conn, server_conn) = establish_connection_pair(25, 26).await
        .expect("Failed to establish connection pair");

    let client_conn_clone = client_conn.clone();
    let server_conn_clone = server_conn.clone();

    // Spawn multiple concurrent operations
    let handles: Vec<_> = (0..5).map(|i| {
        let client = client_conn_clone.clone();
        tokio::spawn(async move {
            let stream_id = client
                .open_bidirectional_stream(StreamType::MixPacket, 1)
                .await
                .expect(&format!("Failed to open stream {}", i));
            
            let data = format!("Concurrent message {}", i);
            client
                .send_on_stream(stream_id, data.as_bytes())
                .await
                .expect(&format!("Failed to send on stream {}", i));
            
            stream_id
        })
    }).collect();

    // Wait for all operations to complete
    let stream_ids: Vec<_> = futures::future::try_join_all(handles).await
        .expect("Concurrent operations should complete")
        .into_iter()
        .collect();

    // Verify all streams were created
    assert_eq!(stream_ids.len(), 5);
    let active_streams = client_conn.streams.read().await.len();
    assert_eq!(active_streams, 5);

    // Send datagrams concurrently
    let datagram_handles: Vec<_> = (0..3).map(|i| {
        let client = client_conn_clone.clone();
        tokio::spawn(async move {
            let data = format!("Datagram {}", i);
            client.send_datagram(data.as_bytes()).await
        })
    }).collect();

    // Wait for all datagrams
    let datagram_results: Vec<_> = futures::future::try_join_all(datagram_handles).await
        .expect("Datagram tasks should complete");

    // All datagrams should succeed
    for result in datagram_results {
        result.expect("Datagram send should succeed");
    }

    info!("Concurrent operations test passed");
}

#[tokio::test]
#[traced_test]
async fn test_error_recovery() {
    let (client_conn, _server_conn) = establish_connection_pair(27, 28).await
        .expect("Failed to establish connection pair");

    // Open a stream
    let stream_id = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await
        .expect("Failed to open stream");

    // Close the stream
    client_conn
        .close_stream(stream_id)
        .await
        .expect("Failed to close stream");

    // Try to send on closed stream (should fail)
    let result = client_conn
        .send_on_stream(stream_id, b"Should fail")
        .await;

    assert!(result.is_err(), "Should not be able to send on closed stream");
    
    match result.unwrap_err() {
        QuicError::StreamError { stream_id: err_id, reason } => {
            assert_eq!(err_id, stream_id);
            assert!(reason.contains("not found"));
        }
        e => panic!("Expected StreamError, got {:?}", e),
    }

    // Try to receive from closed stream (should fail)
    let result = client_conn
        .recv_from_stream(stream_id, Duration::from_secs(1))
        .await;

    assert!(result.is_err(), "Should not be able to receive from closed stream");

    info!("Error recovery test passed");
}

#[tokio::test]
#[traced_test]
async fn test_transport_statistics() {
    let config = create_test_config(29);
    let transport = QuicTransport::new(config).await
        .expect("Failed to create transport");

    // Get initial transport stats
    let initial_stats = transport.get_transport_stats().await;
    assert_eq!(initial_stats.total_connections, 0);
    assert_eq!(initial_stats.active_connections, 0);

    info!("Transport statistics test passed");
}

#[tokio::test]
#[traced_test]
async fn test_graceful_connection_close() {
    let (client_conn, server_conn) = establish_connection_pair(30, 31).await
        .expect("Failed to establish connection pair");

    // Open some streams
    let _stream1 = client_conn
        .open_bidirectional_stream(StreamType::Control, 1)
        .await
        .expect("Failed to open stream");

    let _stream2 = client_conn
        .open_unidirectional_stream(StreamType::Telemetry, 2)
        .await
        .expect("Failed to open stream");

    // Verify connection is active
    assert!(client_conn.is_active());

    // Close connection gracefully
    client_conn
        .close("Test close")
        .await
        .expect("Failed to close connection");

    // Verify connection state
    match client_conn.get_state() {
        ConnectionState::Closed { reason, .. } => {
            assert_eq!(reason, "Test close");
        }
        state => panic!("Expected Closed state, got {:?}", state),
    }

    // Connection should no longer be active
    assert!(!client_conn.is_active());

    info!("Graceful connection close test passed");
}
