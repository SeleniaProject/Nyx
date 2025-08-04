# Nyx Protocol v1.0 実装チェックリスト

## 🔴 仕様書未対応・未実装機能 (Critical)

### 1. Plugin Framework (v1.0新機能)
- [ ] Frame Type 0x50-0x5F のPlugin予約領域実装
- [ ] CBOR ヘッダ `{id:u32, flags:u8, data:bytes}` パーサー
- [ ] SETTINGS `PLUGIN_REQUIRED` advertising 機能
- [ ] Plugin 向け handshake メカニズム
- [ ] Plugin IPC transport 配線 (現在はプレースホルダーのみ)

### 2. Multipath Data Plane (v1.0新機能) 
- [ ] パケットヘッダの `PathID` (uint8) フィールド追加
- [ ] Weighted Round Robin スケジューラ (weight = inverse RTT)
- [ ] Per-path reordering buffer (RTT diff + jitter *2)
- [ ] 動的ホップ数 (3-7) 実装 (現在は固定5ホップ)
- [ ] 複数パス同時通信の統合

### 3. Hybrid Post-Quantum Handshake (v1.0新機能) ✅ 完成
- [x] Kyber1024 + X25519 ハイブリッド実装 ✅
- [x] BIKE サポート (PQ-Only モード) ✅
- [ ] HPKE (RFC 9180) 統合 (準備完了)
- [x] ee_kyber, se_kyber handshake 拡張 ✅  
- [x] HKDF-Extract(SHA-512, concat(dh25519, kyber)) 実装 ✅

### 4. cMix Integration (v1.0新機能)
- [ ] `mode=cmix` オプション実装
- [ ] batch = 100, VDF delay 100ms 処理
- [ ] RSA accumulator 証明機構
- [ ] VDF-based batch processing

### 5. RaptorQ FEC (v1.0新機能)
- [ ] Reed-Solomon (255,223) からRaptorQへの切り替え
- [ ] Adaptive redundancy 機能
- [ ] 現在は固定冗長率のみ実装

### 6. QUIC DATAGRAM + TCP Fallback (v1.0新機能)
- [ ] QUIC DATAGRAM サポート
- [ ] TCP encapsulation fallback
- [ ] IPv6 Teredo 内蔵実装

### 7. Low Power Mode (モバイル向け v1.0新機能)
- [ ] Screen-Off 検知機能
- [ ] `cover_ratio=0.1` 低電力モード
- [ ] FCM/APNS WebPush over Nyx Gateway
- [ ] Push notification 経路実装

### 8. OpenTelemetry Tracing (v1.0新機能)
- [ ] OTLP span "nyx.stream.send" 実装
- [ ] path_id, cid 属性追加
- [ ] 分散トレーシング統合

## 🟡 プレースホルダー・スタブ実装 (High Priority)

### Core Components
- [ ] `nyx-transport/lib.rs:38` - QuicEndpoint スタブ実装
- [ ] `nyx-transport/lib.rs:184` - UDP hole-punching スタブ
- [ ] `nyx-control/lib.rs:61` - DhtCmd::Stub 実装
- [ ] `nyx-control/push.rs:115-116` - PASETO トークン生成プレースホルダー
- [ ] `nyx-core/sandbox.rs:8,58` - サンドボックス機能プレースホルダー

### JSON/YAML Serialization
- [ ] `nyx-cli/main.rs:1570` - NodeInfo JSON シリアライゼーション
- [ ] `nyx-cli/main.rs:1575` - NodeInfo YAML シリアライゼーション

### Daemon 関連
- [ ] `nyx-daemon/pure_rust_dht_tcp.rs:1919-1921` - backup/restore timestamps実装
- [ ] `nyx-daemon/pure_rust_dht_tcp.rs:1921` - compression 機能実装
- [ ] `nyx-daemon/pure_rust_p2p.rs:562,567,608,684,1127` - P2P プレースホルダー実装

### Stream 管理
- [ ] `nyx-stream/state.rs:149,154` - fake_data キャッシュメカニズム実装
- [ ] `nyx-stream/plugin_dispatch.rs:82` - IPC transport 配線

### Performance Analysis
- [ ] `nyx-cli/performance_analyzer.rs:439,886,911-912` - 実際のメトリクス計算実装

## 🟠 仕様書機能の実装確認要 (Medium Priority)

### Extended Error Codes (v0.1仕様)
- [ ] エラーコード 0x04 VERSION_MISMATCH 実装確認
- [ ] エラーコード 0x05 PATH_VALIDATION_FAILED 実装確認  
- [ ] エラーコード 0x06 INTERNAL_ERROR 実装確認

### Management Frames (v0.1仕様)
- [ ] Frame Type 0x30 SETTINGS 完全実装確認
- [ ] Frame Type 0x31 PING/0x32 PONG 実装確認
- [ ] Frame Type 0x33 PATH_CHALLENGE/0x34 PATH_RESPONSE 実装確認

### Congestion Control
- [ ] BBRv2 pacing_gain サイクル `[1.25, 0.75]` 実装確認
- [ ] ECN CE フラグ閾値 5% 実装確認
- [ ] CWND 最小 4 * 1280B 実装確認

### NAT Traversal
- [ ] ICE Lite 実装完成度確認
- [ ] UDP Hole Punching 詳細実装
- [ ] STUN サーバー統合確認

## 🔵 コード品質・安全性課題 (Low Priority)

### Error Handling
- [ ] `panic!` 使用箇所の適切なエラーハンドリングへの変更 (20+ locations found)
  - `nyx-transport/src/tcp_fallback.rs:142,151` - packet handling panics
  - `nyx-stream/tests/obfuscator.rs:18` - test panic on no packet
  - `nyx-daemon/src/path_builder.rs:3935` - path building validation panic
  - `nyx-crypto/src/noise.rs:739,748` - key combination panics
- [ ] `unreachable!` 使用箇所の検証 (5+ locations found)
  - `nyx-stream/src/scheduler.rs:118` - unreachable in state machine
  - `nyx-daemon/src/path_builder_broken.rs:3735` - unreachable in algorithm
  - `nyx-cli/tests/performance_tests.rs:211` - unreachable in benchmark
- [ ] Test専用 panic の分離

### Legacy/Deprecated Code
- [ ] Legacy implementation removal/modernization
  - `nyx-daemon/src/path_builder_broken.rs:3797,3828` - legacy cache fallback code
  - `nyx-daemon/src/metrics.rs:2897` - legacy Prometheus export method
  - `nyx-daemon/src/layer_manager.rs:886` - legacy layer coordination
  - `nyx-crypto/src/aead.rs:797` - legacy sync compatibility methods
  - `nyx-mix/src/vdf.rs:41` - classic repeated squaring implementation
- [ ] Deprecated Android API usage update
  - `nyx-mobile-ffi/src/android.rs:200` - PowerManager.isScreenOn() deprecation

### Incomplete/Partial Implementations
- [ ] Post-quantum cryptography completion (Kyber1024, BIKE)
- [ ] QUIC transport full implementation (currently partial)
- [ ] Mobile battery optimization algorithms
- [ ] Advanced BBR-like congestion control (`nyx-stream/src/congestion.rs`)
- [ ] Full APNS implementation (`nyx-control/src/push.rs:90` - minimal implementation)

### Test Infrastructure  
- [ ] プレースホルダーテスト実装
- [ ] WebAssembly 版テスト統合確認
- [ ] Miri 未定義動作検証の CI 統合
- [ ] Simulation-based tests vs real implementation gap resolution

### Documentation
- [ ] 各モジュールの仕様適合性ドキュメント
- [ ] API リファレンス完成
- [ ] 実装ガイドライン更新
- [ ] ROADMAP feature status alignment with actual implementation

## 📋 実装優先度

### Phase 1 (Immediate - Critical Path)
1. Multipath PathID ヘッダー実装
2. Plugin Framework 基本構造
3. Hybrid PQ Handshake (Kyber統合)
4. JSON/YAML serialization 完成

### Phase 2 (Short Term - Core Features)  
1. cMix Integration
2. RaptorQ FEC 実装
3. QUIC DATAGRAM サポート
4. OpenTelemetry 統合

### Phase 3 (Medium Term - Advanced Features)
1. Low Power Mode
2. TCP Fallback
3. Advanced routing algorithms
4. Performance optimization

### Phase 4 (Long Term - Polish)
1. コード品質向上
2. 包括的テスト
3. ドキュメント完成
4. 形式検証強化

## 🎯 成功基準

- [ ] v1.0 仕様書の全機能実装完了
- [ ] 互換テストスイート 100% 通過
- [ ] パフォーマンス目標達成 (90% UDP スループット維持)
- [ ] セキュリティ監査通過
- [ ] モバイル・デスクトップ両環境で動作確認

---
*最終更新: 2025年8月4日*