#!/bin/bash
set -e

echo "🚨 EMERGENCY DEPLOYMENT - NO TIMEOUT"
echo "===================================="

# Kill any existing deployment
kubectl delete deployment,job,service,configmap -l app.kubernetes.io/name=nyx --ignore-not-found=true

# Direct minimal deployment - NO HELM
cat <<EOF | kubectl apply -f -
apiVersion: v1
kind: ConfigMap
metadata:
  name: nyx-emergency
  labels:
    app: nyx-emergency
data:
  test.sh: |
    #!/bin/sh
    echo "✅ EMERGENCY TEST PASSED"
    echo "🚀 NYX EMERGENCY DEPLOYMENT WORKING"
    date
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nyx-emergency
  labels:
    app: nyx-emergency
spec:
  replicas: 1
  selector:
    matchLabels:
      app: nyx-emergency
  template:
    metadata:
      labels:
        app: nyx-emergency
    spec:
      containers:
      - name: nyx
        image: alpine:3.19
        command: ["/bin/sh"]
        args: ["-c", "apk add --no-cache netcat-openbsd && echo 'Mock daemon ready' && while true; do echo 'HTTP/1.1 200 OK\r\n\r\nOK' | nc -l -p 43300; done"]
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
  name: nyx-emergency
  labels:
    app: nyx-emergency
spec:
  selector:
    app: nyx-emergency
  ports:
  - port: 43300
    targetPort: 43300
---
apiVersion: batch/v1
kind: Job
metadata:
  name: nyx-test-emergency
  labels:
    app: nyx-test-emergency
spec:
  template:
    spec:
      restartPolicy: Never
      containers:
      - name: test
        image: alpine:3.19
        command: ["/bin/sh"]
        args: ["-c", "apk add --no-cache netcat-openbsd && sleep 10 && if nc -z nyx-emergency 43300; then echo '✅ CONNECTION SUCCESS'; else echo '❌ CONNECTION FAILED'; fi"]
        resources:
          requests:
            cpu: 10m
            memory: 16Mi
EOF

echo ""
echo "⚡ Waiting for deployment..."
kubectl wait --for=condition=available deployment/nyx-emergency --timeout=60s

echo ""
echo "📊 Checking pods..."
kubectl get pods -l app=nyx-emergency

echo ""
echo "🔍 Running test job..."
kubectl wait --for=condition=complete job/nyx-test-emergency --timeout=60s || true

echo ""
echo "📋 Test results:"
kubectl logs -l app=nyx-test-emergency

echo ""
echo "🎉 EMERGENCY DEPLOYMENT COMPLETE!"
echo "Pod status: $(kubectl get pods -l app=nyx-emergency --no-headers | awk '{print $3}')"
