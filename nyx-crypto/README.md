# nyx-crypto

Nyx の暗号基盤。Noise/HPKE/AEAD/KDF を収容する純Rust実装（unsafe禁止）。

- AEAD: ChaCha20-Poly1305 ラッパー（鍵ゼロ化）。
- KDF: HKDF-SHA256 薄ラッパー、RFC8439スタイル nonce 合成。
- HPKE (feature=hpke): X25519-HKDF-SHA256 + AES-GCM-128。
- Session: 単方向 AEAD セッション（seq/nonce上限・枯渇検出・Dropゼロ化）。
- Noise guard: メッセージ長に防御的上限チェック。
- Keystore: PBKDF2(HMAC-SHA256)+AES-GCM-256 による小規模シークレットの封緘（純Rust、ゼロ化）。
- PCR: Post-Compromise Recovery の鍵導出ヘルパ（HKDF/BLAKE3）。

## 開発
- clippy 厳格: `cargo clippy -p nyx-crypto -- -D warnings`
- テスト:
  - 通常: `cargo test -p nyx-crypto`
  - HPKE有効: `cargo test -p nyx-crypto --features hpke`
  - ベンチ: `cargo bench -p nyx-crypto --features bench`

## Feature
- `classic`: X25519 (デフォルト)
- `kyber`: PQ（将来移行）。
- `hpke`: RFC9180 HPKE を有効化。
- `runtime`: tokio依存の補助を有効化。
  - keystoreのasync保存/読込（オプション）
