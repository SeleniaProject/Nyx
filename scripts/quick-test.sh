#!/bin/bash
set -e

echo "🚀 NYX QUICK TEST - UBUNTU DEPLOYMENT"
echo "======================================"

# Check prerequisites
if ! command -v docker >/dev/null 2>&1; then
    echo "❌ Docker not found. Installing..."
    curl -fsSL https://get.docker.com | sh
    sudo usermod -aG docker "$USER"
    echo "📋 Please logout and login again, then re-run this script"
    exit 1
fi

if ! docker info >/dev/null 2>&1; then
    echo "🔄 Starting Docker..."
    sudo systemctl start docker
    sleep 3
fi

if ! command -v kubectl >/dev/null 2>&1; then
    echo "📦 Installing kubectl..."
    curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
    chmod +x kubectl
    sudo mv kubectl /usr/local/bin/
fi

if ! command -v helm >/dev/null 2>&1; then
    echo "⚒️ Installing Helm..."
    curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
fi

if ! command -v kind >/dev/null 2>&1; then
    echo "🎯 Installing kind..."
    curl -Lo kind "https://kind.sigs.k8s.io/dl/v0.23.0/kind-linux-amd64"
    chmod +x kind
    sudo mv kind /usr/local/bin/
fi

# Create simple kind cluster
echo "🏗️ Creating kind cluster..."
if ! kind get clusters 2>/dev/null | grep -q "^nyx$"; then
    cat > /tmp/kind-simple.yaml <<EOF
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
  - role: worker
EOF
    kind create cluster --name nyx --config /tmp/kind-simple.yaml
else
    echo "✅ Using existing cluster"
fi

# Set kubectl context
kubectl config use-context kind-nyx

echo "⚡ Quick deployment with Alpine..."
helm upgrade --install nyx ./charts/nyx \
    --values ./charts/nyx/values-quick.yaml \
    --wait --timeout=2m

echo ""
echo "🎉 DEPLOYMENT COMPLETE!"
echo "======================================"
echo "📊 Check status:"
echo "kubectl get pods -o wide"
echo ""
echo "📋 View logs:"
echo "kubectl logs -l app=nyx-daemon"
echo "kubectl logs -l app=nyx-bench"
echo ""
echo "🧹 Cleanup:"
echo "helm uninstall nyx && kind delete cluster --name nyx"
