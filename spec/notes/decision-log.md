# docs/notes/decision-log.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止

## 目次
- [目的](#目的)
- [ADRテンプレート](#adrテンプレート)
- [管理ルール](#管理ルール)
- [サンプルエントリ](#サンプルエントリ)
- [関連ドキュメント](#関連ドキュメント)

## 目的
Nyxプロジェクトの意思決定履歴を透明化し、背景・根拠・影響を追跡可能にする。

## ADRテンプレート
```
# ADR-XXXX タイトル
- ステータス: Proposed / Accepted / Superseded / Deprecated
- 日付: YYYY-MM-DD
- 背景:
- 問題:
- 選択肢:
  1. 
  2. 
- 決定:
- 根拠:
- 影響:
- フォローアップ:
- 撤退条件:
- 関連資料: [requirements.md#...](../requirements.md#...), [architecture/overview.md](../architecture/overview.md), ...
```

## 管理ルール
- 番号は連番。ステータス変更時はエントリ更新。
- 大規模変更や依存更新は必ずADR作成。
- 撤退条件を必須記載。

## サンプルエントリ
```
# ADR-0001 ハイブリッド鍵交換方式の採用
- ステータス: Accepted
- 日付: 2025-04-01
- 背景: PQ移行準備
- 問題: 従来方式では量子耐性不足
- 選択肢:
  1. X25519のみ
  2. X25519 + Kyber1024 (採用)
- 決定: ハイブリッド方式
- 根拠: 将来互換性、性能影響軽微
- 影響: ハンドシェイク複雑化
- フォローアップ: 形式検証
- 撤退条件: PQ脆弱性が発覚
- 関連資料: [security/encryption.md](../security/encryption.md)
```

## 関連ドキュメント
- [notes/meeting-notes.md](./meeting-notes.md)
- [roadmap.md](../roadmap.md)
- [templates/module-template.md](../templates/module-template.md)

> **宣言**: 実装コード無し、C/C++依存無し。