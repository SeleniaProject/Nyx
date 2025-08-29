#!/bin/bash
set -e

echo "🔄 NYX ENHANCED P2P COMMUNICATION DEPLOYMENT"
echo "============================================"

# Clean up existing
kubectl delete deployment,service -l app=nyx-p2p --ignore-not-found=true

# Deploy enhanced P2P communication pods
kubectl apply -f - <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nyx-p2p
  labels:
    app: nyx-p2p
spec:
  replicas: 4
  selector:
    matchLabels:
      app: nyx-p2p
  template:
    metadata:
      labels:
        app: nyx-p2p
    spec:
      affinity:
        podAntiAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
          - weight: 100
            podAffinityTerm:
              labelSelector:
                matchLabels:
                  app: nyx-p2p
              topologyKey: kubernetes.io/hostname
      containers:
      - name: nyx-p2p
        image: alpine:3.19
        command: ["/bin/sh"]
        args:
          - "-c"
          - |
            apk add --no-cache netcat-openbsd curl &&
            
            # Get pod info
            MY_IP=\$(hostname -i) &&
            MY_NAME=\$(hostname) &&
            
            echo "🚀 P2P Node \$MY_NAME starting at \$MY_IP:43300" &&
            
            # Start P2P communication daemon
            while true; do
              {
                echo "HTTP/1.1 200 OK"
                echo "Content-Type: application/json"
                echo "X-Pod-Name: \$MY_NAME"
                echo "X-Pod-IP: \$MY_IP"
                echo "X-Timestamp: \$(date -Iseconds)"
                echo ""
                echo "{\"pod\":\"\$MY_NAME\",\"ip\":\"\$MY_IP\",\"status\":\"active\",\"timestamp\":\"\$(date -Iseconds)\",\"connections\":[]}"
              } | nc -l -p 43300
              sleep 0.1
            done &
            
            # Start peer discovery and communication
            sleep 10 &&
            while true; do
              echo "🔍 [\$MY_NAME] Discovering peers..." &&
              
              # Discover peers via headless service
              PEER_IPS=\$(nslookup nyx-p2p-headless.default.svc.cluster.local 2>/dev/null | grep "Address:" | grep -v "#" | awk '{print \$2}' | grep -E '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+\$' || echo "") &&
              
              if [ -n "\$PEER_IPS" ]; then
                for PEER_IP in \$PEER_IPS; do
                  if [ "\$PEER_IP" != "\$MY_IP" ]; then
                    echo "📡 [\$MY_NAME] Connecting to peer \$PEER_IP..." &&
                    PEER_INFO=\$(echo "GET /status HTTP/1.1\r\nHost: \$PEER_IP\r\nX-Source: \$MY_NAME\r\n\r\n" | nc "\$PEER_IP" 43300 2>/dev/null | tail -1) &&
                    if [ -n "\$PEER_INFO" ]; then
                      echo "✅ [\$MY_NAME] Connected to peer: \$PEER_INFO"
                    else
                      echo "❌ [\$MY_NAME] Failed to connect to \$PEER_IP"
                    fi
                  fi
                done
              else
                echo "⚠️  [\$MY_NAME] No peers discovered via DNS"
              fi &&
              
              sleep 30
            done
        ports:
        - containerPort: 43300
        env:
        - name: NODE_NAME
          valueFrom:
            fieldRef:
              fieldPath: spec.nodeName
        - name: POD_NAME
          valueFrom:
            fieldRef:
              fieldPath: metadata.name
        - name: POD_IP
          valueFrom:
            fieldRef:
              fieldPath: status.podIP
        resources:
          requests:
            cpu: 15m
            memory: 32Mi
          limits:
            cpu: 100m
            memory: 128Mi
---
apiVersion: v1
kind: Service
metadata:
  name: nyx-p2p
  labels:
    app: nyx-p2p
spec:
  selector:
    app: nyx-p2p
  ports:
  - port: 43300
    targetPort: 43300
  type: ClusterIP
---
apiVersion: v1
kind: Service
metadata:
  name: nyx-p2p-headless
  labels:
    app: nyx-p2p
spec:
  clusterIP: None
  selector:
    app: nyx-p2p
  ports:
  - port: 43300
    targetPort: 43300
EOF

echo ""
echo "⚡ Waiting for P2P deployment..."
kubectl wait --for=condition=available deployment/nyx-p2p --timeout=120s

echo ""
echo "📊 P2P Pod distribution:"
kubectl get pods -l app=nyx-p2p -o wide

echo ""
echo "🔍 Monitoring P2P communication (30 seconds)..."
sleep 30

echo ""
echo "📋 P2P Communication Logs:"
kubectl logs -l app=nyx-p2p --tail=20

echo ""
echo "🧪 Testing P2P network:"
kubectl run p2p-test --image=alpine:3.19 --rm -i --restart=Never -- sh -c "
  apk add --no-cache netcat-openbsd &&
  echo 'Testing P2P network discovery...' &&
  
  # Test each pod directly
  POD_IPS=\$(nslookup nyx-p2p-headless.default.svc.cluster.local | grep 'Address:' | grep -v '#' | awk '{print \$2}' | grep -E '^[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+\$') &&
  
  for POD_IP in \$POD_IPS; do
    echo \"Testing connection to \$POD_IP...\" &&
    RESPONSE=\$(echo 'GET /status HTTP/1.1\r\nHost: \$POD_IP\r\n\r\n' | nc \$POD_IP 43300 | tail -1) &&
    if [ -n \"\$RESPONSE\" ]; then
      echo \"✅ Pod \$POD_IP response: \$RESPONSE\"
    else
      echo \"❌ Pod \$POD_IP no response\"
    fi
  done
"

echo ""
echo "🎉 P2P COMMUNICATION DEPLOYMENT COMPLETE!"
echo "🔄 Pods are actively discovering and communicating with each other"
echo "📊 Check logs: kubectl logs -l app=nyx-p2p -f"
