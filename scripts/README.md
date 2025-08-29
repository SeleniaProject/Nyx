# Nyx Network - Simple Setup Scripts

完璧なワンライナーでKubernetesクラスタ + Nyxデプロイ + マルチノードベンチマーク！

## 🚀 Ubuntu/Linux クイックスタート

```bash
# 1. セットアップ（初回のみ）
curl -fsSL https://github.com/SeleniaProject/Nyx/raw/main/scripts/nyx-setup.sh | bash

# 2. デプロイ＆ベンチマーク実行
curl -fsSL https://github.com/SeleniaProject/Nyx/raw/main/scripts/nyx-deploy.sh | bash
```

または、リポジトリクローン後：
```bash
./scripts/nyx-setup.sh    # 初回セットアップ
./scripts/nyx-deploy.sh   # デプロイ＆ベンチマーク
```

## 💻 Windows クイックスタート

管理者権限でPowerShellを開いて：

```powershell
# 1. セットアップ（初回のみ）
.\scripts\nyx-setup.bat

# 2. デプロイ＆ベンチマーク実行  
.\scripts\nyx-deploy.bat
```

## 📊 実行内容

### セットアップスクリプト
- Docker/Docker Desktop インストール
- kubectl インストール  
- Helm インストール
- kind インストール

### デプロイスクリプト
- マルチノードkindクラスタ作成（1 CP + 3 Worker）
- Prometheus Operator インストール
- Nyx 6レプリカデプロイ
- 3並列ベンチマークJob実行
- **完全な性能評価レポート表示**

## 🎯 ベンチマーク内容

- **マルチノード接続マトリックス**テスト
- **ロードバランシング**検証（50回テスト）
- **同時接続ストレス**テスト（15並列接続）
- **スループット**測定（全Pod間）
- **リソース使用量**監視
- **ネットワーク復旧力**テスト

## 🏆 性能評価

- 🥇 **EXCELLENT**: 90%+ 接続成功 + 80%+ LB成功
- 🥈 **GOOD**: 70%+ 接続成功 + 60%+ LB成功  
- 🥉 **NEEDS IMPROVEMENT**: 改善要

## 🛠️ トラブルシューティング

### Docker関連
```bash
# Docker起動
sudo systemctl start docker

# Docker権限
sudo usermod -aG docker $USER
# ログアウト・ログイン後に再実行
```

### クラスタリセット
```bash
# 完全クリーンアップ
./scripts/nyx-cleanup.sh

# 再デプロイ
./scripts/nyx-deploy.sh
```

**Windows:**
```powershell
# 完全クリーンアップ
.\scripts\nyx-cleanup.bat

# 再デプロイ
.\scripts\nyx-deploy.bat
```

## ✅ U22プログラミングコンテスト対応

すべての要素が完璧に統合され、本格的な分散システム性能評価が可能です！

- 🚀 **ワンライナー簡単実行**
- 🏗️ **マルチノードクラスタ自動構築**  
- 📊 **包括的性能ベンチマーク**
- 🔒 **プロダクション品質セキュリティ**
- 📈 **自動性能評価システム**

---

# 元のTLA+検証スクリプト

以下は元のプロトコル検証スクリプトです：

**Usage:**
```bash
python3 scripts/verify.py [options]
```

**Options:**
- `--timeout SECONDS`: Verification timeout (default: 600)
- `--java-opts OPTS`: Java options for TLA+ (default: "-Xmx4g")
- `--tla-only`: Run only TLA+ model checking
- `--rust-only`: Run only Rust property tests
- `--output FILE`: Output file for report (default: "verification_report.json")

**Example:**
```bash
# Full verification pipeline
python3 scripts/verify.py --timeout 900 --output full_report.json

# TLA+ only with more memory
python3 scripts/verify.py --tla-only --java-opts "-Xmx8g"

# Rust property tests only
python3 scripts/verify.py --rust-only
```

#### `generate-verification-report.py`
Generates comprehensive verification reports with coverage metrics and requirement traceability.

**Usage:**
```bash
python3 scripts/generate-verification-report.py verification_report.json [options]
```

**Options:**
- `--output FILE`: Output JSON report (default: "verification_coverage_report.json")
- `--html FILE`: Generate HTML report
- `--project-root DIR`: Project root directory (default: ".")

**Example:**
```bash
# Generate comprehensive report with HTML output
python3 scripts/generate-verification-report.py verification_report.json \
    --output coverage_report.json \
    --html verification_report.html
```

### Build Integration Scripts

#### `build-verify.sh` (Linux/macOS)
Build script that integrates verification with the Rust build process.

**Usage:**
```bash
./scripts/build-verify.sh [options]
```

**Options:**
- `--skip-tla`: Skip TLA+ model checking
- `--skip-rust`: Skip Rust property tests
- `--timeout SECONDS`: Verification timeout
- `--java-opts OPTS`: Java options for TLA+

**Environment Variables:**
- `VERIFICATION_TIMEOUT`: Verification timeout in seconds
- `JAVA_OPTS`: Java options for TLA+ model checking
- `SKIP_TLA`: Skip TLA+ model checking (true/false)
- `SKIP_RUST`: Skip Rust property tests (true/false)

#### `build-verify.ps1` (Windows)
PowerShell version of the build script for Windows compatibility.

**Usage:**
```powershell
.\scripts\build-verify.ps1 [options]

#### `run-hybrid-tests.ps1` (Windows) / `run-hybrid-tests.sh` (Linux/macOS)

nyx-crypto のハイブリッド・ハンドシェイク関連テストを素早く再現するためのヘルパーです。OS に応じて PowerShell 版/ Bash 版を使えます。

使い方例:

- すべて実行

  ```powershell
  .\scripts\run-hybrid-tests.ps1
  ```

- テスト名でフィルタ（PowerShell のワイルドカード不要、cargo のパターン一致）

  ```powershell
  .\scripts\run-hybrid-tests.ps1 -Filter test_key_pair_generation
  .\scripts\run-hybrid-tests.ps1 -Filter test_complete_handshake_protocol
  ```

Linux/macOS の場合:

```bash
./scripts/run-hybrid-tests.sh [Filter]
```

内部的には以下を実行します（共通）:

```powershell
cargo test -p nyx-crypto --features hybrid-handshake [Filter] -- --nocapture
```
```

**Options:**
- `-SkipTla`: Skip TLA+ model checking
- `-SkipRust`: Skip Rust property tests
- `-Timeout SECONDS`: Verification timeout
- `-JavaOpts OPTS`: Java options for TLA+

### Cargo Integration

#### `cargo-verify`
Custom Cargo command for running formal verification.

**Installation:**
```bash
# Make executable
chmod +x scripts/cargo-verify

# Add to PATH or create symlink
ln -s $(pwd)/scripts/cargo-verify ~/.cargo/bin/cargo-verify
```

**Usage:**
```bash
cargo verify [options]
```

**Options:**
- `--tla-only`: Run only TLA+ model checking
- `--rust-only`: Run only Rust property tests
- `--timeout SECONDS`: Verification timeout
- `--quick`: Run quick verification (basic TLA+ only)
- `--html-report FILE`: Generate HTML report

**Examples:**
```bash
# Full verification
cargo verify

# Quick verification for development
cargo verify --quick

# TLA+ only with HTML report
cargo verify --tla-only --html-report tla_report.html

# Rust property tests with extended timeout
cargo verify --rust-only --timeout 1200
```

## Verification Pipeline Architecture

### 1. TLA+ Model Checking
- **Model**: `formal/nyx_multipath_plugin.tla`
- **Configurations**: Multiple TLC configurations for different scenarios
- **Properties**: Safety invariants and liveness properties
- **Output**: State exploration statistics and counterexamples

### 2. Rust Property-Based Testing
- **Location**: `nyx-conformance/tests/`
- **Framework**: Proptest for property-based testing
- **Coverage**: Protocol state machine, multipath selection, capability negotiation, cryptographic operations, network simulation
- **Output**: Test results and property violation examples

### 3. Coverage Analysis
- **Requirements Traceability**: Maps verification results to formal requirements
- **Code Coverage**: Rust code coverage analysis (with cargo-tarpaulin if available)
- **TLA+ Coverage**: Model checking configuration and property coverage
- **Integration Coverage**: Cross-verification between TLA+ and Rust tests

### 4. Reporting System
- **JSON Reports**: Machine-readable verification results
- **HTML Reports**: Human-readable coverage reports with visualizations
- **CI Integration**: GitHub Actions workflow integration
- **Metrics**: Composite scoring system for verification quality

## CI/CD Integration

### GitHub Actions Workflows

#### `formal-verification.yml`
Comprehensive formal verification workflow that runs on:
- Push to main/develop branches
- Pull requests
- Weekly schedule
- Manual dispatch

**Features:**
- Multi-platform testing (Ubuntu, Windows)
- Parallel TLA+ and Rust verification
- Comprehensive reporting
- Artifact collection
- Status checks

#### `tla-ci.yml`
Lightweight TLA+ model checking for quick feedback:
- Runs on formal/ directory changes
- Basic model checking with caching
- Fast feedback for TLA+ model changes

### Integration with Existing CI

The verification pipeline integrates with existing CI workflows:

```yaml
# Add to existing CI workflow
- name: Run formal verification
  run: |
    python3 scripts/verify.py --timeout 600
  continue-on-error: false

- name: Upload verification results
  uses: actions/upload-artifact@v4
  with:
    name: verification-results
    path: verification_report.json
```

## Development Workflow

### 1. Local Development
```bash
# Quick verification during development
cargo verify --quick

# Full verification before commit
cargo verify --html-report verification.html
```

### 2. Pre-commit Hooks
```bash
# Add to .git/hooks/pre-commit
#!/bin/bash
echo "Running formal verification..."
if ! python3 scripts/verify.py --timeout 300; then
    echo "Formal verification failed. Commit aborted."
    exit 1
fi
```

### 3. Release Process
```bash
# Comprehensive verification for releases
./scripts/build-verify.sh --timeout 1800
python3 scripts/generate-verification-report.py build_verification_report.json \
    --html release_verification_report.html
```

## Configuration

### Environment Variables
- `VERIFICATION_TIMEOUT`: Default timeout for verification steps
- `JAVA_OPTS`: Java options for TLA+ model checking
- `SKIP_TLA`: Skip TLA+ model checking in build scripts
- `SKIP_RUST`: Skip Rust property tests in build scripts
- `PROPTEST_CASES`: Number of property test cases to run
- `PROPTEST_RNG_SEED`: Seed for property test randomization

### TLA+ Configuration
TLA+ model checking configurations are in `formal/`:
- `basic.cfg`: Quick smoke test (30s)
- `comprehensive.cfg`: Full verification (5-10min)
- `scalability.cfg`: Large-scale testing (15-30min)
- `capability_stress.cfg`: Capability negotiation stress test (10-20min)
- `liveness_focus.cfg`: Temporal properties focus (2-5min)

### Rust Test Configuration
Property-based tests in `nyx-conformance/tests/`:
- `multipath_selection_properties.rs`: Path generation and validation
- `capability_negotiation_properties.rs`: Handshake protocol verification
- `protocol_state_machine_properties.rs`: State transition testing
- `cryptographic_operation_properties.rs`: Cryptographic property verification
- `network_simulation_properties.rs`: End-to-end protocol behavior

## Troubleshooting

### Common Issues

#### TLA+ Model Checking Fails
```bash
# Check Java version and memory
java -version
java -Xmx4g -version

# Run with verbose output
cd formal
java -Xmx4g -cp tla2tools.jar tlc2.TLC -config basic.cfg nyx_multipath_plugin.tla
```

#### Rust Property Tests Fail
```bash
# Run specific test with verbose output
cd nyx-conformance
cargo test multipath_selection_properties --verbose

# Run with specific seed for reproducibility
PROPTEST_RNG_SEED=42 cargo test
```

#### Verification Pipeline Hangs
```bash
# Check for deadlocks or infinite loops
ps aux | grep -E "(java|cargo|python)"

# Kill hanging processes
pkill -f "tlc2.TLC"
pkill -f "cargo test"
```

#### Memory Issues
```bash
# Increase Java heap size
export JAVA_OPTS="-Xmx8g"

# Monitor memory usage
htop
```

### Performance Optimization

#### TLA+ Model Checking
- Increase Java heap size for larger models
- Use symmetry reduction in TLC configurations
- Parallelize model checking with multiple workers
- Cache TLA+ tools jar file

#### Rust Property Testing
- Adjust `PROPTEST_CASES` for test thoroughness vs. speed
- Use release builds for property tests
- Parallelize test execution with `cargo test --jobs N`

#### CI/CD Optimization
- Cache dependencies and build artifacts
- Use matrix builds for parallel execution
- Optimize artifact collection and storage
- Use conditional workflows for relevant changes

## Metrics and Reporting

### Verification Metrics
- **Composite Score**: Weighted average of all verification aspects
- **Verification Success Rate**: Percentage of successful verifications
- **Requirements Coverage**: Percentage of requirements covered by verification
- **TLA+ Coverage**: Percentage of model properties verified
- **Code Coverage**: Percentage of Rust code covered by tests

### Grading System
- **A+ (95-100%)**: Excellent verification coverage
- **A (90-94%)**: Very good verification coverage
- **A- (85-89%)**: Good verification coverage
- **B+ (80-84%)**: Acceptable verification coverage
- **B (75-79%)**: Needs improvement
- **C+ (65-74%)**: Significant gaps in verification
- **C (60-64%)**: Major verification issues
- **F (<60%)**: Inadequate verification coverage

### Report Formats
- **JSON**: Machine-readable for CI/CD integration
- **HTML**: Human-readable with visualizations
- **Console**: Quick feedback during development
- **GitHub Actions Summary**: Integrated CI/CD reporting

## Contributing

### Adding New Verification Tests

#### TLA+ Model Extensions
1. Extend `formal/nyx_multipath_plugin.tla` with new properties
2. Add corresponding TLC configuration file
3. Update `scripts/verify.py` to include new configuration
4. Add requirement mapping in report generator

#### Rust Property Tests
1. Create new test file in `nyx-conformance/tests/`
2. Implement property-based tests using proptest
3. Update `scripts/verify.py` to run new test category
4. Add requirement traceability mapping

#### Verification Pipeline Enhancements
1. Extend `VerificationPipeline` class in `scripts/verify.py`
2. Add new metrics to coverage analyzer
3. Update report generator with new visualizations
4. Add CI/CD workflow integration

### Code Style and Standards
- Follow Python PEP 8 for Python scripts
- Use type hints for better code documentation
- Add comprehensive docstrings for all functions
- Include error handling and logging
- Write unit tests for verification pipeline components

## License

This verification infrastructure is part of the Nyx protocol project and follows the same licensing terms.