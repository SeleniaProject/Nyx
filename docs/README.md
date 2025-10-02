# docs/README.md

> **品質ポリシー** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止（[詳細](./requirements.md#遵守事項)）

## 目次
- [ビジョンと位置づけ](#ビジョンと位置づけ)
- [エレベーターピッチ](#エレベーターピッチ)
- [解決する課題と価値提案](#解決する課題と価値提案)
- [主要ペルソナとシナリオ](#主要ペルソナとシナリオ)
- [成功指標 (KPI/OKR)](#成功指標-kpiokr)
- [全体アーキテクチャ俯瞰](#全体アーキテクチャ俯瞰)
- [文書ナビゲーション](#文書ナビゲーション)
- [貢献ガイドライン](#貢献ガイドライン)
- [トレーサビリティと準拠](#トレーサビリティと準拠)

## ビジョンと位置づけ
Nyxは、匿名通信の「プライバシ・性能・実用性」の三律背反を同時解決し、個人・組織・社会インフラが安心して採用できる匿名ネットワーク基盤を提供するプロジェクトである。ミッションは、**ポスト量子時代にも耐えうる匿名通信の世界標準**を確立し、セキュアインターネットのデファクトとして機能すること。

- **ビジョン**: あらゆる通信が暗黙的に匿名化され、誰もが安心して情報共有できる社会を実現する。
- **ミッション**: 次世代匿名通信プロトコルを通じ、モバイルからエンタープライズまで、応答性と信頼性を両立した体験を届ける。
- **プロダクト原則**: "Security by Design"、"Performance without compromise"、"Formalized trust"、"Inclusive UX"。

## エレベーターピッチ
> **Nyxは、ハイブリッドミックスネットと高性能ストリーム層を統合した匿名通信プラットフォームであり、組織と個人が高度なプライバシ保護とリアルタイム性を同時に享受できる世界を実現する。**

- 0-RTTハンドシェイクで高速接続を実現
- 動的経路選択とカバートラフィックでメタデータ漏洩を抑止
- モバイル端末を第一級利用者として最適化
- ポスト量子暗号とのハイブリッド運用で将来互換性を担保

## 解決する課題と価値提案
| 課題領域 | 現状の痛み | Nyxの解決策 | 価値指標 |
|-----------|-------------|--------------|-----------|
| 匿名性と応答性 | 高い遅延と切断率 | 自己最適化ルーティングとFEC統合 | P95遅延 \u2264 350ms / 接続維持率 \u2265 99.5% |
| スケーラビリティ | 大規模利用で性能劣化 | レイヤードアーキテクチャとキャパシティプラン | 10^6 同時セッションを線形拡張 |
| モバイル適合 | バッテリーとネットワーク制限 | アダプティブカバー交通とモバイル優先経路 | バッテリ消費比従来比 -25% |
| メタデータ漏洩 | 交通解析への脆弱性 | 可変カバートラフィックとタイミング撹乱 | 交通分析抵抗指標 \u2265 0.9 |
| ポスト量子耐性 | 将来の暗号脅威 | ハイブリッド鍵交換とアルゴリズムアジリティ | PQ移行手順の無停止実行 |

## 主要ペルソナとシナリオ
- **政府調達担当者 (RegSec Officer)**: 規制遵守と監査可能性を重視し、[セキュリティ仕様](./security/auth.md)および[脆弱性対策](./security/vulnerability.md)を参照して導入判定。
- **モバイルアプリ開発者 (Mobile Integrator)**: SDKを利用し、遅延と省電力の要件を[性能/スケーラビリティ](./performance/scalability.md)と[UIガイドライン](./ui/overview.md)から確認。
- **プライバシ研究者 (Academic Partner)**: プロトコルアーキテクチャと形式検証方針を[アーキテクチャ概説](./architecture/overview.md)と[テスト計画](./testing/metrics.md)から理解。
- **サイト信頼性エンジニア (SRE)**: 運用SOPやロールバック手順を[インフラ抽象設計](./deployment/infrastructure.md)と[ロールバック戦略](./deployment/rollback.md)から参照。

### 代表的UXシナリオ
1. モバイルユーザがNyx対応メッセージングを利用し、既存アプリと同等のレスポンスで匿名チャットを継続。
2. 報道機関がNyx経由で情報提供を受け、送信者匿名性と完全性監査を両立。
3. 企業内マイクロサービスがNyxネットワークを用いたゼロトラスト通信を実現し、内部脅威を低減。

## 成功指標 (KPI/OKR)
- **OKR-O1**: 主要ユーザ体験の匿名性と性能の両立
  - KR1: P95遅延 350ms以下を維持 (検証: [performance/benchmark](./performance/benchmark.md))
  - KR2: セッション持続率99.5%以上 (監視: [metrics](./testing/metrics.md))
  - KR3: カバー交通適応率90%以上 (監視: [dataflow](./architecture/dataflow.md))
- **OKR-O2**: セキュリティとポスト量子耐性の保証
  - KR1: 主要暗号スイートをPQハイブリッドへ完全移行 ([encryption](./security/encryption.md))
  - KR2: STRIDEカテゴリ別残留リスクを"Low"以下 ([vulnerability](./security/vulnerability.md))
- **OKR-O3**: エコシステムと開発効率
  - KR1: SDK統合に要するセットアップ時間1日以内 ([templates/module-template](./templates/module-template.md))
  - KR2: 新規機能リリースサイクル2週間以内 ([roadmap](./roadmap.md))

## 全体アーキテクチャ俯瞰
```
+-------------------+      +------------------+      +-------------------+
|  クライアント群   | ---> | 安全ストリーム層 | ---> |   ミックス経路層   |
| (モバイル/デスク) |      | (0-RTT,多重化)   |      | (適応ルーティング) |
+-------------------+      +------------------+      +-------------------+
        |                          |                          |
        v                          v                          v
+---------------------+    +------------------+        +------------------+
| オブフスケーション層 | -> | FEC & カバー交通 |  ->    | 監視/テレメトリ層 |
+---------------------+    +------------------+        +------------------+
```
※詳細は[architecture/overview.md](./architecture/overview.md)と[architecture/dataflow.md](./architecture/dataflow.md)を参照。

## 文書ナビゲーション
| カテゴリ | ドキュメント | 概要 |
|----------|--------------|------|
| 要件 | [requirements](./requirements.md) | 機能・非機能・用語集とトレーサビリティ規約 |
| アーキテクチャ | [overview](./architecture/overview.md), [dataflow](./architecture/dataflow.md), [tech-stack](./architecture/tech-stack.md) | システム設計、データ流、技術選定 |
| UI/UX | [ui/overview](./ui/overview.md) 他 | 情報設計、視覚・モーション規範、アクセシビリティ |
| セキュリティ | [security/auth](./security/auth.md), [security/encryption](./security/encryption.md), [security/vulnerability](./security/vulnerability.md) | 認証、暗号、脅威対策 |
| 性能/テスト | [performance/scalability](./performance/scalability.md), [testing/unit-tests](./testing/unit-tests.md) 他 | 性能策、テスト戦略、品質指標 |
| デプロイ | [deployment/infrastructure](./deployment/infrastructure.md) 他 | 抽象インフラ、CI/CD、ロールバック |
| 参照・テンプレ | [notes](./notes/meeting-notes.md), [templates](./templates/module-template.md) | 議事録・ADR・テンプレート |

## 貢献ガイドライン
- **コミット規約**: Conventional Commits (`feat:`, `fix:`, `docs:`など)を必須とし、変更概要と影響範囲を明示。
- **変更フロー**: 要件→設計→テスト→レビューの順で、各ステップの成果物を対応ドキュメントへ反映。
- **レビュー基準**: セキュリティ、性能、アクセシビリティ、国際化、運用性の観点でChecklistsを適用。
- **禁止事項**: 実装コード断片、C/C++依存ライブラリの提案、検証されない性能主張の記載。

## トレーサビリティと準拠
- すべての要件は[requirements.md](./requirements.md)でID管理し、設計・テストドキュメントにマッピング。
- SLO/SLIは[testing/metrics.md](./testing/metrics.md)で集中管理し、CI/CDゲートと連動。
- 法規制・コンプライアンス要件は[security/vulnerability.md](./security/vulnerability.md#監査と法的遵守)で定義。

> **遵守宣言**: 本プロジェクトの文書群は、実装コードを出力せず、C/C++および同系統依存ライブラリへの依存を一切許容しない。代替手段は各ドキュメント内で明示する。