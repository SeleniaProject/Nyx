@echo off
setlocal enabledelayedexpansion

echo ======================================================
echo NYX NETWORK - KUBERNETES DEPLOYMENT WITH BENCHMARKS
echo ======================================================
echo Creating kind cluster and deploying Nyx with performance testing
echo ======================================================

REM Check if Docker is running
docker info >nul 2>&1
if %errorLevel% neq 0 (
    echo [!] Docker is not running. Please start Docker Desktop first.
    pause
    exit /b 1
)

REM Create kind cluster configuration
echo Creating multi-node kind cluster configuration...
(
echo kind: Cluster
echo apiVersion: kind.x-k8s.io/v1alpha4
echo nodes:
echo   - role: control-plane
echo   - role: worker
echo   - role: worker
echo   - role: worker
) > %TEMP%\kind-nyx.yaml

REM Create or use existing kind cluster
echo Creating kind cluster 'nyx'...
kind get clusters | findstr /C:"nyx" >nul 2>&1
if %errorLevel% neq 0 (
    kind create cluster --name nyx --config %TEMP%\kind-nyx.yaml
) else (
    echo Using existing 'nyx' cluster
)

REM Build and load local image
echo Building Nyx daemon local image...
docker build -f Dockerfile.legacy -t nyx-daemon:local .
if %errorLevel% neq 0 (
    echo [!] Docker build failed. Please check Dockerfile.legacy exists.
    pause
    exit /b 1
)

echo Loading image into kind cluster...
kind load docker-image nyx-daemon:local --name nyx

REM Create namespace
echo Creating Kubernetes namespace...
kubectl create namespace nyx --dry-run=client -o yaml | kubectl apply -f -

REM Add Prometheus Operator for ServiceMonitor support
echo Installing Prometheus Operator...
helm repo add prometheus-community https://prometheus-community.github.io/helm-charts
helm repo update
helm install prometheus-operator prometheus-community/kube-prometheus-stack --namespace monitoring --create-namespace --set grafana.enabled=false --set alertmanager.enabled=false --set prometheus.enabled=false --set kubeStateMetrics.enabled=false --set nodeExporter.enabled=false --set prometheusOperator.enabled=true

REM Deploy Nyx with multi-node configuration
echo Deploying Nyx with multi-node performance testing...
helm upgrade --install nyx ./charts/nyx -n nyx --set image.repository=nyx-daemon --set image.tag=local --set image.pullPolicy=IfNotPresent --set replicaCount=6 --set bench.enabled=true --set bench.replicas=3 --set bench.testDurationSeconds=45 --set bench.concurrentConnections=15 --set pdb.enabled=true --set pdb.minAvailable=3 --set serviceMonitor.enabled=true --set probes.startup.enabled=false --set probes.liveness.enabled=false --set probes.readiness.enabled=false

REM Wait for deployment
echo Waiting for Nyx deployment to complete...
kubectl rollout status -n nyx deploy/nyx --timeout=300s

REM Wait for benchmark job completion
echo Waiting for benchmark job to complete...
kubectl wait -n nyx --for=condition=complete job/nyx-bench --timeout=600s

REM Show results
echo ======================================================
echo MULTI-NODE PERFORMANCE BENCHMARK RESULTS
echo ======================================================
kubectl logs -n nyx job/nyx-bench

echo ======================================================
echo CLUSTER STATUS
echo ======================================================
kubectl get pods,svc,pdb -n nyx -o wide

echo ======================================================
echo NODE DISTRIBUTION
echo ======================================================
kubectl get pods -n nyx -o wide | findstr /V "NAME" | for /f "tokens=7" %%i in ('more') do echo %%i | sort

echo ======================================================
echo DEPLOYMENT COMPLETE!
echo Check the benchmark results above for performance rating
echo ======================================================
pause
