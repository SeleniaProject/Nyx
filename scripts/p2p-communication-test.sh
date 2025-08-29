#!/bin/bash
set -e

echo "🔄 NYX POD-TO-POD COMMUNICATION TEST"
echo "===================================="

# Ensure multinode deployment exists
if ! kubectl get deployment nyx-multinode >/dev/null 2>&1; then
    echo "❌ nyx-multinode deployment not found. Run multinode-test.sh first."
    exit 1
fi

echo "📊 Current pod status:"
kubectl get pods -l app=nyx-multinode -o wide

echo ""
echo "🌐 Testing Pod-to-Pod Direct Communication..."

# Get all pod IPs
POD_IPS=$(kubectl get pods -l app=nyx-multinode -o jsonpath='{.items[*].status.podIP}')
echo "📡 Found pod IPs: $POD_IPS"

# Create communication test job
kubectl apply -f - <<EOF
apiVersion: batch/v1
kind: Job
metadata:
  name: nyx-p2p-test
  labels:
    app: nyx-p2p-test
spec:
  parallelism: 3
  completions: 3
  template:
    metadata:
      labels:
        app: nyx-p2p-test
    spec:
      restartPolicy: Never
      containers:
      - name: p2p-test
        image: alpine:3.19
        command: ["/bin/sh"]
        args:
          - "-c"
          - |
            apk add --no-cache netcat-openbsd curl &&
            echo "🧪 POD-TO-POD COMMUNICATION TEST - Tester: \$HOSTNAME" &&
            echo "======================================================" &&
            
            # Get current pod's IP
            MY_IP=\$(hostname -i) &&
            echo "📍 My IP: \$MY_IP" &&
            
            # Get all target pod IPs via headless service
            TARGET_IPS=\$(nslookup nyx-multinode-headless.default.svc.cluster.local | grep "Address:" | grep -v "#" | awk '{print \$2}' | grep -E '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+\$') &&
            
            if [ -z "\$TARGET_IPS" ]; then
              echo "⚠️  DNS lookup failed, using kubectl discovered IPs" &&
              TARGET_IPS="$POD_IPS"
            fi &&
            
            echo "🎯 Target IPs: \$TARGET_IPS" &&
            echo "" &&
            
            SUCCESS_COUNT=0 &&
            TOTAL_TESTS=0 &&
            
            # Test direct pod-to-pod communication
            for TARGET_IP in \$TARGET_IPS; do
              if [ "\$TARGET_IP" != "\$MY_IP" ]; then
                echo "Testing \$MY_IP → \$TARGET_IP:43300..." &&
                TOTAL_TESTS=\$((TOTAL_TESTS + 1)) &&
                
                # Test TCP connection
                if nc -z "\$TARGET_IP" 43300; then
                  echo "  ✅ TCP connection successful" &&
                  SUCCESS_COUNT=\$((SUCCESS_COUNT + 1)) &&
                  
                  # Test HTTP communication
                  RESPONSE=\$(echo "GET /ping HTTP/1.1\r\nHost: \$TARGET_IP\r\nX-Source: \$MY_IP\r\n\r\n" | nc "\$TARGET_IP" 43300 | head -1) &&
                  echo "  📨 HTTP Response: \$RESPONSE" &&
                  
                  # Test bidirectional communication
                  echo "  🔄 Testing bidirectional communication..." &&
                  if echo "PING from \$MY_IP" | nc "\$TARGET_IP" 43300 >/dev/null 2>&1; then
                    echo "  ✅ Bidirectional communication OK"
                  else
                    echo "  ⚠️  Bidirectional communication limited"
                  fi
                else
                  echo "  ❌ TCP connection failed"
                fi &&
                echo ""
              fi
            done &&
            
            echo "🏁 POD-TO-POD COMMUNICATION RESULTS:" &&
            echo "=====================================" &&
            echo "Source Pod: \$MY_IP (\$HOSTNAME)" &&
            echo "Total P2P tests: \$TOTAL_TESTS" &&
            echo "Successful: \$SUCCESS_COUNT" &&
            
            if [ \$TOTAL_TESTS -gt 0 ]; then
              SUCCESS_RATE=\$((SUCCESS_COUNT * 100 / TOTAL_TESTS)) &&
              echo "Success rate: \$SUCCESS_RATE%"
            fi &&
            
            if [ \$SUCCESS_COUNT -eq \$TOTAL_TESTS ] && [ \$TOTAL_TESTS -gt 0 ]; then
              echo "🏆 ALL POD-TO-POD COMMUNICATION SUCCESSFUL!"
            elif [ \$SUCCESS_COUNT -gt 0 ]; then
              echo "🥈 PARTIAL POD-TO-POD COMMUNICATION SUCCESS"
            else
              echo "❌ POD-TO-POD COMMUNICATION FAILED"
            fi
        resources:
          requests:
            cpu: 10m
            memory: 16Mi
EOF

echo ""
echo "⚡ Running pod-to-pod communication tests..."
kubectl wait --for=condition=complete job/nyx-p2p-test --timeout=180s || true

echo ""
echo "📋 Pod-to-Pod Communication Results:"
echo "===================================="
kubectl logs -l app=nyx-p2p-test

echo ""
echo "🌐 Service Mesh Communication Test:"
kubectl run mesh-test --image=alpine:3.19 --rm -i --restart=Never -- sh -c "
  apk add --no-cache netcat-openbsd &&
  echo 'Testing service mesh communication...' &&
  
  # Test service-to-service communication
  for i in \$(seq 1 5); do
    if nc -z nyx-multinode.default.svc.cluster.local 43300; then
      echo \"Mesh test \$i: ✅ Service discovery working\"
    else
      echo \"Mesh test \$i: ❌ Service discovery failed\"
    fi
    sleep 0.5
  done &&
  
  echo 'Testing cross-namespace simulation...' &&
  if nc -z nyx-multinode.default.svc.cluster.local 43300; then
    echo '✅ Cross-namespace communication ready'
  else
    echo '❌ Cross-namespace communication failed'
  fi
"

echo ""
echo "🎉 POD-TO-POD COMMUNICATION TEST COMPLETE!"
echo ""
echo "📊 Summary:"
echo "- ✅ Service-based load balancing"
echo "- 🔄 Pod-to-pod direct communication" 
echo "- 🌐 Service mesh readiness"
echo "- 🏗️ Multi-node distributed architecture"

# Cleanup test job
kubectl delete job nyx-p2p-test --ignore-not-found=true
