# 🚀 NYX NETWORK - ONE-LINER SETUP FOR U22 CONTEST

## ⚡ QUICK START (管理者PowerShell必要)

```powershell
# Step 1: Install tools (管理者として実行)
cd "c:\Users\Aqua\Programming\SeleniaProject\NyxNet"
.\install-tools.bat

# Step 2: Restart PowerShell, then create cluster
kind create cluster --config kind-config.yaml

# Step 3: Deploy Nyx with benchmarks
.\helm.exe upgrade --install nyx .\charts\nyx --values .\charts\nyx\values.yaml --set bench.enabled=true --set image.tag="latest" --set image.pullPolicy="Never" --timeout=10m

# Step 4: Check results
kubectl get pods -n nyx -o wide
kubectl logs -l app=nyx-bench -n nyx
```

## 🐳 Docker Desktop Alternative

Docker Desktopがインストール済みの場合：

```powershell
# Enable Kubernetes in Docker Desktop settings
# Then create namespace and deploy
kubectl create namespace nyx
.\helm.exe upgrade --install nyx .\charts\nyx --values .\charts\nyx\values.yaml --set bench.enabled=true --namespace nyx
```

## 📊 Performance Results

```powershell
# Monitor deployment
kubectl get pods -n nyx --watch

# View benchmark results
kubectl logs -l app=nyx-bench -n nyx --tail=50

# Check service endpoints
kubectl get svc -n nyx
```

## 🔧 Troubleshooting

```powershell
# If pods are pending
kubectl describe pods -n nyx

# If image pull fails
docker images | grep nyx

# Restart deployment
.\helm.exe delete nyx -n nyx
.\helm.exe upgrade --install nyx .\charts\nyx --values .\charts\nyx\values.yaml --set bench.enabled=true --namespace nyx
```

**🏆 U22コンテスト用マルチノードテスト環境完成！**
