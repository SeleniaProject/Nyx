# 🚀 NYX NETWORK - UBUNTU SERVER DEPLOYMENT

## ⚡ One-Liner Setup & Deploy

```bash
# Complete setup and deployment in one command
curl -fsSL https://raw.githubusercontent.com/SeleniaProject/Nyx/main/scripts/nyx-setup.sh | bash
```

## 🔧 Manual Step-by-Step

```bash
# 1. Clone repository
git clone https://github.com/SeleniaProject/Nyx.git
cd Nyx

# 2. Make scripts executable
chmod +x scripts/*.sh

# 3. Run setup (installs Docker, kubectl, Helm, kind)
./scripts/nyx-setup.sh

# 4. Deploy with benchmarks
./scripts/nyx-deploy.sh

# 5. Check results
kubectl get pods -o wide
kubectl logs -l app=nyx-bench
```

## 📊 Quick Status Check

```bash
# Check cluster status
kind get clusters
kubectl cluster-info

# Check all pods
kubectl get pods -A

# View benchmark results
kubectl logs -l app=nyx-bench --tail=50

# Monitor in real-time
kubectl logs -f -l app=nyx-daemon
```

## 🎯 U22 Contest Features

✅ **Multi-node cluster** (1 control-plane + 3 workers)  
✅ **6 daemon pods** + **3 benchmark jobs**  
✅ **Performance testing** with connectivity matrix  
✅ **Resource monitoring** with Prometheus metrics  
✅ **Load balancing** validation  
✅ **Production-ready** Helm charts  

## 🧹 Cleanup

```bash
# Remove everything
./scripts/nyx-cleanup.sh

# Or manual cleanup
helm uninstall nyx || true
kind delete cluster --name nyx-cluster
```

**Just run the one-liner on your Ubuntu server!** 🏆
