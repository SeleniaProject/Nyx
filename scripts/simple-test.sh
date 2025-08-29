#!/bin/sh
# Simple connectivity test for Alpine compatibility
# Tests TCP connection to nyx daemon on port 43300

echo "=== NYX CONNECTIVITY TEST ==="
echo "Testing connection to nyx daemon..."

# Use nc (netcat) for simple TCP connectivity test
if nc -z nyx-service 43300 2>/dev/null; then
    echo "✓ TCP connection to nyx-service:43300 successful"
    
    # Test HTTP metrics endpoint
    if command -v wget >/dev/null 2>&1; then
        echo "Testing HTTP metrics endpoint..."
        if wget -q -O - http://nyx-service:9090/metrics >/dev/null 2>&1; then
            echo "✓ HTTP metrics endpoint accessible"
        else
            echo "✗ HTTP metrics endpoint failed"
        fi
    elif command -v curl >/dev/null 2>&1; then
        echo "Testing HTTP metrics endpoint..."
        if curl -s http://nyx-service:9090/metrics >/dev/null 2>&1; then
            echo "✓ HTTP metrics endpoint accessible"
        else
            echo "✗ HTTP metrics endpoint failed"
        fi
    else
        echo "⚠ No wget/curl available for HTTP test"
    fi
    
    echo "=== CONNECTIVITY TEST PASSED ==="
    exit 0
else
    echo "✗ TCP connection to nyx-service:43300 failed"
    echo "=== CONNECTIVITY TEST FAILED ==="
    exit 1
fi
