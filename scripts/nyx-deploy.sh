#!/bin/bash
set -euo pipefail

echo "======================================================"
echo "NYX NETWORK - KUBERNETES DEPLOYMENT WITH BENCHMARKS"
echo "======================================================"
echo "Creating kind cluster and deploying Nyx with performance testing"
echo "======================================================"

# Check if Docker is running
if ! docker info >/dev/null 2>&1; then
    echo "[!] Docker is not running. Please start Docker first."
    exit 1
fi

# Create kind cluster configuration
echo "Creating multi-node kind cluster configuration..."
cat > /tmp/kind-nyx.yaml <<'EOF'
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
  - role: control-plane
  - role: worker
  - role: worker
  - role: worker
EOF

# Create or use existing kind cluster
echo "Creating kind cluster 'nyx'..."
if ! kind get clusters | grep -q '^nyx$'; then
    kind create cluster --name nyx --config /tmp/kind-nyx.yaml
else
    echo "Using existing 'nyx' cluster"
fi

# Build and load local image
echo "Building Nyx daemon local image..."
if [ ! -f "Dockerfile.legacy" ]; then
    echo "[!] Dockerfile.legacy not found. Please run from repository root."
    exit 1
fi

docker build -f Dockerfile.legacy -t nyx-daemon:local .
echo "Loading image into kind cluster..."
kind load docker-image nyx-daemon:local --name nyx

# Create namespace
echo "Creating Kubernetes namespace..."
kubectl create namespace nyx --dry-run=client -o yaml | kubectl apply -f -

# Add Prometheus Operator for ServiceMonitor support
echo "Installing Prometheus Operator..."
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update

# Check if already installed and upgrade if needed
if helm list -n monitoring | grep -q prometheus-operator; then
    echo "Prometheus Operator already installed, upgrading..."
    helm upgrade prometheus-operator prometheus-community/kube-prometheus-stack \
      --namespace monitoring \
      --set grafana.enabled=false --set alertmanager.enabled=false \
      --set prometheus.enabled=false --set kubeStateMetrics.enabled=false \
      --set nodeExporter.enabled=false --set prometheusOperator.enabled=true
else
    echo "Installing new Prometheus Operator..."
    helm install prometheus-operator prometheus-community/kube-prometheus-stack \
      --namespace monitoring --create-namespace \
      --set grafana.enabled=false --set alertmanager.enabled=false \
      --set prometheus.enabled=false --set kubeStateMetrics.enabled=false \
      --set nodeExporter.enabled=false --set prometheusOperator.enabled=true
fi

# Deploy Nyx with multi-node configuration
echo "Deploying Nyx with multi-node performance testing..."

# Clean up any existing deployment first
if kubectl get deployment nyx -n nyx >/dev/null 2>&1; then
    echo "Cleaning up existing Nyx deployment..."
    kubectl delete job nyx-bench -n nyx --ignore-not-found=true
    helm uninstall nyx -n nyx || true
    sleep 5
fi

helm upgrade --install nyx ./charts/nyx -n nyx \
  --set image.repository=nyx-daemon --set image.tag=local --set image.pullPolicy=IfNotPresent \
  --set replicaCount=6 --set bench.enabled=true --set bench.replicas=3 \
  --set bench.testDurationSeconds=30 --set bench.concurrentConnections=5 \
  --set pdb.enabled=true --set pdb.minAvailable=3 --set serviceMonitor.enabled=true \
  --set probes.startup.enabled=false --set probes.liveness.enabled=false --set probes.readiness.enabled=false

# Wait for deployment
echo "Waiting for Nyx deployment to complete..."
kubectl rollout status -n nyx deploy/nyx --timeout=300s

# Wait for all daemon pods to be ready
echo "Waiting for all daemon pods to be ready..."
kubectl wait -n nyx --for=condition=ready pod -l app.kubernetes.io/name=nyx --timeout=300s

# Check pod status before benchmark
echo "Checking pod status before benchmark..."
kubectl get pods -n nyx -o wide

# Wait for benchmark job completion with more time and better error handling
echo "Waiting for benchmark job to complete..."
if ! kubectl wait -n nyx --for=condition=complete job/nyx-bench --timeout=300s; then
    echo "Benchmark job did not complete within 5 minutes. Checking status..."
    kubectl describe job nyx-bench -n nyx
    kubectl get pods -n nyx | grep bench
    echo "Showing benchmark pod logs..."
    kubectl logs -n nyx -l job-name=nyx-bench --tail=50
    echo "Continuing with partial results..."
fi

# Show results
echo "======================================================"
echo "MULTI-NODE PERFORMANCE BENCHMARK RESULTS"
echo "======================================================"

# Try to get benchmark results
if kubectl get job nyx-bench -n nyx >/dev/null 2>&1; then
    echo "Benchmark job found. Getting results..."
    kubectl logs -n nyx job/nyx-bench --tail=100
else
    echo "Benchmark job not found. Showing current pod status..."
    kubectl get pods -n nyx -o wide
fi

echo "======================================================"
echo "CLUSTER STATUS"
echo "======================================================"
kubectl get pods,svc,pdb -n nyx -o wide

echo "======================================================"
echo "NODE DISTRIBUTION"
echo "======================================================"
kubectl get pods -n nyx -o wide | awk 'NR>1{print $7}' | sort | uniq -c

echo "======================================================"
echo "DEPLOYMENT COMPLETE!"
echo "Check the benchmark results above for performance rating"
echo "===================================================="
