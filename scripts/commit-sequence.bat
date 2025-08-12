@echo off
REM This script contains explicit git add/commit pairs per file.
REM Generated to reflect current pending changes. You can edit messages before running.

REM NOTE: Run this file from the repo root

git add build-protoc/build.rs
git commit -m "build(build-protoc): refine protoc build script configuration"

git add docs/SANDBOX_WINDOWS.md
git commit -m "docs(sandbox): update Windows Job Object limits and implementation status"

git add docs/SDK_WASM_FEATURE_MATRIX.md
git commit -m "docs(wasm): refresh SDK WASM feature matrix and notes"

git add nyx-conformance/tests/aead_comprehensive.rs
git commit -m "test(conformance): extend AEAD comprehensive vectors and assertions"

git add nyx-conformance/tests/network_simulation_properties.proptest-regressions
git commit -m "test(conformance): refresh proptest corpus for network simulation properties"

git add nyx-conformance/tests/raptorq_prop.proptest-regressions
git commit -m "test(conformance): refresh proptest corpus for RaptorQ properties"

git add nyx-control/Cargo.toml
git commit -m "chore(control): sync dependencies and feature flags"

git add nyx-core/Cargo.toml
git commit -m "chore(core): align feature gates (zero_copy/telemetry) and deps"

git add nyx-core/src/advanced_routing.rs
git commit -m "core(routing): minor adjustments to advanced routing utilities"

git add nyx-core/src/multipath_dataplane.rs
git commit -m "core(multipath): dataplane improvements and cleanup"

git add nyx-core/src/plugin_framework.rs
git commit -m "core(plugin): clarify framework integration points and types"

git add nyx-crypto/src/aead.rs
git commit -m "crypto(aead): harmonize API with zero-copy integration and tighten errors"

git add nyx-daemon/Cargo.toml
git commit -m "chore(daemon): sync dependencies/features and workspace versions"

git add nyx-daemon/src/alert_system.rs
git commit -m "daemon(alerts): refine alert emission and thresholds"

git add nyx-daemon/src/alert_system_enhanced.rs
git commit -m "daemon(alerts): improve enhanced alerting paths and metrics"

git add nyx-daemon/src/alert_system_test.rs
git commit -m "test(daemon): update alert system tests"

git add nyx-daemon/src/health_monitor.rs
git commit -m "daemon(health): refine disk/memory checks and reporting"

git add nyx-daemon/src/libp2p_network.rs
git commit -m "daemon(p2p): tidy network handler and events"

git add nyx-daemon/src/main.rs
git commit -m "daemon(main): integrate recent subsystems and wiring"

git add nyx-daemon/src/metrics.rs
git commit -m "daemon(metrics): refine counters, gauges, and export formatting"

git add nyx-daemon/src/path_builder.rs
git commit -m "daemon(path): improve path builder logic and error handling"

git add nyx-daemon/src/path_builder/integration_tests.rs
git commit -m "test(daemon): update path builder integration tests"

git add nyx-daemon/src/path_builder/tests.rs
git commit -m "test(daemon): update path builder unit tests"

git add nyx-daemon/src/proto.rs
git commit -m "daemon(proto): sync generated/service types with implementation"

git add nyx-daemon/src/pure_rust_dht_tcp.rs
git commit -m "daemon(dht): improve pure Rust DHT TCP behavior and IDs"

git add nyx-sdk/src/daemon.rs
git commit -m "sdk(daemon): update client daemon wiring and config"

git add nyx-sdk/src/reconnect.rs
git commit -m "sdk(reconnect): tune reconnect policy and timing"

git add nyx-sdk/src/retry.rs
git commit -m "sdk(retry): refine retry backoff and limits"

git add nyx-sdk/src/stream.rs
git commit -m "sdk(stream): minor stream handling updates"

git add nyx-stream/src/plugin_sandbox.rs
git commit -m "stream(plugin-sandbox): tighten sandbox config and checks"

git add nyx-stream/src/plugin_sandbox_macos.rs
git commit -m "stream(plugin-sandbox): update macOS sandbox profile and launcher"

git add nyx-stream/src/plugin_sandbox_windows.rs
git commit -m "stream(plugin-sandbox): update Windows job object restrictions"

git add nyx-telemetry/Cargo.toml
git commit -m "chore(telemetry): sync deps and features"

git add nyx-telemetry/src/lib.rs
git commit -m "telemetry: refine metrics interfaces and integration hooks"

echo All commits attempted. Review git log for results.
exit /b 0


