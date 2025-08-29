#!/usr/bin/env python3
"""
Test mock daemon for nyx-daemon TCP compatibility
"""
import socket
import json
import threading
import time
from http.server import HTTPServer, BaseHTTPRequestHandler

class MetricsHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/metrics':
            self.send_response(200)
            self.send_header('Content-type', 'text/plain')
            self.end_headers()
            metrics = """# HELP nyx_connections_total Total connections
# TYPE nyx_connections_total counter
nyx_connections_total 42
# HELP nyx_bandwidth_bytes_total Total bandwidth
# TYPE nyx_bandwidth_bytes_total counter
nyx_bandwidth_bytes_total 1048576
# HELP nyx_latency_seconds Connection latency
# TYPE nyx_latency_seconds histogram
nyx_latency_seconds_bucket{le="0.1"} 10
nyx_latency_seconds_bucket{le="0.5"} 25
nyx_latency_seconds_bucket{le="1.0"} 40
nyx_latency_seconds_bucket{le="+Inf"} 42
nyx_latency_seconds_sum 15.2
nyx_latency_seconds_count 42
"""
            self.wfile.write(metrics.encode())
        else:
            self.send_response(404)
            self.end_headers()
    
    def log_message(self, format, *args):
        pass  # Suppress HTTP log messages

def tcp_server():
    """Mock TCP server on port 43300"""
    server_socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server_socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server_socket.bind(('0.0.0.0', 43300))
    server_socket.listen(5)
    print("Mock TCP daemon listening on port 43300...")
    
    while True:
        try:
            client, addr = server_socket.accept()
            print(f"TCP connection from {addr}")
            
            # Send mock response
            response = json.dumps({
                "status": "ok", 
                "daemon": "nyx-mock", 
                "version": "1.0.0",
                "ready": True,
                "connections": 42,
                "bandwidth": "1MB/s"
            })
            client.send(f"HTTP/1.1 200 OK\r\nContent-Length: {len(response)}\r\n\r\n{response}".encode())
            client.close()
        except Exception as e:
            print(f"TCP server error: {e}")

def http_server():
    """HTTP metrics server on port 9090"""
    httpd = HTTPServer(('0.0.0.0', 9090), MetricsHandler)
    print("Mock HTTP metrics server listening on port 9090...")
    httpd.serve_forever()

if __name__ == "__main__":
    print("Starting Nyx Mock Daemon...")
    print("TCP port 43300 for daemon communication")
    print("HTTP port 9090 for Prometheus metrics")
    
    # Start both servers in separate threads
    tcp_thread = threading.Thread(target=tcp_server, daemon=True)
    http_thread = threading.Thread(target=http_server, daemon=True)
    
    tcp_thread.start()
    http_thread.start()
    
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nShutting down mock daemon...")
