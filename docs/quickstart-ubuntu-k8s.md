# Ubuntu 向けクイックスタート: Kubernetes で Nyx を試す（ワンライナー付き）

この手順は Ubuntu 22.04+ で、Kubernetes クラスタの用意から Nyx のデプロイ、簡易ベンチ Job の実行・ログ確認までを一気通貫で行います。

- 推奨パス: Docker + kind（最小で手早い）
- 代替パス: microk8s（Docker 不要のオールインワン）

> 注意: 初回インストールではグループ変更の反映のため、シェル再起動が必要な場合があります（特に docker グループ）。

---

## 1) ワンライナー（Docker + kind 版）

Ubuntu 上で「ツール導入 → kind クラスタ作成 → Nyx を Helm で展開 → ベンチ Job 完了まで待機 → ログ表示」まで実行します。

```bash
set -euo pipefail; \
if ! command -v docker >/dev/null 2>&1; then curl -fsSL https://get.docker.com | sh; sudo usermod -aG docker "$USER"; fi; \
if ! command -v kubectl >/dev/null 2>&1; then \
  sudo apt-get update -y; \
  sudo apt-get install -y ca-certificates curl gnupg; \
  sudo install -m 0755 -d /etc/apt/keyrings; \
  curl -fsSL https://pkgs.k8s.io/core:/stable:/v1.30/deb/Release.key | sudo gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg; \
  echo "deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] https://pkgs.k8s.io/core:/stable:/v1.30/deb/ /" | sudo tee /etc/apt/sources.list.d/kubernetes.list >/dev/null; \
  sudo apt-get update -y; \
  sudo apt-get install -y kubectl; \
fi; \
if ! command -v helm >/dev/null 2>&1; then curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash; fi; \
if ! command -v kind >/dev/null 2>&1; then \
  curl -Lo kind https://kind.sigs.k8s.io/dl/v0.23.0/kind-linux-amd64; chmod +x kind; sudo mv kind /usr/local/bin/; \
fi; \
if ! docker info >/dev/null 2>&1; then echo "[!] Docker daemon が起動していません。'sudo systemctl start docker' などで起動してください。"; exit 1; fi; \
(kind get clusters | grep -q '^nyx$' || kind create cluster --name nyx); \
kubectl create namespace nyx --dry-run=client -o yaml | kubectl apply -f -; \
helm upgrade --install nyx ./charts/nyx -n nyx --set replicaCount=3 --set bench.enabled=true; \
kubectl rollout status -n nyx deploy/nyx --timeout=300s; \
kubectl wait -n nyx --for=condition=complete job/nyx-bench --timeout=600s; \
kubectl logs -n nyx job/nyx-bench
```

メモ:
- 初回に `usermod -aG docker $USER` を実行した場合、`newgrp docker` するか一度ログインし直してください。
- 既存のクラスタ名 `nyx` がある場合は再利用されます。
- 画像リポジトリを変えたい場合は `--set image.repository=... --set image.tag=...` を追加してください。

---

## 2) 代替: microk8s ワンライナー（Docker 不要）

Docker を入れたくない場合は microk8s を使うと簡単です（Helm も組み込み）。

```bash
set -euo pipefail; \
sudo snap install microk8s --classic; \
sudo usermod -aG microk8s "$USER"; \
newgrp microk8s <<'EOF'
set -euo pipefail
microk8s status --wait-ready
microk8s enable dns storage helm3
microk8s kubectl create namespace nyx --dry-run=client -o yaml | microk8s kubectl apply -f -
microk8s helm3 upgrade --install nyx ./charts/nyx -n nyx --set replicaCount=3 --set bench.enabled=true
microk8s kubectl rollout status -n nyx deploy/nyx --timeout=300s
microk8s kubectl wait -n nyx --for=condition=complete job/nyx-bench --timeout=600s
microk8s kubectl logs -n nyx job/nyx-bench
EOF
```

メモ:
- 初回は `newgrp microk8s` でグループ反映しています。別ターミナルでも可。
- `microk8s helm3` を使う点に注意（通常の `helm` とはコマンド名が異なります）。

---

## 3) よくあるトラブル

- Docker デーモンが起動していない
  - `sudo systemctl enable --now docker` で起動します。
- 権限エラー（permission denied / dial unix / connect permission）
  - `sudo usermod -aG docker $USER` 後に再ログイン（または `newgrp docker`）。
- イメージ取得に時間がかかる
  - ネットワーク制限がある環境ではプロキシ設定やローカルレジストリの利用を検討してください。

---

## 4) 参考: Helm Chart 設定の主なキー

`charts/nyx/values.yaml` の例:
- `replicaCount`: レプリカ数（例: 3）
- `stateful.enabled`: StatefulSet を有効化（安定した Pod ID が欲しい場合）
- `topologySpreadConstraints`: ノード/ゾーンに均等分散したい場合に設定
- `bench.enabled`: ベンチ Job を有効化
- `bench.image`, `bench.command`, `bench.args`: ベンチ内容をカスタマイズ

---

## 5) 片付け

- kind クラスタを削除:

```bash
kind delete cluster --name nyx
```

- microk8s を無効化/削除:

```bash
microk8s stop
sudo snap remove microk8s
```

---

以上で Ubuntu 上の Kubernetes 環境に Nyx を一括デプロイして、簡単な通信/ベンチ確認まで行えます。
