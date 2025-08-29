@echo off
setlocal enabledelayedexpansion

echo ======================================================
echo NYX NETWORK - WINDOWS KUBERNETES SETUP
echo ======================================================
echo Setting up Docker Desktop + kubectl + Helm + kind
echo Then deploying Nyx with multi-node performance testing
echo ======================================================

REM Check if running as administrator
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo [!] This script requires administrator privileges
    echo Please run as administrator and try again
    pause
    exit /b 1
)

REM Install Chocolatey if not present
where choco >nul 2>&1
if %errorLevel% neq 0 (
    echo Installing Chocolatey package manager...
    powershell -Command "Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))"
)

REM Install Docker Desktop
echo Installing Docker Desktop...
choco install docker-desktop -y

REM Install kubectl
echo Installing kubectl...
choco install kubernetes-cli -y

REM Install Helm
echo Installing Helm...
choco install kubernetes-helm -y

REM Install kind
echo Installing kind...
choco install kind -y

echo ======================================================
echo SETUP COMPLETE! 
echo Please restart Docker Desktop and run:
echo   nyx-deploy.bat
echo ======================================================
pause
