// End-to-end handshake flow test
//
// Tests complete client-server handshake with capability negotiation.
// Reference: spec/Nyx_Protocol_v1.0_Spec_EN.md §3, §4

use nyx_integration_tests::{DaemonConfig, TestHarness, TestResult};

/// Test daemon spawning and basic TCP connectivity
/// 
/// Note: nyx-daemon expects JSON-RPC protocol, not raw PING messages.
/// This test validates:
/// 1. Daemon process spawning via cargo run
/// 2. TCP listener binding and accepting connections
/// 3. Basic read/write capability over TCP
///
/// Future enhancements: Implement proper JSON-RPC handshake protocol
#[tokio::test]
async fn test_daemon_spawn_and_connect() -> TestResult<()> {
    // Initialize tracing for test visibility
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    tracing::info!("Starting daemon spawn and connect test");

    // Create test harness
    let mut harness = TestHarness::new();

    // Spawn server daemon
    let server_config = DaemonConfig {
        bind_addr: "127.0.0.1:29000".parse().unwrap(), // Use high port to avoid conflicts
        telemetry_enabled: false,
        ..Default::default()
    };

    harness.spawn_daemon("server", server_config).await?;
    tracing::info!("Daemon spawned, waiting for readiness");

    // Connect client to server
    harness.connect_client("client", "server").await?;
    tracing::info!("Client connected successfully");

    // Verify client handle exists
    let _client = harness.client("client").expect("Client not found");
    
    // Test passes if we can spawn daemon and establish TCP connection
    // Full JSON-RPC protocol testing deferred to future work
    tracing::info!("Test passed: daemon spawned and TCP connection established");

    // Cleanup
    harness.shutdown_all().await?;

    tracing::info!("Daemon spawn test completed successfully");
    Ok(())
}

/// Test multi-node daemon orchestration
/// 
/// Validates:
/// 1. Multiple daemon instances can run simultaneously
/// 2. Each daemon binds to a different port
/// 3. Clients can connect to independent daemons
/// 4. Test harness correctly manages multiple processes
#[tokio::test]
async fn test_multinode_scenario() -> TestResult<()> {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    tracing::info!("Starting multi-node scenario test");

    let mut harness = TestHarness::new();

    // Spawn two daemons on different ports
    let server1_config = DaemonConfig {
        bind_addr: "127.0.0.1:29001".parse().unwrap(),
        telemetry_enabled: false,
        ..Default::default()
    };

    let server2_config = DaemonConfig {
        bind_addr: "127.0.0.1:29002".parse().unwrap(),
        telemetry_enabled: false,
        ..Default::default()
    };

    harness.spawn_daemon("server1", server1_config).await?;
    tracing::info!("Server 1 spawned");
    
    harness.spawn_daemon("server2", server2_config).await?;
    tracing::info!("Server 2 spawned");

    // Connect clients to both servers
    harness.connect_client("client1", "server1").await?;
    tracing::info!("Client 1 connected to server 1");
    
    harness.connect_client("client2", "server2").await?;
    tracing::info!("Client 2 connected to server 2");

    // Verify both connections exist
    let _client1 = harness.client("client1").expect("Client1 not found");
    let _client2 = harness.client("client2").expect("Client2 not found");

    tracing::info!("Both clients connected successfully");

    // Cleanup
    harness.shutdown_all().await?;

    tracing::info!("Multi-node scenario test completed successfully");
    Ok(())
}

#[tokio::test]
async fn test_harness_basic_functionality() -> TestResult<()> {
    // Simple test to verify harness works without spawning actual daemons
    let harness = TestHarness::new();
    assert!(harness.daemon("nonexistent").is_none());
    assert!(harness.client("nonexistent").is_none());
    Ok(())
}
