#!/bin/bash
set -euo pipefail

echo "======================================================"
echo "NYX NETWORK - UBUNTU KUBERNETES SETUP"
echo "======================================================"
echo "Setting up Docker + kubectl + Helm + kind"
echo "Then deploying Nyx with multi-node performance testing"
echo "======================================================"

# Install Docker if not present
if ! command -v docker >/dev/null 2>&1; then
    echo "Installing Docker..."
    curl -fsSL https://get.docker.com | sh
    sudo usermod -aG docker "$USER"
    echo "[!] Docker installed. Please logout and login again, then run this script again."
    exit 0
fi

# Check if Docker daemon is running
if ! docker info >/dev/null 2>&1; then
    echo "[!] Docker daemon is not running. Starting Docker..."
    sudo systemctl enable --now docker
    sleep 3
    if ! docker info >/dev/null 2>&1; then
        echo "[!] Failed to start Docker. Please start manually and try again."
        exit 1
    fi
fi

# Install kubectl if not present
if ! command -v kubectl >/dev/null 2>&1; then
    echo "Installing kubectl..."
    sudo apt-get update -y
    sudo apt-get install -y ca-certificates curl gnupg
    sudo install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://pkgs.k8s.io/core:/stable:/v1.30/deb/Release.key | sudo gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg
    echo "deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] https://pkgs.k8s.io/core:/stable:/v1.30/deb/ /" | sudo tee /etc/apt/sources.list.d/kubernetes.list >/dev/null
    sudo apt-get update -y
    sudo apt-get install -y kubectl
fi

# Install Helm if not present
if ! command -v helm >/dev/null 2>&1; then
    echo "Installing Helm..."
    curl -fsSL https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash
fi

# Install kind if not present
if ! command -v kind >/dev/null 2>&1; then
    echo "Installing kind..."
    curl -Lo kind "https://kind.sigs.k8s.io/dl/v0.23.0/kind-linux-amd64"
    chmod +x kind
    sudo mv kind /usr/local/bin/
fi

echo "======================================================"
echo "SETUP COMPLETE!"
echo "Now run: ./nyx-deploy.sh"
echo "======================================================"
