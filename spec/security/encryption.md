# docs/security/encryption.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止

## 目次
- [目的](#目的)
- [暗号スイート構成](#暗号スイート構成)
- [鍵管理](#鍵管理)
- [転送時暗号](#転送時暗号)
- [保管時暗号](#保管時暗号)
- [鍵ローテーション](#鍵ローテーション)
- [SBOMと秘密管理](#sbomと秘密管理)
- [検証と監査](#検証と監査)
- [関連ドキュメント](#関連ドキュメント)

## 目的
Nyxにおける暗号設計、鍵管理、転送/保管時の保護、ポスト量子対応を定義する。

## 暗号スイート構成
| コンポーネント | アルゴリズム (現行) | PQ代替 | 選択理由 |
|----------------|--------------------|--------|----------|
| 鍵交換 | X25519 | Kyber1024 | 高速/安定、PQ移行可能 |
| 対称暗号 | ChaCha20-Poly1305 | Ascon128a | CPU効率、低消費電力 |
| ハッシュ | SHA-256 | BLAKE3 | 標準化/高性能 |
| 署名 | Ed25519 | Dilithium3 | 小さい鍵サイズ、成熟度 |
- すべてRust/Go純実装を利用し、C/C++依存を排除。

## 鍵管理
- **KMS**: HashiCorp Vault互換APIまたはクラウドKMS。アクセスはmTLS+OIDC。
- **キー階層**: ルート、マスター、セッション鍵の3層。セッション鍵はメモリ内のみ。
- **秘密分散**: マスター鍵バックアップはShamir Secret Sharingで複数保管。

## 転送時暗号
- レイヤ: Secure Stream (エンドツーエンド)、Transport (mTLS)。
- ハンドシェイク: Noise_Nyxパターン。0-RTTは事前共有鍵＋追加検証。
- ポリシー: 必須暗号スイートは最小暗号強度 (128-bit) を下回らない。

## 保管時暗号
- データ分類に応じAES-GCM/ChaCha20を利用（マネージドKMS）。
- 監査ログはWrite-Onceストレージに暗号化保存。鍵は期間限定アクセス。
- メタデータは匿名化/トークナイズ。

## 鍵ローテーション
| 鍵種 | 周期 | トリガ | 対応 |
|------|------|--------|------|
| セッション鍵 | 10分 or 1GB | 自動 | ハンドシェイク更新 |
| マスター鍵 | 90日 | スケジュール | 新旧併存期間7日 |
| ルート鍵 | 1年 | ガバナンス承認 | セキュアセレモニー |
- ローテーション結果は[notes/decision-log.md](../notes/decision-log.md)に記録。

## SBOMと秘密管理
- 暗号関連ライブラリはSBOMで追跡 (SPDX形式)。
- シークレットは`secret://`スキームで参照し、ファイルシステム保存禁止。
- CI/CDではSecrets Managerを利用し、[deployment/ci-cd.md](../deployment/ci-cd.md)参照。

## 検証と監査
- 暗号実装は形式検証とプロパティテストを実施 ([testing/unit-tests.md](../testing/unit-tests.md))。
- 外部監査は年次実施。レポートはSecurity Councilでレビュー。
- PQアルゴリズムはNIST進捗を追跡し、互換性テストを半期ごとに実施。

## 関連ドキュメント
- [security/auth.md](./auth.md)
- [security/vulnerability.md](./vulnerability.md)
- [architecture/dataflow.md](../architecture/dataflow.md)
- [performance/benchmark.md](../performance/benchmark.md)

> **宣言**: 実装コードを含まず、C/C++依存を排除する。