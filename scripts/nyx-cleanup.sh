#!/bin/bash
set -euo pipefail

echo "======================================================"
echo "NYX NETWORK - CLEANUP SCRIPT"
echo "======================================================"
echo "Cleaning up Nyx deployment and kind cluster"
echo "======================================================"

# Clean up Nyx resources
echo "Cleaning up Nyx resources..."
kubectl delete job nyx-bench -n nyx --ignore-not-found=true
helm uninstall nyx -n nyx --ignore-not-found || true
kubectl delete namespace nyx --ignore-not-found=true

# Clean up Prometheus Operator
echo "Cleaning up Prometheus Operator..."
helm uninstall prometheus-operator -n monitoring --ignore-not-found || true
kubectl delete namespace monitoring --ignore-not-found=true

# Delete kind cluster
echo "Deleting kind cluster 'nyx'..."
if kind get clusters | grep -q '^nyx$'; then
    kind delete cluster --name nyx
    echo "Kind cluster 'nyx' deleted successfully"
else
    echo "Kind cluster 'nyx' not found"
fi

echo "======================================================"
echo "CLEANUP COMPLETE!"
echo "Run ./nyx-deploy.sh to redeploy fresh"
echo "======================================================"
