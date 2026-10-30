"""Exercise the actual native validator CDP transport against bounded loopback endpoints."""
import ast
import base64
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import threading
import time
from urllib.parse import urlsplit
from urllib.request import ProxyHandler, build_opener


CASES = (
    'handshake_eof', 'handshake_deadline', 'handshake_header_cap', 'handshake_rejected',
    'event_deadline', 'ping_deadline', 'partial_frame_deadline', 'screenshot_deadline',
    'cdp_error', 'javascript_exception', 'success', 'connect_retries_eof', 'connect_deadline',
)
PNG = base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a/2kAAAAASUVORK5CYII=')


def load_transport(source):
    tree = ast.parse(source.read_text(encoding='utf-8-sig'))
    definitions = [node for node in tree.body if isinstance(node, (ast.ClassDef, ast.FunctionDef)) and node.name in ('CDP', 'connect')]
    assert len(definitions) == 2, 'The real native validator must provide CDP and connect'
    scope = {'base64': base64, 'json': json, 'os': os, 'socket': socket, 'struct': struct, 'time': time,
             'urlsplit': urlsplit, 'LOCAL_HTTP': build_opener(ProxyHandler({}))}
    exec(compile(ast.Module(body=definitions, type_ignores=[]), str(source), 'exec'), scope)
    return scope


def read_exact(connection, size):
    data = b''
    while len(data) < size:
        part = connection.recv(size - len(data))
        if not part:
            raise EOFError('The fixture client closed')
        data += part
    return data


def read_request(connection):
    while True:
        head = read_exact(connection, 2)
        opcode, length = head[0] & 15, head[1] & 127
        if length == 126:
            length = struct.unpack('!H', read_exact(connection, 2))[0]
        elif length == 127:
            length = struct.unpack('!Q', read_exact(connection, 8))[0]
        assert length <= 100_000, 'The fixture request exceeded its bound'
        mask = read_exact(connection, 4) if head[1] & 128 else None
        data = read_exact(connection, length)
        if mask:
            data = bytes(value ^ mask[index % 4] for index, value in enumerate(data))
        if opcode == 1:
            return json.loads(data)
        if opcode == 8:
            raise EOFError('The fixture client sent a close frame')


def send_frame(connection, value, opcode=1):
    data = json.dumps(value).encode() if opcode == 1 else value
    length = len(data)
    header = bytes([0x80 | opcode, length]) if length < 126 else bytes([0x80 | opcode, 126]) + struct.pack('!H', length)
    connection.sendall(header + data)


class Endpoint(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def log_message(self, *_):
        pass

    def handle(self):
        try:
            super().handle()
        except (OSError, EOFError):
            pass

    def do_GET(self):
        if self.path == '/json/list':
            body = json.dumps([{'type': 'page', 'url': 'http://tauri.localhost',
                                'webSocketDebuggerUrl': f'ws://127.0.0.1:{self.server.server_port}/transport'}]).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.send_header('Connection', 'close')
            self.end_headers()
            self.wfile.write(body)
            self.close_connection = True
            return
        self.close_connection = True
        self.connection.settimeout(2)
        self.server.handshakes += 1
        case = self.server.case
        if case == 'handshake_eof' or (case == 'connect_retries_eof' and self.server.handshakes == 1):
            self.connection.shutdown(socket.SHUT_RDWR)
            return
        if case == 'handshake_rejected':
            self.connection.sendall(b'HTTP/1.1 403 Forbidden\r\nAuthorization: fixture-private-value\r\nSet-Cookie: fixture-private-value\r\nContent-Length: 0\r\n\r\n')
            return
        if case == 'handshake_deadline':
            self.connection.sendall(b'HTTP/1.1 101 Switching Protocols\r\nX-Fixture: ')
            while not self.server.stopping.wait(.01):
                self.connection.sendall(b'x')
            return
        accept = base64.b64encode(hashlib.sha1((self.headers['Sec-WebSocket-Key'] + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11').encode()).digest()).decode()
        self.send_response(101)
        self.send_header('Upgrade', 'websocket')
        self.send_header('Connection', 'Upgrade')
        self.send_header('Sec-WebSocket-Accept', accept)
        if case == 'handshake_header_cap':
            self.send_header('X-Fixture', 'x' * 17_000)
        self.end_headers()
        if case == 'handshake_header_cap':
            return
        while not self.server.stopping.is_set():
            request = read_request(self.connection)
            self.server.requests += 1
            if case in ('event_deadline', 'screenshot_deadline', 'connect_deadline', 'ping_deadline'):
                while not self.server.stopping.is_set():
                    if case == 'ping_deadline':
                        send_frame(self.connection, b'fixture', opcode=9)
                    else:
                        send_frame(self.connection, {'method': 'Network.dataReceived', 'params': {'requestId': 'fixture'}})
                    self.server.events += 1
                    self.server.stopping.wait(.01)
                return
            if case == 'partial_frame_deadline':
                self.connection.sendall(bytes([0x81, 100]) + b'{')
                while not self.server.stopping.wait(.01):
                    self.connection.sendall(b' ')
                return
            if case == 'cdp_error':
                result = {'id': request['id'], 'error': {'code': -32000, 'message': 'fixture-CDP-error'}}
            elif case == 'javascript_exception':
                result = {'id': request['id'], 'result': {'result': {'type': 'undefined'}, 'exceptionDetails': {'text': 'fixture-JS-error'}}}
            elif request['method'] == 'Page.captureScreenshot':
                result = {'id': request['id'], 'result': {'data': base64.b64encode(PNG).decode()}}
            else:
                value = '10002' if case == 'connect_retries_eof' else 'fixture-ok'
                result = {'id': request['id'], 'result': {'result': {'value': value}}}
            send_frame(self.connection, result)


class ImageSink:
    data = None

    def write_bytes(self, data):
        self.data = data


def run_case(source, case):
    scope = load_transport(source)
    server = ThreadingHTTPServer(('127.0.0.1', 0), Endpoint)
    server.daemon_threads = True
    server.case = case
    server.stopping = threading.Event()
    server.handshakes = server.requests = server.events = 0
    thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=.02), daemon=True)
    thread.start()
    client = None
    started = time.monotonic()
    try:
        endpoint = f'ws://127.0.0.1:{server.server_port}/transport'
        if case == 'connect_retries_eof':
            scope['debug_port'] = server.server_port
            client = scope['connect']('10002', timeout=3)
            assert server.handshakes >= 2, 'Discovery did not retry the transient handshake EOF'
        elif case == 'connect_deadline':
            scope['debug_port'] = server.server_port
            try:
                scope['connect']('10002', timeout=.3)
            except AssertionError as error:
                assert 'not ready' in str(error) and 'deadline exceeded' in str(error), str(error)
            else:
                raise AssertionError('Discovery accepted a connection that never returned its account')
            assert server.events >= 2
        elif case.startswith('handshake_'):
            expected = EOFError if case == 'handshake_eof' else TimeoutError if case == 'handshake_deadline' else AssertionError
            try:
                client = scope['CDP'](endpoint, timeout=.3 if case == 'handshake_deadline' else 1)
            except expected as error:
                message = str(error)
                assert 'CDP handshake' in message, message
                assert 'fixture-private-value' not in message, 'Handshake diagnostics exposed a private header'
                if case == 'handshake_rejected':
                    assert 'httpStatus=403' in message
                if case == 'handshake_header_cap':
                    assert '16384' in message
                if case == 'handshake_eof':
                    assert 'receivedBytes=0' in message
            else:
                raise AssertionError(f'{case} did not reject the broken handshake')
        else:
            client = scope['CDP'](endpoint, timeout=1)
            if case in ('event_deadline', 'ping_deadline', 'partial_frame_deadline', 'screenshot_deadline'):
                try:
                    if case == 'screenshot_deadline':
                        sink = ImageSink()
                        client.screenshot(sink, timeout=.3)
                    else:
                        client.command('Runtime.evaluate', {'expression': '1'}, timeout=.3)
                except TimeoutError as error:
                    method = 'Page.captureScreenshot' if case == 'screenshot_deadline' else 'Runtime.evaluate'
                    assert method in str(error) and 'id=1' in str(error), str(error)
                else:
                    raise AssertionError(f'{case} did not enforce its absolute deadline')
                if case != 'partial_frame_deadline':
                    assert server.events >= 2, 'The endpoint did not produce repeated traffic'
            elif case in ('cdp_error', 'javascript_exception'):
                try:
                    client.evaluate('1', timeout=1)
                except AssertionError as error:
                    expected = 'fixture-CDP-error' if case == 'cdp_error' else 'fixture-JS-error'
                    assert expected in str(error), 'The original CDP/JavaScript error was lost'
                else:
                    raise AssertionError('A real protocol/JavaScript error was accepted')
            elif case == 'success':
                assert client.evaluate('1', timeout=1) == 'fixture-ok'
                sink = ImageSink()
                client.screenshot(sink, timeout=1)
                assert sink.data == PNG, 'Screenshot data was not preserved'
        elapsed = time.monotonic() - started
        assert elapsed < (4 if case == 'connect_retries_eof' else 2), f'{case} exceeded its bounded operation time: {elapsed}'
        return {'case': case, 'passed': True, 'seconds': round(elapsed, 3)}
    finally:
        if client:
            client.close()
        server.stopping.set()
        server.shutdown()
        server.server_close()
        thread.join(timeout=1)


def main():
    if len(sys.argv) >= 4 and sys.argv[1] == '--case':
        case, source = sys.argv[2], Path(sys.argv[3]).resolve(strict=True)
        assert case in CASES
        print(json.dumps(run_case(source, case)), flush=True)
        return
    source = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).with_name('native_profiles.py')
    source = source.resolve(strict=True)
    for case in CASES:
        # A regression must fail this owned child instead of hanging the CI driver itself.
        result = subprocess.run([sys.executable, '-u', str(Path(__file__).resolve()), '--case', case, str(source)],
                                capture_output=True, text=True, encoding='utf-8', timeout=6)
        assert result.returncode == 0, f'{case} failed:\n{result.stdout}\n{result.stderr}'
        print(result.stdout.strip(), flush=True)
    print(json.dumps({'passed': len(CASES), 'realLoopbackEndpoints': True, 'realQQTouched': False}), flush=True)


if __name__ == '__main__':
    main()
