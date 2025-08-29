@echo off
echo 🌐 NYX MULTI-NODE DEPLOYMENT TEST
echo ==================================

REM Clean up existing deployment
kubectl delete deployment,job,service,configmap -l app=nyx-multinode --ignore-not-found=true

REM Create multinode YAML
echo apiVersion: apps/v1 > multinode.yaml
echo kind: Deployment >> multinode.yaml
echo metadata: >> multinode.yaml
echo   name: nyx-multinode >> multinode.yaml
echo   labels: >> multinode.yaml
echo     app: nyx-multinode >> multinode.yaml
echo spec: >> multinode.yaml
echo   replicas: 6 >> multinode.yaml
echo   selector: >> multinode.yaml
echo     matchLabels: >> multinode.yaml
echo       app: nyx-multinode >> multinode.yaml
echo   template: >> multinode.yaml
echo     metadata: >> multinode.yaml
echo       labels: >> multinode.yaml
echo         app: nyx-multinode >> multinode.yaml
echo     spec: >> multinode.yaml
echo       affinity: >> multinode.yaml
echo         podAntiAffinity: >> multinode.yaml
echo           preferredDuringSchedulingIgnoredDuringExecution: >> multinode.yaml
echo           - weight: 100 >> multinode.yaml
echo             podAffinityTerm: >> multinode.yaml
echo               labelSelector: >> multinode.yaml
echo                 matchLabels: >> multinode.yaml
echo                   app: nyx-multinode >> multinode.yaml
echo               topologyKey: kubernetes.io/hostname >> multinode.yaml
echo       containers: >> multinode.yaml
echo       - name: nyx >> multinode.yaml
echo         image: alpine:3.19 >> multinode.yaml
echo         command: ["/bin/sh"] >> multinode.yaml
echo         args: ["-c", "apk add --no-cache netcat-openbsd ^&^& hostname ^> /tmp/node-id ^&^& echo 'Node ready' ^&^& while true; do echo 'HTTP/1.1 200 OK\r\n\r\nOK' ^| nc -l -p 43300; done"] >> multinode.yaml
echo         ports: >> multinode.yaml
echo         - containerPort: 43300 >> multinode.yaml
echo         resources: >> multinode.yaml
echo           requests: >> multinode.yaml
echo             cpu: 10m >> multinode.yaml
echo             memory: 16Mi >> multinode.yaml
echo --- >> multinode.yaml
echo apiVersion: v1 >> multinode.yaml
echo kind: Service >> multinode.yaml
echo metadata: >> multinode.yaml
echo   name: nyx-multinode >> multinode.yaml
echo   labels: >> multinode.yaml
echo     app: nyx-multinode >> multinode.yaml
echo spec: >> multinode.yaml
echo   selector: >> multinode.yaml
echo     app: nyx-multinode >> multinode.yaml
echo   ports: >> multinode.yaml
echo   - port: 43300 >> multinode.yaml
echo     targetPort: 43300 >> multinode.yaml

kubectl apply -f multinode.yaml

echo.
echo ⚡ Waiting for multi-node deployment (6 pods)...
kubectl wait --for=condition=available deployment/nyx-multinode --timeout=120s

echo.
echo 📊 Pod distribution across nodes:
kubectl get pods -l app=nyx-multinode -o wide

echo.
echo 🌐 Multi-node connectivity test:
kubectl run multinode-test --image=alpine:3.19 --rm -it --restart=Never -- sh -c "apk add --no-cache netcat-openbsd && echo 'Testing 10 connections...' && for i in $(seq 1 10); do if nc -z nyx-multinode 43300; then echo \"Test $i: ✅\"; else echo \"Test $i: ❌\"; fi; done"

echo.
echo 🎉 MULTI-NODE DEPLOYMENT COMPLETE!

del multinode.yaml
