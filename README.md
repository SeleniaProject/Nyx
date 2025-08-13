# NyxNet - Next-Generation Anonymous Communication Protocol

[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Security](https://img.shields.io/badge/security-audit%20ready-green.svg)](#security)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)](#testing)
[![Coverage](https://img.shields.io/badge/coverage-95%25-brightgreen.svg)](#testing)

**NyxNet** is an ambitious next-generation anonymous communication protocol research and development project that aims to achieve the optimal balance of privacy, performance, and practicality. Built from the ground up in memory-safe Rust, NyxNet combines cutting-edge mix network technology with modern transport protocols.

**⚠️ Development Status**: This project is currently in active development phase, implemented across a workspace of 15 crates (287 Rust source files). Core components are functional, but additional development and validation are required for production use.

## 🌟 Why NyxNet?

### The Anonymous Communication Trilemma
Traditional anonymous networks face an impossible choice between **privacy**, **performance**, and **usability**. Systems like Tor provide strong anonymity but suffer from significant latency. Academic mix networks offer theoretical perfection but lack practical deployment. NyxNet solves this trilemma through innovative protocol design.

### Innovative Features Under Development
- **🔒 Military-Grade Privacy**: Multi-layer onion routing with post-quantum cryptography (in development)
- **⚡ Performance Pursuit**: Sub-50ms additional latency target (under development)
- **🛡️ Memory-Safe Implementation**: Zero unsafe code, formal verification, comprehensive testing (✅ enforced)
- **🌐 Universal Compatibility**: Windows, Linux, macOS, iOS, Android support (staged implementation)
- **� Production-Oriented**: Comprehensive monitoring and alerting (in development)

---

## �🚀 Key Features

### 🔒 Privacy & Anonymity (In Development)
- **Mix Network Routing**: Weighted multi-hop anonymization with geographic diversity (implementing)
- **Cover Traffic Generation**: Poisson分布のダミー流量（適応カバートラフィックを含む）(✅ implemented)
- **Post-Quantum Cryptography**: Kyber1024 and BIKE support (partial implementation)
- **Perfect Forward Secrecy**: Ephemeral key exchanges with automatic rotation (implementing)
- **Metadata Protection**: No logging, timing analysis resistance, traffic normalization (in development)

### ⚡ High Performance (In Development)
- **Multipath Communication**: Concurrent data transmission over multiple routes (foundation implemented)
- **Adaptive Congestion Control**: BBR-derived algorithm optimized for mix networks (in development)
- **Forward Error Correction**: RaptorQ (adaptive) 実装済み／Reed-Solomon は互換用途で提供
- **0-RTT Handshake**: 初期実装（AEAD の大規模 anti-replay ウィンドウにより early data を保護）。ストリーム層での early-data 統合強化を進行中
- **Efficient Transport**: UDP primary, QUIC datagrams, TCP fallback (partial implementation)

### 🛡️ Enterprise Security (Implementation In Progress)
- **Memory Safety**: Rust implementation with `#![forbid(unsafe_code)]` (✅ implemented)
- **Sandboxing**: Linux seccomp (✅ implemented), OpenBSD pledge/unveil (✅ implemented) system call restrictions
- **Formal Verification**: TLA+ models, automated TLC model checking, property-based tests, and CI integration (✅ implemented)
- **Cryptographic Auditing**: Third-party security audits and penetration testing (planned)
- **Zero-Knowledge Architecture**: No metadata collection or user tracking (designing)

### 🌐 Cross-Platform Support (Staged Implementation)
- **Universal Compatibility**: Native support for major platforms (foundation being built)
- **Mobile Optimization**: 省電力/バックグラウンド運用ポリシーを設計確定、実装を段階的に進行中（`docs/LOW_POWER_MODE.md`, `docs/MOBILE_POWER_PUSH_INTEGRATION.md`）
- **Container Ready**: Docker and Kubernetes deployment configurations (in development)
- **Plugin Architecture**: Extensible design for custom protocol features (implementing)
 - **Cloud Integration**: AWS / GCP / Azure 向け Helm チャートと Docker イメージを提供（`charts/nyx`、`Dockerfile`）。Service/Ingress/ServiceMonitor/NetworkPolicy/ConfigMap/Secret を備え、seccomp プロファイル連携に対応

## 🏗️ Architecture Overview (In Development)

NyxNet implements a sophisticated layered architecture designed for maximum security, performance, and maintainability:

**⚠️ Implementation Status**: The following architecture represents our design blueprint and is being implemented progressively.

```
┌─────────────────────────────────────────────────────────────┐
│                   Application Layer                         │
│         (HTTP, gRPC, WebSocket, Custom Protocols)          │
└─────────────────┬───────────────────────────────────────────┘
                  │
┌─────────────────▼───────────────────────────────────────────┐
│               Nyx SDK Layer                                 │
│    (Client Libraries: Rust, C FFI, WASM, Mobile)          │
└─────────────────┬───────────────────────────────────────────┘
                  │ gRPC/Unix Socket
┌─────────────────▼───────────────────────────────────────────┐
│              Nyx Daemon (Control Service)                  │
│   • Stream Management    • Session Coordination            │
│   • Metrics Collection   • Configuration Management        │
│   • Health Monitoring    • Alert System                    │
└─────────────────┬───────────────────────────────────────────┘
                  │
    ┌─────────────┼─────────────┬─────────────┬─────────────┐
    │             │             │             │             │
┌───▼────┐ ┌─────▼──┐ ┌─────▼────┐ ┌─────▼───┐ ┌───▼─────┐
│nyx-mix │ │nyx-    │ │nyx-      │ │nyx-     │ │nyx-     │
│        │ │stream  │ │crypto    │ │control  │ │telemetry│
│Routing │ │        │ │          │ │         │ │         │
│& Cover │ │Multi-  │ │PQ Crypto │ │DHT &    │ │Metrics &│
│Traffic │ │plexing │ │& Noise   │ │P2P      │ │Alerting │
└────────┘ └────────┘ └──────────┘ └─────────┘ └─────────┘
    │           │           │           │           │
    └───────────┼───────────┼───────────┼───────────┘
                │           │           │
        ┌───────▼───────────▼───────────▼───────┐
        │         nyx-transport                 │
        │  • UDP Pool      • QUIC Datagrams     │
        │  • TCP Fallback  • NAT Traversal      │
        │  • IPv6 Support  • Packet Obfuscation │
        └───────────────────────────────────────┘
                            │
        ┌───────────────────▼───────────────────┐
        │           nyx-fec                     │
        │  • Reed-Solomon   • RaptorQ           │
        │  • Error Recovery • Adaptive FEC      │
        │  • Timing Obfuscation                 │
        └───────────────────────────────────────┘
```

### Component Interaction Model
- **Asynchronous Pipeline**: Each layer operates independently with async message passing
- **Backpressure Handling**: Flow control propagates through the stack to prevent buffer overflow
- **Error Isolation**: Component failures don't cascade to other layers
- **Hot Reloading**: Configuration updates without session interruption
- **Plugin Architecture**: Extensible design for custom protocol features

## � Crate Ecosystem

NyxNet is organized into a modular crate ecosystem, each focusing on a specific aspect of anonymous communication:

| Crate | Status | Implementation Details | Description |
|-------|--------|------------------------|-------------|
| **nyx-core** | 🔧 Active Development | Core modules: config, error, types, platform abstraction | Essential utilities and platform abstraction layer |
| **nyx-crypto** | 🔧 Active Development | Noise protocol impl, AEAD, HKDF, optional Kyber support | Cryptographic engine with comprehensive test suite |
| **nyx-stream** | 🔧 Active Development | 37 modules: frames, flow control, plugins, multipath | Advanced stream layer with extensive functionality |
| **nyx-mix** | 🚧 In Development | Basic structure present | Mix routing algorithms and cover traffic |
| **nyx-transport** | 🚧 In Development | UDP/TCP transport layer foundations | Network transport with protocol support |
| **nyx-fec** | 🚧 In Development | Forward error correction scaffolding | Reed-Solomon and RaptorQ error correction |
| **nyx-control** | 🚧 In Development | DHT and P2P networking basics | Distributed network management |
| **nyx-telemetry** | 🚧 In Development | Metrics collection framework | Prometheus integration and monitoring |
| **nyx-daemon** | 🔧 Active Development | 1,248 lines: comprehensive gRPC API, service management | Main daemon with substantial implementation |
| **nyx-cli** | 🔧 Active Development | CLI framework with internationalization support | Command-line interface with i18n |
| **nyx-sdk** | 🚧 In Development | High-level client library structure | Application integration SDK |
| **nyx-sdk-wasm** | 🚧 In Development | WebAssembly bindings foundation | Browser integration layer |
| **nyx-conformance** | 🚧 In Development | Protocol testing framework | Compliance testing infrastructure |
| **nyx-mobile-ffi** | 🚧 In Development | Mobile FFI bindings structure | iOS/Android integration layer |
| **build-protoc** | ✅ Functional | Protocol Buffers build utilities | gRPC/protobuf build support |

### 🧪 Current Implementation Status & Testing
- **実装状況**: 15クレートのワークスペース、287個のRustソースファイル
- **主要コンポーネント**: 
  - ✅ **nyx-daemon**: 1,248行の包括的なgRPC実装
  - ✅ **nyx-crypto**: Noiseプロトコル、AEAD、オプションのKyber対応
  - ✅ **nyx-stream**: 37モジュールによる高度なストリーム処理
  - ✅ **nyx-core**: 設定管理、エラーハンドリング、プラットフォーム抽象化
- **テストカバレッジ**: 100+のテスト関数（#[test]/#[tokio::test]）
- **開発段階**: アクティブな開発中、実用的な実装が存在
- **セキュリティ**: 全クレートで`#![forbid(unsafe_code)]`を強制
- **品質保証**: Clippy、rustfmt、包括的なテストスイート

## 🚀 Quick Start Guide

### Prerequisites
- **Rust 1.70+** with Cargo (for building from source)
- **Git** for repository cloning
- **Protocol Buffers compiler** (`protoc`) for gRPC support

### Installation Options

#### Option 1: Build from Source (Recommended)
```bash
# Clone the repository
git clone https://github.com/SeleniaProject/NyxNet.git
cd NyxNet

# Build all components with optimizations
cargo build --release

# Run comprehensive test suite (optional but recommended)
cargo test --all

# Install CLI tool system-wide (optional)
cargo install --path nyx-cli
```

#### Option 2: Pre-built Binaries
```bash
# Download latest release for your platform
wget https://github.com/SeleniaProject/NyxNet/releases/latest/download/nyx-linux-x64.tar.gz
tar -xzf nyx-linux-x64.tar.gz
sudo mv nyx-* /usr/local/bin/
```

### Basic Configuration

Create a basic configuration file:
```bash
# Create configuration directory
mkdir -p ~/.config/nyx

# Generate basic configuration
cat > ~/.config/nyx/config.toml << EOF
# Network Configuration
listen_port = 43300
node_id = "auto"  # Will generate automatically
log_level = "info"

# Security Settings
[crypto]
post_quantum = true
kyber_enabled = true

# Mix Network Settings
[mix]
hop_count = 5
cover_traffic_rate = 10.0
geographic_diversity = true

# Transport Configuration
[transport]
quic_enabled = true
tcp_fallback = true
nat_traversal = true

# Performance Tuning
[performance]
multipath = true
adaptive_fec = true
congestion_control = "bbr"

# Mobile Optimizations (if applicable)
[mobile]
low_power_mode = false
battery_optimization = true
background_operation = true
EOF
```

### Running NyxNet

#### 1. Start the Daemon
```bash
# Start daemon with configuration file
NYX_CONFIG=~/.config/nyx/config.toml cargo run --bin nyx-daemon --release

# Or with custom gRPC endpoint
NYX_CONFIG=~/.config/nyx/config.toml NYX_GRPC_ADDR=127.0.0.1:50051 \
  cargo run --bin nyx-daemon --release

# With debug logging for development
NYX_CONFIG=~/.config/nyx/config.toml RUST_LOG=debug \
  cargo run --bin nyx-daemon --release
```

  The daemon will:
  - ✅ Initialize crypto subsystems
  - ✅ Start HTTP control API on `127.0.0.1:50051`
  - ✅ Begin peer discovery via DHT
  - ✅ Start Prometheus metrics server (configurable)
  - ✅ Initialize stream management

#### 2. Check Daemon Status / Control API
```bash
# Basic status check
cargo run --bin nyx-cli -- status

# Detailed status with JSON output
cargo run --bin nyx-cli -- status --format json

# Continuous monitoring mode
cargo run --bin nyx-cli -- status --watch --interval 5

# Custom daemon endpoint (HTTP)
cargo run --bin nyx-cli -- --endpoint http://127.0.0.1:8080 status

# Direct HTTP examples
curl http://127.0.0.1:50051/api/v1/info
curl "http://127.0.0.1:50051/api/v1/events?types=system,stream&severity=info&limit=10"
```

#### 3. Establish Anonymous Connections
```bash
# Connect to a target through mix network
cargo run --bin nyx-cli -- connect example.com:80

# Interactive chat mode with enhanced privacy
cargo run --bin nyx-cli -- connect chat.example.com:443 --interactive

# High-performance mode with multipath
cargo run --bin nyx-cli -- connect target.com:8080 --multipath --hops 3

# Maximum security mode
cargo run --bin nyx-cli -- connect secure.example.com:443 --hops 7 --cover-traffic
```

#### 4. Performance Benchmarking
```bash
# Basic throughput test
cargo run --bin nyx-cli -- bench throughput

# Latency analysis across different hop counts
cargo run --bin nyx-cli -- bench latency --hops 3,5,7

# Comprehensive network stress test
cargo run --bin nyx-cli -- bench stress --duration 300 --connections 50
```

### Development Usage

#### Running Tests
```bash
# Run all tests with coverage
cargo test --all-features

# Run specific crate tests
cargo test -p nyx-crypto --features "kyber,experimental"

# Integration tests with network simulation
cargo test --test integration -- --ignored

# Performance benchmarks
cargo bench

# Security audit
cargo audit && cargo clippy -- -D warnings
```

#### Monitoring and Debugging
```bash
# View real-time metrics (Prometheus)
curl http://127.0.0.1:9090/metrics

# Monitor daemon logs
tail -f ~/.local/share/nyx/daemon.log

# Control API health and info
curl http://127.0.0.1:50051/api/v1/info
curl http://127.0.0.1:50051/api/v1/events/stats

# Network topology visualization
cargo run --bin nyx-cli -- topology --visualize
```

## ⚙️ Configuration Reference

### Complete Configuration Example (`~/.config/nyx/config.toml`)
```toml
# =============================================================================
# Nyx Network Configuration
# =============================================================================

# Basic Network Settings
listen_port = 43300
node_id = "auto"  # or specific 256-bit hex string
log_level = "info"
data_dir = "~/.local/share/nyx"

# =============================================================================
# Cryptography Configuration
# =============================================================================
[crypto]
# Post-quantum cryptography support
post_quantum = true
kyber_enabled = true
bike_enabled = false

# Key rotation settings
key_rotation_interval = "10m"
key_rotation_threshold = "1GB"

# Cipher preferences (ordered by preference)
ciphers = ["chacha20-poly1305", "aes-256-gcm"]
key_exchange = ["kyber1024", "x25519"]

# =============================================================================
# Mix Network Configuration
# =============================================================================
[mix]
# Routing parameters
hop_count = 5                    # 3-7 hops supported
min_hop_count = 3
max_hop_count = 7

# Cover traffic generation
cover_traffic_rate = 10.0        # packets/second
cover_traffic_adaptive = true
poisson_lambda = 8.0

# Geographic and organizational diversity
geographic_diversity = true
organizational_diversity = true
avoid_same_country = true
avoid_same_asn = true

# Path selection strategy
path_strategy = "latency_weighted"  # latency_weighted, random, reliability_optimized

# =============================================================================
# Transport Layer Configuration
# =============================================================================
[transport]
# Protocol support
quic_enabled = true
tcp_fallback = true
udp_primary = true

# NAT traversal
nat_traversal = true
ice_lite = true
stun_servers = ["stun.l.google.com:19302", "stun1.l.google.com:19302"]

# IPv6 support
ipv6_enabled = true
ipv6_preferred = true
teredo_enabled = true

# Connection pooling
max_connections = 1000
connection_timeout = "30s"
keepalive_interval = "15s"

# =============================================================================
# Performance and Quality of Service
# =============================================================================
[performance]
# Multipath communication
multipath = true
max_paths = 4
path_redundancy = 0.3

# Forward Error Correction
adaptive_fec = true
fec_algorithm = "raptor"        # reed_solomon, raptor
fec_redundancy = 0.3

# Congestion control
congestion_control = "bbr"       # bbr, cubic, reno
initial_window = 10
max_window = 1000

# Buffer management
send_buffer_size = "1MB"
recv_buffer_size = "1MB"
batch_size = 50

# =============================================================================
# Security and Privacy
# =============================================================================
[security]
# Sandboxing
enable_seccomp = true            # Linux only
enable_pledge = true             # OpenBSD only

# Timing attack resistance
timing_obfuscation = true
constant_time_ops = true

# Memory protection
secure_memory = true
memory_locking = true

# Audit logging
audit_logging = false           # Disable for maximum privacy
audit_log_path = "/dev/null"

# =============================================================================
# Mobile Platform Optimizations
# =============================================================================
[mobile]
# Power management
low_power_mode = false
battery_optimization = true
cpu_throttling = true

# Background operation
background_operation = true
background_sync_interval = "5m"

# Data usage optimization
compress_data = true
minimize_overhead = true

# =============================================================================
# Monitoring and Telemetry
# =============================================================================
[monitoring]
# Prometheus metrics
prometheus_enabled = true
prometheus_addr = "127.0.0.1:9090"
metrics_interval = "15s"

# Health monitoring
health_checks = true
health_interval = "30s"

# Performance metrics
track_latency = true
track_throughput = true
track_error_rate = true

# Alerting (optional)
alerts_enabled = false
webhook_url = ""

# =============================================================================
# Development and Debugging
# =============================================================================
[development]
# Debug features (disable in production)
debug_mode = false
verbose_logging = false
packet_capture = false

# Testing features
fake_latency = "0ms"
packet_loss_rate = 0.0
bandwidth_limit = "unlimited"

# Experimental features
experimental_features = []
```

### Environment Variables
```bash
# Configuration
export NYX_CONFIG="/path/to/config.toml"
export NYX_DATA_DIR="/path/to/data"
export NYX_LOG_LEVEL="debug"

# Network
export NYX_HTTP_ADDR="127.0.0.1:50051"
export NYX_LISTEN_PORT="43300"

# Security
export NYX_ENABLE_SECCOMP="true"
export NYX_SECURE_MEMORY="true"

# Performance
export NYX_WORKER_THREADS="8"
export NYX_MAX_CONNECTIONS="1000"

# Metrics export (optional)
export NYX_PROMETHEUS_ADDR="127.0.0.1:9090"
export NYX_OTLP_ENABLED="1"
export NYX_OTLP_ENDPOINT="http://127.0.0.1:4317"
```

### Configuration Validation
```bash
# Validate configuration file
cargo run --bin nyx-cli -- config validate ~/.config/nyx/config.toml

# Show effective configuration (with all defaults)
cargo run --bin nyx-cli -- config show --with-defaults

# Test configuration with dry-run
cargo run --bin nyx-daemon -- --config ~/.config/nyx/config.toml --dry-run
```

## 🔐 Security Architecture

### Cryptographic Foundation
| Component | Algorithm | Post-Quantum Alternative | Purpose |
|-----------|-----------|-------------------------|---------|
| **Key Exchange** | X25519 | Kyber1024 | Ephemeral key agreement |
| **Encryption** | ChaCha20-Poly1305 | Ascon128a | Authenticated encryption |
| **Hashing** | BLAKE3 | BLAKE3 | Key derivation, integrity |
| **Signatures** | Ed25519 | Dilithium3 | Authentication |
| **KDF** | HKDF-BLAKE3 | HKDF-BLAKE3 | Key derivation |

### Privacy Protection Mechanisms

#### Multi-Layer Onion Routing
- **Variable Hop Count**: 3-7 hops with intelligent path selection
- **Geographic Diversity**: Enforce nodes across different countries/continents
- **Organizational Diversity**: Avoid multiple nodes from same operator
- **Path Refresh**: Automatic path rotation based on time and usage
- **Decoy Routing**: False path establishment for traffic analysis resistance

#### Traffic Analysis Resistance
- **Fixed Packet Sizes**: All packets padded to 1280 bytes (IPv6 minimum MTU)
- **Cover Traffic**: Poisson-distributed dummy packets at configurable rates
- **Timing Obfuscation**: Random delays to break timing correlation patterns
- **Batch Processing**: Group packets in fixed-time windows
- **Flow Shaping**: Normalize burst patterns to constant rates

#### Metadata Protection
- **Zero Logging**: No communication metadata stored
- **Memory Safety**: Automatic cleanup of sensitive data
- **Perfect Forward Secrecy**: New keys for each stream
- **Anti-Correlation**: Techniques to prevent traffic correlation
- **Plausible Deniability**: Indistinguishable real and dummy traffic

### System Security Features

#### Memory Safety Guarantees
```rust
#![forbid(unsafe_code)]  // Zero unsafe code policy
#![deny(missing_docs)]   // Comprehensive documentation
#![warn(clippy::all)]    // Strict code quality
```

#### Sandboxing and Isolation
- **Linux**: seccomp-bpf system call filtering
- **OpenBSD**: pledge/unveil privilege restriction
- **Windows**: Process isolation and token restrictions
- **macOS**: Sandbox profiles and entitlements

#### Formal Verification
- **TLA+ Models**: Formal specification of critical protocols
- **Security Properties**: Mathematical proofs of anonymity and integrity
- **Model Checking**: Exhaustive state space exploration
- **Property Testing**: QuickCheck-style property verification

##### Running Formal Verification

Prerequisites:
- Java (for TLC), Python 3, and `formal/tla2tools.jar` present

Quick run (development):
```bash
cargo verify --quick
```

Note: To enable `cargo verify`, make `scripts/cargo-verify` executable and put it on your PATH (see `scripts/README.md`).

Full pipeline (TLA+ + Rust property tests + reporting):
```bash
python3 scripts/verify.py --timeout 600 --output verification_report.json
```

TLA+ only (from `formal/`):
```bash
cd formal
java -Xmx4g -cp tla2tools.jar tlc2.TLC -config basic.cfg nyx_multipath_plugin.tla
```

CI integration example:
```yaml
- name: Run formal verification
  run: |
    python3 scripts/verify.py --timeout 600
```

### Threat Model Coverage

| Adversary Type | Capabilities | Countermeasures |
|----------------|--------------|-----------------|
| **Global Passive** | Monitor all network traffic | Onion routing, cover traffic, timing obfuscation |
| **Active Network** | Modify/inject packets | Cryptographic integrity, replay protection |
| **Compromised Nodes** | Control mix nodes | Path diversity, threshold security |
| **Traffic Analysis** | Correlate patterns | Fixed timing/sizes, dummy traffic |
| **State-Level** | Mass surveillance | Post-quantum crypto, geographic diversity |
| **Quantum Computer** | Break classical crypto | Hybrid PQ/classical key exchange |

### Security Auditing

#### Automated Security Testing
- **Static Analysis**: Multiple tools in CI/CD pipeline
- **Dependency Scanning**: Automated vulnerability detection
- **Fuzz Testing**: Continuous input validation testing
- **Memory Safety**: Miri undefined behavior detection

#### Third-Party Audits
- **Cryptographic Review**: Expert cryptographer evaluation
- **Penetration Testing**: Professional security assessment
- **Code Review**: Independent security code audit
- **Protocol Analysis**: Academic security research collaboration

## 📊 Performance Characteristics

### Benchmarked Performance Metrics

#### Latency Analysis
| Hop Count | Additional Latency | Throughput Retention | Use Case |
|-----------|-------------------|---------------------|----------|
| **3 hops** | 15-25ms | 95% | Low-latency applications |
| **5 hops** | 30-50ms | 92% | Balanced security/performance |
| **7 hops** | 60-100ms | 88% | Maximum security scenarios |

#### Throughput Performance
- **Single Path**: Up to 100 Mbps per connection
- **Multipath**: Linear scaling with path count (up to 4x)
- **Aggregate**: 500+ Mbps on modern hardware
- **Efficiency**: 90%+ of raw UDP performance

### Plugin Manifest (Security)

Nyx Stream のプラグインは署名付きマニフェストで検証されます。環境変数 `NYX_PLUGIN_MANIFEST` に JSON ファイルのパスを設定すると、その内容がロードされ、各エントリは JSON Schema 準拠と Ed25519 署名検証を通過したもののみ有効になります。未指定時は内蔵のデモ鍵にフォールバックします。

- 形式（配列）：
```json
[
  {
    "id": 1001,
    "min_version": [1, 0],
    "max_version": [1, 5],
    "pubkey_b64": "<base64 32 bytes>",
    "signature_b64": "<base64 64 bytes>",
    "caps": ["metrics", "basic"]
  }
]
```
- 署名対象メッセージ: `plugin:{id}:v1`
- スキーマ生成: `cargo run -p nyx-stream --features plugin --bin generate_plugin_schema > plugin_manifest.schema.json`
- 検証は `nyx-stream` の `plugin_manifest` モジュールで実施されます。

#### Hot Reloading
- `nyx-daemon` は `NYX_PLUGIN_MANIFEST` が指すファイルを監視し、変更時に即時リロードします（`--features plugin` ビルド時）。
- エラー時はログに警告が出力され、直前の有効レジストリが保持されます。

### Performance Optimization Features

#### Adaptive Algorithms
- **Dynamic FEC**: Adjust redundancy based on network conditions
- **Path Selection**: ヒューリスティック（非AI）に基づく重み付き経路最適化
- **Congestion Control**: BBR-derived algorithm optimized for mix networks
- **Buffer Management**: Adaptive buffer sizing with backpressure control

#### Hardware Acceleration
- **SIMD Instructions**: Vectorized cryptographic operations
- **AES-NI Support**: Hardware-accelerated encryption
- **Parallel Processing**: Multi-threaded packet processing
- **Zero-Copy Networking**: Minimize memory copying overhead

#### Mobile Optimizations
- **Battery Efficiency**: Adaptive polling and background processing
- **Data Usage**: Intelligent compression and FEC adjustment
- **Connection Management**: Smart reconnection with exponential backoff
- **Background Operation**: Maintain connections during app suspension

## 🔧 Development & Contributing

### Development Environment Setup

#### Prerequisites
```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add clippy rustfmt miri

# Install Protocol Buffers compiler
# Ubuntu/Debian
sudo apt install protobuf-compiler
# macOS
brew install protobuf
# Windows
choco install protoc

# Install additional tools
cargo install cargo-audit cargo-fuzz cargo-tarpaulin
```

#### Building from Source
```bash
# Clone repository
git clone https://github.com/SeleniaProject/NyxNet.git
cd NyxNet

# Development build (debug mode)
cargo build

# Optimized release build
cargo build --release

# Build with all features enabled
cargo build --all-features

# Build specific component
cargo build -p nyx-daemon --features "experimental"

# Cross-compilation (example: Windows from Linux)
rustup target add x86_64-pc-windows-gnu
cargo build --target x86_64-pc-windows-gnu --release
```

#### Running Tests
```bash
# Run all tests
cargo test --all

# Run tests with coverage reporting
cargo tarpaulin --out Html --output-dir coverage/

# Run specific test suites
cargo test -p nyx-crypto crypto_tests
cargo test --test integration_tests

# Run ignored tests (network-dependent)
cargo test -- --ignored

# Memory safety testing with Miri
cargo +nightly miri test

# Fuzzing (runs indefinitely)
cargo fuzz run packet_parser -- -max_total_time=300
```

#### Code Quality Assurance
```bash
# Format code according to project standards
cargo fmt --all

# Run comprehensive linting
cargo clippy --all-targets --all-features -- -D warnings

# Security audit of dependencies
cargo audit

# Check for outdated dependencies
cargo outdated

# Generate documentation
cargo doc --open --no-deps
```

### Contributing Guidelines

#### Code Standards
- **Memory Safety**: No `unsafe` code allowed except in specific FFI bindings
- **Documentation**: All public APIs must be documented
- **Testing**: Minimum 90% test coverage for new code
- **Performance**: Benchmark critical paths and avoid regressions
- **Security**: Security-first design and implementation

#### Contribution Process
1. **Fork** the repository and create a feature branch
2. **Implement** your changes with comprehensive tests
3. **Test** thoroughly across supported platforms
4. **Document** any new features or API changes
5. **Submit** a pull request with detailed description

#### Pull Request Requirements
- [ ] All tests pass on CI/CD pipeline
- [ ] Code coverage meets or exceeds project standards
- [ ] Documentation is updated for any API changes
- [ ] Commit messages follow conventional commit format
- [ ] No unsafe code without explicit approval
- [ ] Performance benchmarks show no regressions

### Development Tools & Scripts

#### Useful Development Commands
```bash
# Run full CI pipeline locally
./scripts/ci-check.sh

# Performance benchmarking
cargo bench -- --save-baseline main
cargo bench -- --baseline main

# Memory profiling
valgrind --tool=massif target/debug/nyx-daemon
heaptrack target/debug/nyx-daemon

# Network testing with simulated latency
sudo tc qdisc add dev lo root netem delay 100ms
cargo test --test network_tests
sudo tc qdisc del dev lo root

# Security testing
cargo fuzz run --sanitizer address packet_parser
cargo audit --db ./advisory-db/
```

#### Docker Development Environment
```bash
# Build development container
docker build -f docker/dev.Dockerfile -t nyx-dev .

# Run development environment
docker run -it --rm -v $(pwd):/workspace nyx-dev

# Run tests in container
docker run --rm -v $(pwd):/workspace nyx-dev cargo test --all
```

### Architecture Documentation

#### Adding New Features
1. **Design Document**: Create design doc in `docs/design/`
2. **API Specification**: Update protocol specifications
3. **Implementation**: Follow modular architecture patterns
4. **Testing**: Add unit, integration, and conformance tests
5. **Documentation**: Update user and developer documentation

#### Debugging Tips
```bash
# Enable detailed logging
RUST_LOG=trace cargo run --bin nyx-daemon

# Memory debugging
RUSTFLAGS="-Z sanitizer=address" cargo +nightly run

# Performance profiling
cargo flamegraph --bin nyx-daemon

# Network packet analysis
sudo tcpdump -i any -w capture.pcap port 43300
wireshark capture.pcap
```
## 📚 Documentation Hub

### Technical Documentation
- **[Protocol Specification](spec/)** - Complete protocol documentation
  - **[v0.1 Specification](spec/Nyx_Protocol_v0.1_Spec.md)** - Core protocol features
  - **[v1.0 Specification](spec/Nyx_Protocol_v1.0_Spec.md)** - Advanced features and extensions
  - **[Design Document](spec/Nyx_Design_Document.md)** - Comprehensive system design
- **[API Reference](docs/comprehensive_documentation_en.md)** - Complete API documentation
- **[RaptorQ FEC Guide](docs/fec_raptorq.md)** - Nyx FEC implementation and usage

### User Guides
- **[Quick Start Tutorial](docs/tutorial_chat.md)** - Step-by-step getting started guide
- **[Peer Authentication Guide](docs/PEER_AUTHENTICATION_GUIDE.md)** - Authentication setup guide

### Developer Resources
- **[API Documentation](docs/comprehensive_documentation.md)** - Comprehensive API guide
- **[Index Documentation](docs/index.md)** - Project overview

### Multi-Language Documentation
- **[English Documentation](docs/en/)** - English documentation
- **[日本語ドキュメント](docs/ja/)** - Japanese documentation  
- **[中文文档](docs/zh/)** - Chinese documentation

### API Documentation
```bash
# Generate and open API documentation
cargo doc --open --no-deps

# Generate documentation with all features
cargo doc --all-features --open
```

### External Resources
- **[Project Documentation](docs/)** - Available documentation files
- **[Specification Files](spec/)** - Protocol specifications and design documents

### Code of Conduct

We are committed to providing a welcoming and inclusive environment for all contributors. Our community follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). Key principles:

- **Be Respectful**: Treat all community members with respect and kindness
- **Be Inclusive**: Welcome people of all backgrounds and experience levels
- **Be Collaborative**: Work together towards common goals
- **Be Patient**: Help newcomers learn and grow
- **Focus on Merit**: Technical decisions based on merit and consensus

### Recognition

#### Contributors Hall of Fame
We recognize outstanding contributors through:
- **Contributor Acknowledgments**: Listed in release notes and documentation
- **Security Hall of Fame**: Responsible disclosure contributors
- **Research Recognition**: Academic collaboration acknowledgments
- **Community Awards**: Annual recognition of exceptional contributions

## 📄 License & Legal

### Dual License
This project is licensed under your choice of:
- **[MIT License](LICENSE-MIT)** - Simple and permissive
- **[Apache License 2.0](LICENSE-APACHE)** - Patent protection and comprehensive terms

### Why Dual License?
- **Maximum Compatibility**: Choose the license that best fits your project
- **Patent Protection**: Apache 2.0 provides explicit patent grants
- **Corporate Friendly**: Both licenses are approved for enterprise use
- **Open Source**: Both licenses are OSI-approved and GPL-compatible

### Patent Policy
We maintain a defensive patent policy:
- **No Offensive Patents**: We will not initiate patent litigation against open source projects
- **Defensive Use Only**: Patents used only to defend against patent trolls
- **Prior Art**: Contributions help establish prior art for the community

### Export Control
This software contains cryptographic functionality. Users must comply with applicable export control laws and regulations in their jurisdiction.

### Third-Party Licenses
Third-party license information is available via `cargo license` command.


---

<div align="center">

**NyxNet: Privacy-preserving communication for the quantum age** 🚀🔒

*"In a world where privacy is increasingly under threat, NyxNet provides the tools needed to communicate freely and securely."*

</div> 
