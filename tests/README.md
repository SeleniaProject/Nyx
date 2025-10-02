# NyxNet Integration Tests

This crate contains integration tests for the NyxNet privacy network protocol stack.

## Test Structure

### Integration Tests (`src/integration/`)

- **multipath_transfer.rs**: Tests concurrent data transfer over multiple network paths with failover capabilities
- **cover_traffic_ratio.rs**: Validates adaptive cover traffic generation under various power modes
- **network_simulation.rs**: Verifies network condition simulation (latency, jitter, packet loss)
- **stress_test.rs**: System resilience under heavy load (concurrent connections, high throughput, sustained traffic)

### Test Harness (`src/test_harness.rs`)

Provides infrastructure for daemon lifecycle management and network simulation:

- `TestHarness`: Manages daemon instances and test network configuration
- `NetworkConfig`: Configurable network conditions (latency, jitter, packet loss)
- `DaemonHandle`/`ClientHandle`: Process lifecycle management

## Running Tests

### Local Development

```bash
# Install cargo-nextest (one-time setup)
cargo install cargo-nextest --locked

# Run non-daemon-dependent tests (e.g., network simulation)
cd tests
cargo nextest run --lib

# Run all tests including daemon-dependent ones (requires nyx-daemon)
cargo nextest run --lib --run-ignored ignored-only

# Use CI profile for extended timeouts and retries
cargo nextest run --lib --profile ci
```

**Alternative (without nextest)**:
```bash
cd tests
cargo test --lib
cargo test --lib -- --ignored --nocapture
```

### CI Environment

Integration tests are automatically executed via GitHub Actions:

```bash
# Trigger workflow manually
gh workflow run integration-tests.yml
```

The workflow performs:
1. Install cargo-nextest
2. Build nyx-daemon in release mode
3. Run non-daemon-dependent tests (using nextest with CI profile)
4. Start nyx-daemon background process
5. Execute daemon-dependent tests (marked `#[ignore]`, using nextest)
6. Generate JUnit XML reports for CI integration
7. Collect logs and artifacts

## Test Requirements

### System Dependencies

- **Linux**: libcap-dev, pkg-config, protobuf-compiler
- **Windows**: No additional dependencies (native build)
- **macOS**: protobuf (via Homebrew)

### Runtime Requirements

For daemon-dependent tests:
- nyx-daemon binary (built via `cargo build -p nyx-daemon --release`)
- Valid `nyx.toml` configuration file
- TCP ports available for daemon communication

## Test Configuration

### Network Conditions

`NetworkConfig` provides presets for common scenarios:

```rust
NetworkConfig::good()      // 20ms latency, 2ms jitter, 0.1% loss
NetworkConfig::poor()      // 100ms latency, 20ms jitter, 2% loss
NetworkConfig::unstable()  // 150ms latency, 50ms jitter, 5% loss
```

### Test Timeouts

- **Non-daemon tests**: ~10 seconds
- **Daemon-dependent tests**: 5-60 seconds per test
- **CI timeout**: 60 minutes total

## Artifacts

CI runs generate:

- **Test logs**: `test-artifacts/*.log`
- **Daemon binary**: `target/release/nyx-daemon`
- **Test summary**: `test-artifacts/summary.txt`
- **JUnit XML**: `tests/target/nextest/ci/junit.xml` (retained for 30 days)

Artifacts are retained for 7 days (30 days for JUnit XML) in GitHub Actions.

## Troubleshooting

### Daemon fails to start

Check:
- `nyx.toml` exists in workspace root
- TCP ports are not in use
- Sufficient permissions (Linux: CAP_NET_ADMIN for raw sockets)

### Tests hang indefinitely

- Increase timeout in test annotations: `#[tokio::test(flavor = "multi_thread")]`
- Check for deadlocks in daemon communication

### High test flakiness

- Adjust network simulation parameters (increase latency tolerance)
- Use larger sample sizes for statistical tests
- Enable `RUST_LOG=debug` for detailed tracing

## Contributing

When adding new integration tests:

1. Follow existing test structure (setup → action → assertion → cleanup)
2. Use `#[ignore]` for daemon-dependent tests
3. Add English comments for complex logic
4. Verify tests pass in CI before merging
5. Update this README with new test descriptions

## License

See workspace-level LICENSE files (Apache-2.0 OR MIT).
