@echo off
REM This script contains explicit git add/commit pairs per file.
REM Generated to reflect current pending changes. You can edit messages before running.

REM NOTE: Run this file from the repo root

git add build-protoc/build.rs
git commit -m "chore: update build-protoc/build.rs"

git add docs/SANDBOX_WINDOWS.md
git commit -m "docs: update SANDBOX_WINDOWS.md"

git add docs/SDK_WASM_FEATURE_MATRIX.md
git commit -m "docs: update SDK_WASM_FEATURE_MATRIX.md"

git add nyx-conformance/tests/aead_comprehensive.rs
git commit -m "tests(conformance): update aead_comprehensive.rs"

git add nyx-conformance/tests/network_simulation_properties.proptest-regressions
git commit -m "tests(conformance): update network_simulation_properties.proptest-regressions"

git add nyx-conformance/tests/raptorq_prop.proptest-regressions
git commit -m "tests(conformance): update raptorq_prop.proptest-regressions"

git add nyx-control/Cargo.toml
git commit -m "chore(control): update Cargo.toml"

git add nyx-core/Cargo.toml
git commit -m "chore(core): update Cargo.toml"

git add nyx-core/src/advanced_routing.rs
git commit -m "core: update advanced_routing.rs"

git add nyx-core/src/multipath_dataplane.rs
git commit -m "core: update multipath_dataplane.rs"

git add nyx-core/src/plugin_framework.rs
git commit -m "core: update plugin_framework.rs"

git add nyx-crypto/src/aead.rs
git commit -m "crypto: update aead.rs"

git add nyx-daemon/Cargo.toml
git commit -m "chore(daemon): update Cargo.toml"

git add nyx-daemon/src/alert_system.rs
git commit -m "daemon: update alert_system.rs"

git add nyx-daemon/src/alert_system_enhanced.rs
git commit -m "daemon: update alert_system_enhanced.rs"

git add nyx-daemon/src/alert_system_test.rs
git commit -m "daemon: update alert_system_test.rs"

git add nyx-daemon/src/health_monitor.rs
git commit -m "daemon: update health_monitor.rs"

git add nyx-daemon/src/libp2p_network.rs
git commit -m "daemon: update libp2p_network.rs"

git add nyx-daemon/src/main.rs
git commit -m "daemon: update main.rs"

git add nyx-daemon/src/metrics.rs
git commit -m "daemon: update metrics.rs"

git add nyx-daemon/src/path_builder.rs
git commit -m "daemon: update path_builder.rs"

git add nyx-daemon/src/path_builder/integration_tests.rs
git commit -m "daemon: update path_builder/integration_tests.rs"

git add nyx-daemon/src/path_builder/tests.rs
git commit -m "daemon: update path_builder/tests.rs"

git add nyx-daemon/src/proto.rs
git commit -m "daemon: update proto.rs"

git add nyx-daemon/src/pure_rust_dht_tcp.rs
git commit -m "daemon: update pure_rust_dht_tcp.rs"

git add nyx-sdk/src/daemon.rs
git commit -m "sdk: update daemon.rs"

git add nyx-sdk/src/reconnect.rs
git commit -m "sdk: update reconnect.rs"

git add nyx-sdk/src/retry.rs
git commit -m "sdk: update retry.rs"

git add nyx-sdk/src/stream.rs
git commit -m "sdk: update stream.rs"

git add nyx-stream/src/plugin_sandbox.rs
git commit -m "stream: update plugin_sandbox.rs"

git add nyx-stream/src/plugin_sandbox_macos.rs
git commit -m "stream: update plugin_sandbox_macos.rs"

git add nyx-stream/src/plugin_sandbox_windows.rs
git commit -m "stream: update plugin_sandbox_windows.rs"

git add nyx-telemetry/Cargo.toml
git commit -m "chore(telemetry): update Cargo.toml"

git add nyx-telemetry/src/lib.rs
git commit -m "telemetry: update lib.rs"

echo All commits attempted. Review git log for results.
exit /b 0


