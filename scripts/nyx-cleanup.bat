@echo off
setlocal enabledelayedexpansion

echo ======================================================
echo NYX NETWORK - CLEANUP SCRIPT
echo ======================================================
echo Cleaning up Nyx deployment and kind cluster
echo ======================================================

REM Clean up Nyx resources
echo Cleaning up Nyx resources...
kubectl delete job nyx-bench -n nyx --ignore-not-found=true 2>nul
helm uninstall nyx -n nyx 2>nul || echo Nyx uninstalled
kubectl delete namespace nyx --ignore-not-found=true 2>nul

REM Clean up Prometheus Operator
echo Cleaning up Prometheus Operator...
helm uninstall prometheus-operator -n monitoring 2>nul || echo Prometheus Operator uninstalled
kubectl delete namespace monitoring --ignore-not-found=true 2>nul

REM Delete kind cluster
echo Deleting kind cluster 'nyx'...
kind get clusters | findstr /C:"nyx" >nul 2>&1
if %errorLevel% equ 0 (
    kind delete cluster --name nyx
    echo Kind cluster 'nyx' deleted successfully
) else (
    echo Kind cluster 'nyx' not found
)

echo ======================================================
echo CLEANUP COMPLETE!
echo Run nyx-deploy.bat to redeploy fresh
echo ======================================================
pause
