#!/bin/sh
set -e
echo "======================================================="
echo "NYX NETWORK DAEMON - MULTI-NODE PERFORMANCE BENCHMARK"
echo "======================================================="
echo "Bench Pod: $(hostname)"
echo "Target service: ${TARGET_SERVICE}:${TARGET_PORT}"
echo "Headless service: ${TARGET_HEADLESS}"
echo "Metrics port: ${METRICS_PORT}"
echo "Test duration: ${TEST_DURATION_SECONDS:-60} seconds"
echo "Concurrent connections: ${CONCURRENT_CONNECTIONS:-10}"
echo ""

# Install required tools
echo "Installing performance testing tools..."
apk add --no-cache curl wget netcat-openbsd iperf3 bind-tools 2>/dev/null || true

# Wait for all daemon pods to be ready
echo "== Waiting for Nyx Daemon Pods =="
for i in $(seq 1 30); do
  READY_PODS=$(nslookup "${TARGET_HEADLESS}" 2>/dev/null | grep -c "Address:" | tail -1 || echo "0")
  echo "Ready daemon pods: $READY_PODS"
  if [ "$READY_PODS" -ge 3 ]; then
    echo "✅ Sufficient pods ready for testing"
    break
  fi
  sleep 2
done
echo ""

# Discover all daemon pod IPs using nslookup (Alpine compatible)
DAEMON_IPS=$(nslookup "${TARGET_HEADLESS}" 2>/dev/null | grep "Address:" | grep -v "#" | awk '{print $2}' | sort || echo "")
echo "== Discovered Daemon Pods =="
for ip in $DAEMON_IPS; do
  echo "  Daemon pod: $ip"
done
echo ""

# Multi-node connectivity matrix test
echo "== Multi-Node Connectivity Matrix =="
SUCCESS_COUNT=0
TOTAL_TESTS=0
for daemon_ip in $DAEMON_IPS; do
  echo "Testing connectivity to daemon $daemon_ip..."
  TOTAL_TESTS=$((TOTAL_TESTS + 1))
  if nc -vz -w 3 "$daemon_ip" "${TARGET_PORT}" 2>&1; then
    echo "  ✅ Connection successful"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
  else
    echo "  ❌ Connection failed"
  fi
done
echo "Connectivity matrix: $SUCCESS_COUNT/$TOTAL_TESTS successful"
echo ""

# Load balancing verification (Alpine shell compatible)
echo "== Load Balancing Verification =="
LB_TESTS=50
LB_SUCCESS=0
for i in $(seq 1 $LB_TESTS); do
  if command -v curl >/dev/null 2>&1; then
    RESPONSE=$(curl -m 2 -s "http://${TARGET_SERVICE}:${METRICS_PORT}/health" 2>/dev/null | head -c 100 || echo "timeout")
    if [ "$RESPONSE" != "timeout" ] && [ -n "$RESPONSE" ]; then
      LB_SUCCESS=$((LB_SUCCESS + 1))
    fi
  fi
  sleep 0.1
done
echo "Load balancer health checks: $LB_SUCCESS/$LB_TESTS successful"
echo ""

# Concurrent connection stress test
echo "== Concurrent Connection Stress Test =="
CONCURRENT=${CONCURRENT_CONNECTIONS:-10}
DURATION=${TEST_DURATION_SECONDS:-30}
STRESS_LOG="/tmp/stress_results"
> "$STRESS_LOG"

echo "Starting $CONCURRENT concurrent connections for $DURATION seconds..."
for i in $(seq 1 $CONCURRENT); do
  (
    END_TIME=$(($(date +%s) + DURATION))
    CONN_SUCCESS=0
    CONN_TOTAL=0
    while [ $(date +%s) -lt $END_TIME ]; do
      CONN_TOTAL=$((CONN_TOTAL + 1))
      if nc -z -w 1 "${TARGET_SERVICE}" "${TARGET_PORT}" 2>/dev/null; then
        CONN_SUCCESS=$((CONN_SUCCESS + 1))
      fi
      sleep 0.1
    done
    echo "Worker $i: $CONN_SUCCESS/$CONN_TOTAL connections successful" >> "$STRESS_LOG"
  ) &
done

# Wait for all background jobs
wait
cat "$STRESS_LOG" | sed 's/^/  /'
echo ""

# Throughput test with multiple daemon pods
echo "== Multi-Pod Throughput Test =="
THROUGHPUT_TOTAL=0
THROUGHPUT_TESTS=0
for daemon_ip in $DAEMON_IPS; do
  echo "Testing throughput to daemon $daemon_ip..."
  THROUGHPUT_TESTS=$((THROUGHPUT_TESTS + 1))
  
  # Simple throughput test using nc
  START_TIME=$(date +%s)
  echo "test data for throughput measurement" | nc -w 2 "$daemon_ip" "${TARGET_PORT}" >/dev/null 2>&1 && THROUGHPUT_TOTAL=$((THROUGHPUT_TOTAL + 1)) || true
  END_TIME=$(date +%s)
  RESPONSE_TIME=$((END_TIME - START_TIME))
  
  echo "  Response time: ${RESPONSE_TIME}s"
done
echo "Throughput test completed: $THROUGHPUT_TOTAL/$THROUGHPUT_TESTS pods responded"
echo ""

# Resource utilization check via metrics
echo "== Resource Utilization Check =="
for daemon_ip in $DAEMON_IPS; do
  echo "Checking metrics from daemon $daemon_ip..."
  if command -v curl >/dev/null 2>&1; then
    METRICS=$(curl -m 3 -s "http://$daemon_ip:${METRICS_PORT}/metrics" 2>/dev/null | grep -E "(cpu|memory|network)" | head -5 || echo "")
    if [ -n "$METRICS" ]; then
      echo "  Sample metrics:"
      echo "$METRICS" | sed 's/^/    /'
    else
      echo "  ⚠️  No metrics available"
    fi
  fi
done
echo ""

# Service discovery resilience test
echo "== Network Resilience Test =="
echo "Testing service discovery resilience..."
for i in $(seq 1 10); do
  DISCOVERED=$(nslookup "${TARGET_SERVICE}" 2>/dev/null | grep -c "Address:" || echo "0")
  if [ "$DISCOVERED" -gt 0 ]; then
    echo "  Round $i: Service discovery OK"
  else
    echo "  Round $i: Service discovery FAILED"
  fi
  sleep 1
done
echo ""

# Final performance summary
echo "== MULTI-NODE PERFORMANCE SUMMARY =="
echo "🎯 Test Configuration:"
DAEMON_COUNT=$(echo $DAEMON_IPS | wc -w)
echo "  - Daemon pods tested: $DAEMON_COUNT"
echo "  - Concurrent connections: $CONCURRENT"
echo "  - Test duration: ${DURATION}s"
echo "  - Load balancer tests: $LB_TESTS"
echo ""
echo "📊 Results:"
echo "  - Pod connectivity: $SUCCESS_COUNT/$TOTAL_TESTS"
echo "  - Load balancing: $LB_SUCCESS/$LB_TESTS"
echo "  - Throughput responses: $THROUGHPUT_TOTAL/$THROUGHPUT_TESTS"
echo ""

# Performance rating
if [ $TOTAL_TESTS -gt 0 ]; then
  CONNECTIVITY_RATE=$((SUCCESS_COUNT * 100 / TOTAL_TESTS))
else
  CONNECTIVITY_RATE=0
fi

if [ $LB_TESTS -gt 0 ]; then
  LB_RATE=$((LB_SUCCESS * 100 / LB_TESTS))
else
  LB_RATE=0
fi

if [ $CONNECTIVITY_RATE -ge 90 ] && [ $LB_RATE -ge 80 ]; then
  echo "🥇 PERFORMANCE RATING: EXCELLENT"
  echo "✅ Multi-node deployment is production-ready!"
  echo "✅ Suitable for high-load distributed applications"
  echo "🚀 Ready for U22 Programming Contest submission!"
elif [ $CONNECTIVITY_RATE -ge 70 ] && [ $LB_RATE -ge 60 ]; then
  echo "🥈 PERFORMANCE RATING: GOOD"
  echo "✅ Multi-node deployment is functional"
  echo "⚠️  Consider tuning for higher loads"
else
  echo "🥉 PERFORMANCE RATING: NEEDS IMPROVEMENT"
  echo "❌ Some connectivity or load balancing issues detected"
  echo "🔧 Review network configuration and resource allocation"
fi
echo "======================================================="
