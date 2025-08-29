#!/bin/bash
set -e

echo "🌐 NYX SIMPLE MULTI-NODE TEST"
echo "=============================="

# Clean up
kubectl delete deployment,service -l app=nyx-multinode --ignore-not-found=true

# Create simple multi-node deployment
kubectl apply -f - <<EOF
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
        args: 
          - "-c" 
          - "apk add --no-cache netcat-openbsd && echo 'Node ready on port 43300' && while true; do echo 'HTTP/1.1 200 OK\r\n\r\nOK' | nc -l -p 43300; done"
        ports:
        - containerPort: 43300
        resources:
          requests:
            cpu: 10m
            memory: 16Mi
          limits:
            cpu: 50m
            memory: 64Mi
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
EOF

echo "⚡ Waiting for multi-node deployment..."
kubectl wait --for=condition=available deployment/nyx-multinode --timeout=120s

echo ""
echo "📊 Pod distribution across nodes:"
kubectl get pods -l app=nyx-multinode -o wide

echo ""
echo "🔍 Simple connectivity test:"
kubectl run test-multinode --image=alpine:3.19 --rm -i --restart=Never -- sh -c "
  apk add --no-cache netcat-openbsd &&
  echo 'Testing 10 connections to load balanced service...' &&
  for i in \$(seq 1 10); do
    if nc -z nyx-multinode 43300; then
      echo \"Test \$i: ✅ SUCCESS\"
    else
      echo \"Test \$i: ❌ FAILED\"
    fi
    sleep 0.5
  done &&
  echo '🏆 Multi-node load balancing test complete!'
"

echo ""
echo "🎉 MULTI-NODE DEPLOYMENT COMPLETE!"
echo "🌐 Total pods: $(kubectl get pods -l app=nyx-multinode --no-headers | wc -l)"
echo "📍 Unique nodes: $(kubectl get pods -l app=nyx-multinode -o wide --no-headers | awk '{print $7}' | sort -u | wc -l)"
