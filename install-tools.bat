@echo off
echo ======================================================
echo NYX NETWORK - QUICK WINDOWS INSTALLER
echo ======================================================
echo Installing kubectl, kind, and Docker Desktop...
echo Note: This requires administrator privileges
echo ======================================================

REM Check if running as administrator
net session >nul 2>&1
if %errorlevel% neq 0 (
    echo [ERROR] This script requires administrator privileges
    echo Please run PowerShell as Administrator and try again
    pause
    exit /b 1
)

echo [1/4] Installing Chocolatey package manager...
powershell -Command "Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))"

echo [2/4] Installing kubectl...
choco install kubernetes-cli -y

echo [3/4] Installing kind...
choco install kind -y

echo [4/4] Installing Docker Desktop...
choco install docker-desktop -y

echo ======================================================
echo INSTALLATION COMPLETE
echo ======================================================
echo Please restart your terminal and run:
echo   docker --version
echo   kubectl version --client
echo   kind version
echo ======================================================
echo Then create cluster with: kind create cluster --config kind-config.yaml
echo ======================================================
pause
