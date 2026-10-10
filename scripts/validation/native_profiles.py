"""Exercise real Tauri profile isolation with simulated protocol endpoints only."""
import base64
import hashlib
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.request import ProxyHandler, build_opener, getproxies
from urllib.parse import urlsplit

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.stderr.reconfigure(encoding="utf-8", errors="replace")

PROJECT = Path(os.environ.get("EAZYQQ_TEST_PROJECT_ROOT", Path(__file__).resolve().parents[2]))
GUI = Path(os.environ.get("EAZYQQ_TEST_GUI", PROJECT / "src-tauri/target/release/eazyqq.exe"))
CLI = Path(os.environ.get("EAZYQQ_TEST_CLI", PROJECT / "src-tauri/target/release/eazyqq_cli.exe"))
FIXTURES = PROJECT / ".test-runtime"
FIXTURES.mkdir(exist_ok=True)
ROOT = Path(tempfile.mkdtemp(prefix="native-profiles-", dir=FIXTURES))
ENV = {**os.environ, "EAZYQQ_ROOT": str(ROOT)}
servers = []
LOCAL_HTTP = build_opener(ProxyHandler({}))
delayed_probe_account = None
delayed_probe_started = threading.Event()
delayed_probe_release = threading.Event()


class Protocol(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def log_message(self, *_): pass
    def handle(self):
        try: super().handle()
        except (ConnectionResetError, ConnectionAbortedError, BrokenPipeError): pass
    def respond(self, body):
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        if self.headers.get("Upgrade", "").lower() == "websocket":
            key = self.headers["Sec-WebSocket-Key"]
            accept = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            self.send_response(101)
            self.send_header("Upgrade", "websocket")
            self.send_header("Connection", "Upgrade")
            self.send_header("Sec-WebSocket-Accept", accept)
            self.end_headers()
            self.connection.settimeout(1)
            while not self.server.stopping.is_set():
                try:
                    if not self.connection.recv(1024): break
                except socket.timeout: pass
            self.close_connection = True
        else: self.respond({"code": 0, "data": {}})
    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length", 0)))
        if self.path == "/get_login_info":
            if self.server.uin == delayed_probe_account:
                delayed_probe_started.set()
                delayed_probe_release.wait(timeout=3)
            body = {"status": "ok", "retcode": 0, "data": {"user_id": self.server.uin, "nickname": "Native fixture"}}
        elif self.path == "/api/auth/login": body = {"code": 0, "data": {"Credential": "fixture"}}
        elif self.path == "/api/QQLogin/CheckLoginStatus": body = {"code": 0, "data": {"isLogin": True, "uin": self.server.uin}}
        elif self.path == "/get_version_info": body = {"status": "ok", "retcode": 0, "data": {"app_version": "7.8.9"}}
        elif self.path.startswith("/api/"): body = {"code": 0, "data": []}
        else: body = {"status": "ok", "retcode": 0, "data": []}
        self.respond(body)


def run(*args):
    result = subprocess.run([str(CLI), *args, "--json"], env=ENV, capture_output=True, text=True, encoding="utf-8", timeout=30)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


class CDP:
    def __init__(self, endpoint):
        url = urlsplit(endpoint)
        self.socket = socket.create_connection((url.hostname, url.port), timeout=10)
        self.socket.settimeout(10)
        key = base64.b64encode(os.urandom(16)).decode()
        request = f"GET {url.path} HTTP/1.1\r\nHost: {url.hostname}:{url.port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        self.socket.sendall(request.encode())
        response = b""
        while not response.endswith(b"\r\n\r\n"): response += self.socket.recv(1)
        assert b"101" in response.split(b"\r\n")[0], response
        self.sequence = 0
    def close(self): self.socket.close()
    def read(self, size):
        result = b""
        while len(result) < size:
            part = self.socket.recv(size - len(result))
            if not part: raise EOFError("CDP connection closed")
            result += part
        return result
    def frame(self, opcode, data):
        mask = os.urandom(4)
        length = len(data)
        header = bytes([0x80 | opcode, 0x80 | length]) if length < 126 else bytes([0x80 | opcode, 0x80 | 126]) + struct.pack("!H", length)
        self.socket.sendall(header + mask + bytes(value ^ mask[index % 4] for index, value in enumerate(data)))
    def screenshot(self, path):
        self.sequence += 1
        self.frame(1,json.dumps({"id":self.sequence,"method":"Page.captureScreenshot","params":{"format":"png"}}).encode())
        while True:
            head=self.read(2); opcode,length=head[0]&15,head[1]&127
            if length==126: length=struct.unpack("!H",self.read(2))[0]
            elif length==127: length=struct.unpack("!Q",self.read(8))[0]
            mask=self.read(4) if head[1]&128 else None
            data=self.read(length)
            if mask: data=bytes(value^mask[index%4] for index,value in enumerate(data))
            if opcode==9: self.frame(10,data); continue
            if opcode==8: raise EOFError("CDP closed during screenshot")
            if opcode!=1: continue
            response=json.loads(data)
            if response.get("id")==self.sequence:
                assert "error" not in response,response
                path.write_bytes(base64.b64decode(response["result"]["data"]))
                return
    def evaluate(self, expression):
        self.sequence += 1
        payload = {"id": self.sequence, "method": "Runtime.evaluate", "params": {"expression": expression, "awaitPromise": True, "returnByValue": True}}
        self.frame(1, json.dumps(payload).encode())
        while True:
            head = self.read(2)
            opcode, length = head[0] & 15, head[1] & 127
            if length == 126: length = struct.unpack("!H", self.read(2))[0]
            elif length == 127: length = struct.unpack("!Q", self.read(8))[0]
            mask = self.read(4) if head[1] & 128 else None
            data = self.read(length)
            if mask: data = bytes(value ^ mask[index % 4] for index, value in enumerate(data))
            if opcode == 8: raise EOFError("CDP closed")
            if opcode == 9: self.frame(10, data); continue
            if opcode != 1: continue
            response = json.loads(data)
            if response.get("id") == self.sequence:
                assert "error" not in response, response
                assert "exceptionDetails" not in response["result"], response
                return response["result"]["result"].get("value")


def connect(uin, timeout=90):
    deadline = time.monotonic() + timeout
    last_error = None
    while time.monotonic() < deadline:
        client = None
        try:
            with LOCAL_HTTP.open(f"http://127.0.0.1:{debug_port}/json/list", timeout=2) as response: pages = json.load(response)
            page = next(item for item in pages if item.get("type") == "page" and "tauri.localhost" in item.get("url", ""))
            client = CDP(page["webSocketDebuggerUrl"])
            value = client.evaluate("(async () => { if (!window.__TAURI_INTERNALS__) return null; const r = await window.__TAURI_INTERNALS__.invoke('get_protocol_status'); return r.data?.qqNumber; })()")
            if value == uin: return client
        except (OSError, EOFError, StopIteration, AssertionError) as error: last_error = str(error)
        if client: client.close()
        time.sleep(0.25)
    raise AssertionError(f"Native frontend for {uin} was not ready: {last_error}")


gui = client = None
registry_override = None
try:
    assert os.name == "nt", "Native profile validation requires Windows"
    for uin in ["10001", "10002"]: run("accounts", "add", "--uin", uin)
    registry_path = next((ROOT / "EazyQQ_Data/accounts").glob("*/instances.json"))
    registry = json.loads(registry_path.read_text(encoding="utf-8-sig"))
    for instance in registry["instances"]:
        for key in ["http_port", "ws_port", "webui_port"]:
            server = ThreadingHTTPServer(("127.0.0.1", 0), Protocol)
            server.daemon_threads = True
            server.uin = instance["uin"]
            server.stopping = threading.Event()
            instance[key] = server.server_address[1]
            servers.append(server)
            threading.Thread(target=server.serve_forever, daemon=True).start()
        directory = registry_path.parent / instance["uin"] / "protocol/config"
        directory.mkdir(parents=True)
        (directory / "webui.json").write_text(json.dumps({"token": "fixture", "port": instance["webui_port"]}))
    registry_path.write_text(json.dumps(registry))
    napcat = ROOT / "napcat"
    napcat.mkdir()
    (napcat / "napcat.mjs").write_text("// Simulated protocol, no QQ launcher is installed here.\n")
    bootstrap = ROOT / "EazyQQ_Data/bootstrap.json"
    hosted = os.environ.get("GITHUB_ACTIONS") == "true"
    bootstrap.write_text(json.dumps({"lastAccount": "10001", "webviewCompatMode": not hosted}))
    opencode_fixture = ROOT / "opencode-fixture.exe"
    subprocess.run(["pwsh","-NoProfile","-File",str(PROJECT / "scripts/validation/build-opencode-fixture.ps1"),"-Target",str(opencode_fixture)],check=True,timeout=60)
    ENV["EAZYQQ_OPENCODE_BIN"] = str(opencode_fixture)
    config = ROOT / "fixture-config.json"
    config.write_text(json.dumps({"ai": {"activeProvider": "opencode", "model": "big-pickle"}, "summary": {"enabled": False}, "napcat": {"autoRestart": False, "heartbeatIntervalSec": 15}, "storage": {"autoSyncFiles": False}, "window": {"closeToTray": False}}))
    for uin in ["10001", "10002"]: run("set-config", "--account", uin, "--key", "app_config", "--file", str(config))
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        debug_port = reservation.getsockname()[1]
    ENV["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = f"--remote-debugging-port={debug_port} --disable-gpu"
    if hosted:
        # Hosted WebView2 may ignore inherited flags; use its documented app override.
        import winreg
        key_path = r"Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments"
        arguments = ENV.pop("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS") + " --no-sandbox"
        previous = []
        with winreg.CreateKey(winreg.HKEY_LOCAL_MACHINE, key_path) as key:
            for app_id in [GUI.name, "com.eazyqq.app"]:
                try: value = winreg.QueryValueEx(key, app_id)
                except FileNotFoundError: value = None
                previous.append((app_id, value))
                winreg.SetValueEx(key, app_id, 0, winreg.REG_SZ, arguments)
        registry_override = (key_path, previous)
    with (ROOT / "gui-stderr.log").open("wb") as stderr:
        gui = subprocess.Popen([str(GUI)], env=ENV, cwd=ROOT, creationflags=subprocess.CREATE_NO_WINDOW, stdout=subprocess.DEVNULL, stderr=stderr)
    client = connect("10001")
    expected_version = json.loads((PROJECT / "package.json").read_text(encoding="utf-8-sig"))["version"]
    assert client.evaluate("window.__TAURI_INTERNALS__.invoke('plugin:app|version')") == expected_version
    assert client.evaluate("(async () => { window.dispatchEvent(new KeyboardEvent('keydown', {key:'F1'})); await new Promise(resolve => setTimeout(resolve, 50)); return !!document.querySelector('button[aria-label=\"关闭使用说明书\"]'); })()") is True
    assert client.evaluate("(async () => { window.dispatchEvent(new KeyboardEvent('keydown', {key:'F1'})); await new Promise(resolve => setTimeout(resolve, 240)); return !document.querySelector('button[aria-label=\"关闭使用说明书\"]'); })()") is True
    assert client.evaluate("window.__TAURI_INTERNALS__.invoke('app_toggle_maximize_window')") is True
    assert client.evaluate("window.__TAURI_INTERNALS__.invoke('app_toggle_maximize_window')") is False
    # Validate discovery and selected-model preservation through the rendered settings.
    client.evaluate("[...document.querySelectorAll('button')].find(button=>button.textContent.includes('系统设置')).click(); true")
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if client.evaluate("!!document.querySelector('section[aria-label=\"OpenCode 免费模型检测\"]') && [...document.querySelectorAll('button')].some(button=>button.textContent==='自动检测免费模型' && !button.disabled)"):
            break
        time.sleep(.1)
    else: raise AssertionError("Free model discovery did not render its ready state")
    original_config = run("ai-config", "--account", "10001")
    client.evaluate("[...document.querySelectorAll('button')].find(button=>button.textContent==='自动检测免费模型').click(); true")
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        if client.evaluate("document.querySelector('section[aria-label=\"OpenCode 免费模型检测\"]')?.textContent.includes('免凭据可用') && document.querySelector('section[aria-label=\"OpenCode 免费模型检测\"]')?.textContent.includes('明确停用')"):
            break
        time.sleep(.1)
    else: raise AssertionError("Free model detection did not distinguish verified and retired models")
    assert run("ai-config", "--account", "10001")["model"] == original_config["model"], "Detection silently changed the model"
    client.evaluate("document.querySelector('section[aria-label=\"OpenCode 免费模型检测\"]').scrollIntoView({block:'center'}); true")
    client.screenshot(ROOT / "free-model-discovery.png")
    assert client.evaluate("localStorage.getItem('eazyqq_profile_validation')") is None
    client.evaluate("localStorage.setItem('eazyqq_profile_validation', '10001'); true")
    assert (registry_path.parent / "10001/webview/main/EBWebView").exists(), "The primary account used a shared browser profile"
    client.evaluate("window.__TAURI_INTERNALS__.invoke('quick_login', {uin:'10002'}); true")
    client.close()
    client = connect("10002")
    assert json.loads(bootstrap.read_text())["lastAccount"] == "10002"
    assert client.evaluate("localStorage.getItem('eazyqq_profile_validation')") is None, "The second account inherited primary localStorage"
    client.evaluate("localStorage.setItem('eazyqq_profile_validation', '10002'); true")
    assert (registry_path.parent / "10002/webview/main/EBWebView").exists(), "The second account used a shared browser profile"
    # Release old bound identity responses only after the new selection is committed.
    delayed_probe_account = "10002"
    client.evaluate("for(let i=0;i<8;i++) window.__TAURI_INTERNALS__.invoke('get_protocol_status').catch(()=>{}); true")
    assert delayed_probe_started.wait(timeout=3), "Old context did not start its status probes"
    client.evaluate("window.__TAURI_INTERNALS__.invoke('quick_login', {uin:'10001'}); true")
    deadline = time.monotonic() + 3
    while json.loads(bootstrap.read_text())["lastAccount"] != "10001" and time.monotonic() < deadline:
        time.sleep(0.005)
    assert json.loads(bootstrap.read_text())["lastAccount"] == "10001", "Explicit selection was not committed"
    delayed_probe_release.set()
    client.close()
    client = connect("10001")
    assert client.evaluate("localStorage.getItem('eazyqq_profile_validation')") == "10001"
    client.evaluate("window.__TAURI_INTERNALS__.invoke('app_close_window'); true")
    client.close()
    client = None
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        try:
            with LOCAL_HTTP.open(f"http://127.0.0.1:{debug_port}/json/list", timeout=1): pass
        except OSError: break
        time.sleep(0.1)
    else: raise AssertionError("The test desktop did not shut down")
    assert json.loads(bootstrap.read_text())["lastAccount"] == "10001", "An old status response reversed selection"
    print(json.dumps({"passed": True, "freeModelDiscovery": True, "accountRoundTrip": True, "delayedStatusSelectionPreserved": True, "privateProfiles": True, "maximizeRoundTrip": True, "manualShortcut": True, "realQQTouched": False, "fixture": str(ROOT)}))
except Exception:
    print(json.dumps({"fixture": str(ROOT), "debugPort": globals().get("debug_port"), "guiExitCode": gui.poll() if gui else None, "systemProxyConfigured": bool(getproxies())}), flush=True)
    browser_state = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", "Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" | ForEach-Object { [pscustomobject]@{pid=$_.ProcessId;parent=$_.ParentProcessId;debug=[regex]::Match($_.CommandLine,'--remote-debugging-port=\\d+').Value} } | ConvertTo-Json -Compress"], capture_output=True, text=True, errors="replace", timeout=15)
    print(f"Browser processes: {browser_state.stdout.strip()}", flush=True)
    command_lines = subprocess.run(["powershell.exe", "-NoProfile", "-NonInteractive", "-Command", "Get-CimInstance Win32_Process -Filter \"Name='msedgewebview2.exe'\" | Where-Object { $_.CommandLine -like '*native-profiles-*' } | Select-Object ProcessId,ParentProcessId,CommandLine | ConvertTo-Json -Compress"], capture_output=True, text=True, errors="replace", timeout=15)
    print(f"Isolated browser arguments: {command_lines.stdout.strip()}", flush=True)
    network = subprocess.run(["netstat", "-ano"], capture_output=True, text=True, errors="replace", timeout=15)
    print("\n".join(line for line in network.stdout.splitlines() if f":{globals().get('debug_port')} " in line), flush=True)
    for log in [ROOT / "gui-stderr.log", *ROOT.glob("EazyQQ_Data/accounts/*/*/logs/*.log")]:
        if log.exists():
            print(f"Diagnostic log: {log.relative_to(ROOT)}", flush=True)
            print("\n".join(log.read_text(encoding="utf-8", errors="replace").splitlines()[-40:]), flush=True)
    raise
finally:
    delayed_probe_release.set()
    if client:
        try: client.evaluate("window.__TAURI_INTERNALS__.invoke('app_close_window'); true")
        except (OSError, EOFError, AssertionError): pass
        client.close()
    for server in servers:
        server.stopping.set()
        server.shutdown()
        server.server_close()
    if gui and gui.poll() is None:
        gui.terminate()
        gui.wait(timeout=5)
    if registry_override:
        import winreg
        key_path, previous = registry_override
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, key_path, 0, winreg.KEY_SET_VALUE) as key:
            for app_id, value in previous:
                if value is None: winreg.DeleteValue(key, app_id)
                else: winreg.SetValueEx(key, app_id, 0, value[1], value[0])
