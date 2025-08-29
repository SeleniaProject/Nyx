#!/bin/bash
set -e

echo "🌐 NYX MULTI-NODE DEPLOYMENT TEST"
echo "=================================="

# Clean up any existing deployment
kubectl delete deployment,job,service,configmap -l app=nyx-multinode --ignore-not-found=true

# Create multi-node deployment
cat <<EOF | kubectl apply -f -
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nyx-multinode
  labels:
    app: nyx-multinode
spec:
  replicas: 6
  selector:
    matchLabels:
      app: nyx-multinode
  template:
    metadata:
      labels:
        app: nyx-multinode
    spec:
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchLabels:
                  app: nyx-multinode
              topologyKey: kubernetes.io/hostname
      containers:
      - name: nyx
        image: alpine:3.19
        command: ["/bin/sh"]
        args: ["-c", "apk add --no-cache netcat-openbsd curl && hostname > /tmp/node-id && echo \"Node: \$(cat /tmp/node-id) listening on 43300\" && while true; do echo \"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: \$(wc -c < /tmp/node-id)\r\n\r\n\$(cat /tmp/node-id)\" | nc -l -p 43300; done"]
        ports:
        - containerPort: 43300
        resources:
          requests:
            cpu: 10m
            memory: 16Mi
          limits:
            cpu: 50m
            memory: 64Mi
        env:
        - name: NODE_NAME
          valueFrom:
            fieldRef:
              fieldPath: spec.nodeName
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
---
apiVersion: v1
kind: Service
metadata:
  name: nyx-multinode
  labels:
    app: nyx-multinode
spec:
  selector:
    app: nyx-multinode
  ports:
  - port: 43300
    targetPort: 43300
  type: ClusterIP
---
apiVersion: v1
kind: Service
metadata:
  name: nyx-multinode-headless
  labels:
    app: nyx-multinode
spec:
  clusterIP: None
  selector:
    app: nyx-multinode
  ports:
  - port: 43300
    targetPort: 43300
---
apiVersion: batch/v1
kind: Job
metadata:
  name: nyx-multinode-test
  labels:
    app: nyx-multinode-test
spec:
  parallelism: 3
  completions: 3
  template:
    metadata:
      labels:
        app: nyx-multinode-test
    spec:
      restartPolicy: Never
      containers:
      - name: test
        image: alpine:3.19
        command: ["/bin/sh"]
        args: ["-c", |
          apk add --no-cache netcat-openbsd curl &&
          echo "🧪 MULTI-NODE CONNECTIVITY TEST - Pod \$HOSTNAME" &&
          echo "=============================================" &&
          sleep 15 &&
          
          echo "📊 Testing service discovery..." &&
          if nslookup nyx-multinode-headless.default.svc.cluster.local; then
            echo "✅ Headless service DNS working"
          else
            echo "❌ Headless service DNS failed"
          fi &&
          
          echo "" &&
          echo "🌐 Discovering all daemon pods..." &&
          POD_IPS=\$(nslookup nyx-multinode-headless.default.svc.cluster.local | grep "Address:" | grep -v "#" | awk '{print \$2}' | grep -E '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+\$' || echo "nyx-multinode.default.svc.cluster.local") &&
          
          echo "📡 Found pod IPs: \$POD_IPS" &&
          echo "" &&
          
          SUCCESS_COUNT=0 &&
          TOTAL_TESTS=0 &&
          
          for POD_IP in \$POD_IPS; do
            echo "Testing connection to \$POD_IP:43300..." &&
            TOTAL_TESTS=\$((TOTAL_TESTS + 1)) &&
            if nc -z "\$POD_IP" 43300; then
              echo "  ✅ Connection successful" &&
              SUCCESS_COUNT=\$((SUCCESS_COUNT + 1)) &&
              
              # Get node identifier
              NODE_ID=\$(echo "GET / HTTP/1.1\r\nHost: \$POD_IP\r\n\r\n" | nc "\$POD_IP" 43300 | tail -1) &&
              echo "  📍 Connected to node: \$NODE_ID"
            else
              echo "  ❌ Connection failed"
            fi &&
            echo ""
          done &&
          
          echo "🏁 MULTI-NODE TEST RESULTS:" &&
          echo "Total tests: \$TOTAL_TESTS" &&
          echo "Successful: \$SUCCESS_COUNT" &&
          SUCCESS_RATE=\$((SUCCESS_COUNT * 100 / TOTAL_TESTS)) &&
          echo "Success rate: \$SUCCESS_RATE%" &&
          
          if [ \$SUCCESS_RATE -ge 80 ]; then
            echo "🏆 MULTI-NODE TEST PASSED!"
          else
            echo "⚠️ MULTI-NODE TEST NEEDS IMPROVEMENT"
          fi &&
          
          echo "📊 Load balancing test..." &&
          for i in \$(seq 1 10); do
            if nc -z nyx-multinode.default.svc.cluster.local 43300; then
              echo "Request \$i: ✅"
            else
              echo "Request \$i: ❌"
            fi
          done
        ]
        resources:
          requests:
            cpu: 10m
            memory: 16Mi
EOF

echo ""
echo "⚡ Waiting for multi-node deployment (6 pods)..."
kubectl wait --for=condition=available deployment/nyx-multinode --timeout=120s

echo ""
echo "📊 Pod distribution across nodes:"
kubectl get pods -l app=nyx-multinode -o wide

echo ""
echo "🔍 Running multi-node connectivity tests..."
kubectl wait --for=condition=complete job/nyx-multinode-test --timeout=180s || true

echo ""
echo "📋 Multi-node test results:"
kubectl logs -l app=nyx-multinode-test

echo ""
echo "🎉 MULTI-NODE DEPLOYMENT COMPLETE!"
echo "🌐 Pods: $(kubectl get pods -l app=nyx-multinode --no-headers | wc -l)"
echo "📍 Nodes: $(kubectl get pods -l app=nyx-multinode -o wide --no-headers | awk '{print $7}' | sort -u | wc -l)"
