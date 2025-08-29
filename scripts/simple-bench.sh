#!/bin/sh
set -e
echo "======================================================"
echo "NYX NETWORK - SIMPLE CONNECTIVITY TEST"
echo "======================================================"
echo "Bench Pod: $(hostname)"
echo "Target service: ${TARGET_SERVICE}:${TARGET_PORT}"
echo "Headless service: ${TARGET_HEADLESS}"
echo ""

# Install minimal tools
echo "Installing basic tools..."
apk add --no-cache curl netcat-openbsd bind-tools 2>/dev/null || true

# Simple service discovery test
echo "== Service Discovery Test =="
echo "Checking if service exists..."
nslookup "${TARGET_SERVICE}" || echo "Service not found"
nslookup "${TARGET_HEADLESS}" || echo "Headless service not found"

# List all discovered daemon IPs
echo "== Discovered Daemon Pods =="
DAEMON_IPS=$(nslookup "${TARGET_HEADLESS}" 2>/dev/null | grep "Address:" | grep -v "#" | awk '{print $2}' | sort || echo "")
if [ -z "$DAEMON_IPS" ]; then
    echo "❌ No daemon pods discovered"
    exit 0
fi

for ip in $DAEMON_IPS; do
    echo "  Daemon pod: $ip"
done

# Test connectivity to each daemon
echo "== Connectivity Test =="
SUCCESS_COUNT=0
TOTAL_TESTS=0

for daemon_ip in $DAEMON_IPS; do
    echo "Testing connection to $daemon_ip:${TARGET_PORT}..."
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    # Try to connect with short timeout
    if nc -z -w 2 "$daemon_ip" "${TARGET_PORT}" 2>/dev/null; then
        echo "  ✅ Connection successful"
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    else
        echo "  ❌ Connection failed"
        # Debug: check if daemon is actually running
        echo "  Debug: Trying to reach any port..."
        nc -z -w 1 "$daemon_ip" 8080 2>/dev/null && echo "    Port 8080 is open" || echo "    No ports responding"
    fi
done

# Simple health check
echo "== Service Health Check =="
if command -v curl >/dev/null 2>&1; then
    echo "Testing service health endpoint..."
    if curl -m 3 -s "http://${TARGET_SERVICE}:${METRICS_PORT}/health" 2>/dev/null; then
        echo "✅ Health endpoint responsive"
    else
        echo "❌ Health endpoint not responding"
    fi
fi

# Summary
echo "== CONNECTIVITY SUMMARY =="
echo "🎯 Test Results:"
echo "  - Daemon pods discovered: $(echo $DAEMON_IPS | wc -w)"
echo "  - Successful connections: $SUCCESS_COUNT/$TOTAL_TESTS"

if [ $SUCCESS_COUNT -gt 0 ]; then
    echo "🥇 RESULT: BASIC CONNECTIVITY OK"
    echo "✅ Daemons are reachable"
else
    echo "🥉 RESULT: CONNECTIVITY ISSUES"
    echo "❌ Check daemon configuration and port bindings"
fi
echo "======================================================"
