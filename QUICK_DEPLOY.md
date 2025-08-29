# 🚀 NYX NETWORK - QUICK DEPLOYMENT

## ⚡ Ubuntu/WSL (推奨)

```bash
# One-liner for complete setup
curl -fsSL https://raw.githubusercontent.com/your-repo/NyxNet/main/scripts/nyx-setup.sh | bash
```

## 🪟 Windows (管理者権限必要)

**PowerShellを管理者として実行してから：**

```powershell
# Step 1: Setup
cd "c:\Users\Aqua\Programming\SeleniaProject\NyxNet"
.\scripts\nyx-setup.bat

# Step 2: Deploy
.\scripts\nyx-deploy.bat

# Step 3: Check Results
kubectl get pods -o wide
kubectl logs -l app=nyx-bench
```

## 🔧 Manual Helm (Chocolatey)

```powershell
# Install Helm
choco install kubernetes-helm -y
refreshenv

# Install Docker Desktop + kind + kubectl manually
# Then run deployment
cd "c:\Users\Aqua\Programming\SeleniaProject\NyxNet"
helm upgrade --install nyx .\charts\nyx\ --values .\charts\nyx\values.yaml --set bench.enabled=true --set image.tag="latest" --set image.pullPolicy="Never"
```

## 📊 Benchmark Results

```powershell
# Check pod status
kubectl get pods -o wide

# View benchmark logs
kubectl logs -l app=nyx-bench

# Cleanup
.\scripts\nyx-cleanup.bat
```

## 🎯 U22コンテスト用設定

✅ Multi-node cluster (1 control-plane + 3 workers)  
✅ 6 daemon pods + 3 parallel benchmark jobs  
✅ Performance testing with connectivity matrix  
✅ Resource monitoring with Prometheus metrics  
✅ Cross-platform automation (Windows/Linux)  
✅ Production-ready Helm charts  

**管理者権限でPowerShellを起動してください！**
