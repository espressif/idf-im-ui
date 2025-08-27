#!/bin/bash

# Setup dummy HTTP server on a non-standard port to catch network attempts
python3 -c "
import http.server
import socketserver
import threading
import time

class NetworkBlockHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        print(f'NETWORK VIOLATION: GET request to {self.path}')
        self.send_response(503)
        self.end_headers()
        self.wfile.write(b'Network access blocked for offline test')

    def do_POST(self):
        print(f'NETWORK VIOLATION: POST request to {self.path}')
        self.send_response(503)
        self.end_headers()
        self.wfile.write(b'Network access blocked for offline test')

    def log_message(self, format, *args):
        pass  # Suppress default logging

def start_server():
    with socketserver.TCPServer(('127.0.0.1', 8888), NetworkBlockHandler) as httpd:
        httpd.serve_forever()

server_thread = threading.Thread(target=start_server, daemon=True)
server_thread.start()
print('Network monitoring server started on port 8888')
time.sleep(1)  # Give server time to start
" &

# Set proxy environment variables to route traffic through our blocking server
export HTTP_PROXY=http://127.0.0.1:8888
export HTTPS_PROXY=http://127.0.0.1:8888
export http_proxy=http://127.0.0.1:8888
export https_proxy=http://127.0.0.1:8888
export FTP_PROXY=http://127.0.0.1:8888
export ftp_proxy=http://127.0.0.1:8888
export NO_PROXY=""
export no_proxy=""

# Execute the command passed as arguments
exec "$@"
