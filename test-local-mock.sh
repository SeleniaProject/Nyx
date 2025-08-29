#!/bin/sh
echo "=== NYX TCP MOCK DAEMON TEST ==="

# Install required tools
apk add --no-cache python3 netcat-openbsd curl 2>/dev/null || true

# Start mock daemon in background
python3 -c "
import socket
import threading
import time
from http.server import HTTPServer, BaseHTTPRequestHandler

class MetricsHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header('Content-type', 'text/plain')
        self.end_headers()
        metrics = '''# HELP nyx_connections_total Total connections
nyx_connections_total 42
'''
        self.wfile.write(metrics.encode())
    def log_message(self, format, *args): pass

def tcp_server():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(('0.0.0.0', 43300))
    s.listen(5)
    print('Mock daemon listening on port 43300...')
    while True:
        try:
            client, addr = s.accept()
            print(f'Connection from {addr}')
            client.send(b'HTTP/1.1 200 OK\r\n\r\n{\"status\":\"ok\"}')
            client.close()
        except: pass

def http_server():
    httpd = HTTPServer(('0.0.0.0', 9090), MetricsHandler)
    httpd.serve_forever()

tcp_thread = threading.Thread(target=tcp_server, daemon=True)
http_thread = threading.Thread(target=http_server, daemon=True)
tcp_thread.start()
http_thread.start()

try:
    while True: time.sleep(1)
except KeyboardInterrupt:
    print('Shutting down...')
" &

# Wait for servers to start
sleep 3

echo "Testing TCP connection..."
if nc -z localhost 43300; then
    echo "✅ TCP port 43300 OK"
else
    echo "❌ TCP port 43300 failed"
fi

echo "Testing HTTP metrics..."
if curl -s http://localhost:9090/metrics | grep -q "nyx_connections"; then
    echo "✅ HTTP metrics OK"
else
    echo "❌ HTTP metrics failed"
fi

echo "Mock daemon test complete"
