# クイックスタート（Ubuntu/Kubernetes）

この手順はローカル/検証用途です。本番では組織のセキュリティ基準に従ってください。

## 前提

- Ubuntu 20.04+ / 22.04+
- kubectl / Helm 3.x / kind もしくは既存の k8s クラスタ

## 1. リポジトリ取得とビルド

```bash
git clone <this repo>
cd NyxNet
cargo build --release
```

## 2. kind で検証用クラスタ（任意）

```bash
kind create cluster --config kind-config.yaml
```

## 3. Helm デプロイ

```bash
helm upgrade --install nyx ./charts/nyx \
  --values ./charts/nyx/values.yaml
```

メトリクスを有効化する場合は ServiceMonitor 等を values 側で有効化してください。

## 4. 状態確認

```bash
kubectl get pods -l app.kubernetes.io/name=nyx -A
```

## 5. 片付け（kind のみ）

```bash
kind delete cluster
```