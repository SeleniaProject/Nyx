# Capability 交渉ポリシー（Nyx Protocol v1.0）

本書は Nyx Protocol における Capability 交渉（機能ネゴシエーション）の規約・拡張ポリシーを定義します。実装は `nyx-stream` クレートで提供され、リファレンス実装とテストは本リポジトリに含まれます。

## 目的
- エンドポイント間で利用可能な機能集合（Capability）を合意する
- 必須機能が未対応の場合は確実かつ相互運用可能な方法でセッションを終了する
- 将来の拡張（後方互換／前方互換）のための明確なルールを提供する

## 用語
- 必須（Required）: 相手が未対応なら即時終了すべき機能
- 任意（Optional）: 相手が未対応でも接続継続可能な機能

## ワイヤ形式（CBOR）
最初の CRYPTO 相当フレームで、Capability の CBOR 配列を交換します。各要素は下記のマップです。

```
{ id: u32, flags: u8, data: bytes }
```

- `id`: Capability ID（32-bit）
- `flags`: 下位ビット 0 が Required を表す（1=Required, 0=Optional）
- `data`: 任意の付加データ（バージョン/サブ機能/パラメータ等）。バイト列（CBOR bytes）。

実装箇所: `nyx-stream/src/capability.rs`（`Capability`/`encode_caps`/`decode_caps`）

## 既定の Capability ID
- `0x0001` = `core`（必須）
- `0x0002` = `plugin_framework`（任意）

コード定義: `nyx-stream/src/capability.rs` の `LOCAL_CAP_IDS`。

## 交渉アルゴリズム
相手から受信した配列に含まれる「必須（Required）」フラグ付きの Capability について、ローカル実装が対応しているか判定します。最初の未対応 ID を見つけた時点で交渉失敗とします。

擬似コード:
```
fn negotiate(local_supported: &[u32], peer_caps: &[Capability]) -> Result<(), Unsupported(id)> {
  for cap in peer_caps {
    if cap.is_required() && !local_supported.contains(&cap.id) {
      return Err(Unsupported(cap.id))
    }
  }
  Ok(())
}
```

実装箇所: `nyx-stream/src/capability.rs::negotiate`

## 未対応必須 Capability のエラー終了（CLOSE 0x07）
必須 Capability に未対応の場合、接続は `ERR_UNSUPPORTED_CAP = 0x07` を用いて CLOSE します。CLOSE の `reason` には未対応 ID の 4 バイト BE を含めます。

- 定数/ビルダ: `nyx-stream/src/management.rs`
  - `ERR_UNSUPPORTED_CAP: u16 = 0x07`
  - `build_close_unsupported_cap(id: u32) -> Vec<u8>`

## 拡張ポリシー
1. 新 Capability 追加
   - 新しい ID を割り当て、既存実装は未対応として扱う。
   - 互換性を壊さないため、既定は Optional を推奨。Required 化は十分なデプロイ後に行う。
2. バージョニング
   - `data` に `{version:u16, params:...}` のような自前スキーマを CBOR で格納してよい。
   - 不明な `data` は仕様に従い無視可能であること。
3. 互換性
   - Optional は常に接続継続可能であること（影響は機能限定のみ）。
   - Required はハードフェイル（CLOSE 0x07）。

## セキュリティ配慮
- 受信 CBOR のサイズ・フィールド境界を厳格に検証（過大入力・オーバーフロー防止）。
- 未知の Capability は Optional であれば無視、Required であれば 0x07 で終了。
- CLOSE の `reason` は 4 バイトのみ（DoS 回避のため簡潔に）。

## 実装/テスト参照
- 実装
  - `nyx-stream/src/capability.rs`
  - `nyx-stream/src/management.rs`
- テスト
  - `nyx-conformance/tests/capability_negotiation_properties.rs`
  - `nyx-stream/tests/plugin_framework_tests.rs`（関連 ID/範囲の検証）

## 変更履歴
- 初版: v1.0（本ドキュメント）


