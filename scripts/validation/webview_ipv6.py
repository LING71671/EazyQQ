"""Exercise CDP discovery against a real IPv6-only HTTP/WebSocket endpoint."""
import ast
import base64
import hashlib
import json
from pathlib import Path
import socket
import struct
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import ProxyHandler, build_opener
from urllib.parse import urlsplit

source = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).with_name('native_profiles.py')
tree = ast.parse(source.read_text(encoding='utf-8-sig'))
definitions = [item for item in tree.body if isinstance(item, (ast.FunctionDef, ast.ClassDef)) and item.name in ['CDP', 'connect']]
assert len(definitions) == 2
scope = {'base64': base64, 'json': json, 'socket': socket, 'struct': struct, 'urlsplit': urlsplit, 'time': time, 'os': __import__('os'), 'LOCAL_HTTP': build_opener(ProxyHandler({}))}
exec(compile(ast.Module(body=definitions, type_ignores=[]), str(source), 'exec'), scope)


class IPv6Server(ThreadingHTTPServer):
    address_family = socket.AF_INET6
    daemon_threads = True


class Endpoint(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *_): pass
    def do_GET(self):
        if self.path == '/json/list':
            data = json.dumps([{'type': 'page', 'url': 'http://tauri.localhost', 'webSocketDebuggerUrl': f'ws://[::1]:{self.server.server_port}/devtools/page/test'}]).encode()
            self.send_response(200)
            self.send_header('Content-Length', str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        self.server.websocket_host = self.headers['Host']
        accept = base64.b64encode(hashlib.sha1((self.headers['Sec-WebSocket-Key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
        self.send_response(101)
        self.send_header('Upgrade', 'websocket')
        self.send_header('Connection', 'Upgrade')
        self.send_header('Sec-WebSocket-Accept', accept)
        self.end_headers()
        head = self.rfile.read(2)
        length = head[1] & 127
        if length == 126: length = struct.unpack('!H', self.rfile.read(2))[0]
        mask = self.rfile.read(4)
        raw = self.rfile.read(length)
        message = json.loads(bytes(value ^ mask[index % 4] for index, value in enumerate(raw)))
        assert message['method'] == 'Runtime.evaluate'
        data = json.dumps({'id': message['id'], 'result': {'result': {'value': '10002'}}}).encode()
        self.wfile.write(bytes([0x81, len(data)]) + data)
        self.wfile.flush()
        self.close_connection = True


with IPv6Server(('::1', 0), Endpoint) as server:
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    scope['debug_port'] = server.server_port
    try:
        client = scope['connect']('10002', timeout=10)
        client.close()
        assert server.websocket_host == f'[::1]:{server.server_port}'
        print(json.dumps({'passed': True, 'ipv6OnlyDiscovery': True, 'websocketAuthorityPreserved': True}))
    finally:
        server.shutdown()
        thread.join(timeout=2)
