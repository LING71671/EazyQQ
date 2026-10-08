"""Validate CLI routing and process ownership using isolated data and fake protocol servers."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import base64
import hashlib
import struct
import sqlite3
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PROJECT = Path(__file__).resolve().parents[2]
CLI = Path(os.environ.get("EAZYQQ_TEST_CLI", PROJECT / "src-tauri/target/debug/eazyqq_cli.exe"))
FIXTURES = PROJECT / ".test-runtime"
FIXTURES.mkdir(exist_ok=True)
ROOT = Path(tempfile.mkdtemp(prefix="cli-", dir=FIXTURES))
ENV = {**os.environ, "EAZYQQ_ROOT": str(ROOT)}
for key in ["LLM_API_KEY", "OPENAI_API_KEY", "AI_API_KEY", "OPENCODE_API_KEY", "OPENCODE_GO_API_KEY"]:
    ENV.pop(key, None)
checks = []


def run(*args, success=True):
    result = subprocess.run([str(CLI), *args, "--json"], env=ENV, capture_output=True, text=True, encoding="utf-8", timeout=60)
    if success:
        assert result.returncode == 0, (args, result.stderr)
        value = json.loads(result.stdout)
    else:
        assert result.returncode != 0, (args, result.stdout)
        value = result
    checks.append(" ".join(args))
    return value


class Protocol(BaseHTTPRequestHandler):
    def handle(self):
        try: super().handle()
        except (ConnectionResetError, ConnectionAbortedError, BrokenPipeError): pass

    protocol_version = "HTTP/1.1"
    def log_message(self, *_):
        pass

    def do_GET(self):
        if self.headers.get("Upgrade", "").lower() == "websocket":
            key = self.headers["Sec-WebSocket-Key"]
            accept = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
            self.send_response(101); self.send_header("Upgrade", "websocket"); self.send_header("Connection", "Upgrade"); self.send_header("Sec-WebSocket-Accept", accept); self.end_headers()
            for owner, text, message_id in [(self.server.uin, "account-" + self.server.uin, 99101), ("10002" if self.server.uin == "10001" else "10001", "foreign-account-event", 99102)]:
                packet = json.dumps({"post_type":"message","message_type":"group","self_id":int(owner),"user_id":11001,"group_id":20001,"message_id":message_id,"time":int(time.time()),"raw_message":text,"sender":{"nickname":"Fixture"}}).encode()
                header = bytes([0x81, len(packet)]) if len(packet) < 126 else bytes([0x81,126]) + struct.pack("!H",len(packet))
                self.connection.sendall(header + packet)
            time.sleep(0.5)
            self.close_connection = True
            return
        self.respond({"code": 0, "data": {}})

    def do_POST(self):
        self.rfile.read(int(self.headers.get("Content-Length", 0)))
        if self.path == "/get_login_info":
            body = {"status": "ok", "retcode": 0, "data": {"user_id": self.server.uin, "nickname": "Fixture"}}
        elif self.path == "/get_group_list":
            body = {"status":"ok","retcode":0,"data":[{"group_id":20001,"group_name":"Fixture group"}]}
        elif self.path == "/get_friend_list":
            body = {"status":"ok","retcode":0,"data":[]}
        elif self.path == "/api/auth/login":
            body = {"code": 0, "data": {"Credential": "fixture"}}
        elif self.path == "/api/QQLogin/CheckLoginStatus":
            body = {"code": 0, "data": {"isLogin": False}}
        elif self.path == "/get_version_info":
            body = {"status": "ok", "retcode": 0, "data": {"app_version": "7.8.9"}}
        else:
            body = {"status": "ok", "retcode": 0, "data": []}
        self.respond(body)

    def respond(self, body):
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


servers, supervisors, workers = [], [], []
unrelated = None
try:
    schema = run("--json", "schema")
    assert schema["version"] == json.loads((PROJECT / "package.json").read_text(encoding="utf-8-sig"))["version"]
    assert {"accounts", "run", "repair", "updates", "ai-models"}.issubset({item["name"] for item in schema["commands"]})
    first = run("accounts", "add", "--uin", "10001")
    second = run("accounts", "add", "--uin", "10002")
    ports = [item[key] for item in [first, second] for key in ["httpPort", "wsPort", "webuiPort"]]
    assert len(set(ports)) == 6
    assert run("accounts", "start", "--all", "--dry-run")["uins"] == ["10001", "10002"]
    run("accounts", "configure", "--uin", "10002", "--auto-start", "false")
    run("accounts", "add", "--uin", "../10003", success=False)
    run("status", "--account", "99999", success=False)
    registry_path = next((ROOT / "EazyQQ_Data/accounts").glob("*/instances.json"))
    machine_dir = registry_path.parent
    for instance in [first, second]:
        protocol_dir = machine_dir / instance["uin"] / "protocol"
        (protocol_dir / "config").mkdir(parents=True)
        (protocol_dir / "config/webui.json").write_text(json.dumps({"host": "127.0.0.1", "port": instance["webuiPort"], "token": "fixture"}))
        (protocol_dir / "napcat.mjs").write_text('const version = typeof marker !== "undefined" && "7.8.9" || "1.0.0-dev";')
        for port in [instance["httpPort"], instance["webuiPort"], instance["wsPort"]]:
            server = ThreadingHTTPServer(("127.0.0.1", port), Protocol)
            server.uin = instance["uin"]
            servers.append(server)
            threading.Thread(target=server.serve_forever, daemon=True).start()
    status = run("status", "--account", "10001")
    assert status["loggedIn"] and status["qqNumber"] == "10001"
    status = run("status", "--account", "10002")
    assert status["loggedIn"] and status["qqNumber"] == "10002"
    health = run("health", "--account", "10002")
    assert next(item for item in health["items"] if item["item"] == "QQ login")["detail"].startswith("Authenticated Fixture (10002)")
    assert any(str(second["webuiPort"]) in item["item"] for item in health["items"])
    assert any(str(second["httpPort"]) in item["item"] for item in health["items"])
    chain = run("chain-status", "--account", "10001")
    assert next(link for link in chain["links"] if link["link"] == "qq_login")["health"] == "ok"
    run("accounts", "select", "--uin", "10001")
    assert run("updates", "versions")["napcatVersion"] == "7.8.9"
    run("ai-config", "--account", "10001")
    assert len(run("--json", "accounts", "list")) == 2

    for instance in [first, second]:
        run("rule", "--account", instance["uin"], "--target", "20001", "--mode", "summary_only", "--cooldown-seconds", "0", "--enabled", "true")
        worker = subprocess.Popen([str(CLI), "--account", instance["uin"], "run", "--json"], env=ENV, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", creationflags=getattr(subprocess,"CREATE_NO_WINDOW",0))
        workers.append(worker)
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        complete = True
        for instance in [first, second]:
            database = machine_dir / instance["uin"] / "eazyqq.db"
            if not database.exists(): complete = False; continue
            try:
                with sqlite3.connect(database) as connection:
                    contents = [row[0] for row in connection.execute("SELECT content FROM messages_log")]
            except sqlite3.OperationalError:
                complete = False; continue
            if "account-" + instance["uin"] not in contents: complete = False
            assert "foreign-account-event" not in contents
        if complete: break
        time.sleep(0.1)
    assert complete, "Workers did not route their own account events into separate databases"
    duplicate = subprocess.run([str(CLI), "--account", "10001", "run", "--json"], env=ENV, capture_output=True, text=True, encoding="utf-8", timeout=10)
    assert duplicate.returncode != 0 and "already running" in duplicate.stderr
    checks.append("two WebSocket workers isolate account events and reject duplicate consumers")
    for worker in workers: worker.terminate(); worker.wait(timeout=5)
    requests = [json.dumps({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"validation","version":"1"}}}), json.dumps({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})]
    mcp = subprocess.run([str(CLI),"mcp"], input="\n".join(requests)+"\n", env=ENV,capture_output=True,text=True,encoding="utf-8",timeout=15)
    assert mcp.returncode == 0
    responses = [json.loads(line) for line in mcp.stdout.splitlines()]
    assert len(responses) == 2 and responses[0]["result"]["serverInfo"]["version"] == schema["version"]
    assert len(responses[1]["result"]["tools"]) >= 9
    checks.append("MCP metadata and tools discovery use clean JSON-RPC stdout")

    if os.name == "nt":
        unrelated = subprocess.Popen(["ping", "-n", "30", "127.0.0.1"], stdout=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
        for instance in [first, second]:
            directory = machine_dir / instance["uin"] / "protocol"
            executable = directory / "NapCatWinBootMain.exe"
            shutil.copy2(Path(os.environ["SystemRoot"]) / "System32/cmd.exe", executable)
            request = f"fixture-{instance['uin']}"
            plan = {"request_id": request, "executable": str(executable), "args": ["/d", "/c", "start /b ping -n 30 127.0.0.1 >nul"], "environment": [], "workdir": str(directory), "output_log": str(directory / "fixture.log")}
            (directory / "eazyqq-launch.json").write_text(json.dumps(plan))
            supervisor = subprocess.Popen([str(CLI)], env={**ENV, "EAZYQQ_PROTOCOL_SUPERVISOR": str(directory)}, creationflags=subprocess.CREATE_NO_WINDOW)
            supervisors.append(supervisor)
            response_path = directory / "eazyqq-launch-result.json"
            deadline = time.monotonic() + 10
            while not response_path.exists() and time.monotonic() < deadline:
                time.sleep(0.05)
            assert json.loads(response_path.read_text())["ok"]
        time.sleep(0.5)
        assert all(process.poll() is None for process in supervisors)
        stopped = run("accounts", "stop", "--uin", "10001")
        assert stopped[0]["ok"]
        supervisors[0].wait(timeout=5)
        assert supervisors[1].poll() is None and unrelated.poll() is None
        stopped = run("accounts", "stop", "--uin", "10002")
        assert stopped[0]["ok"]
        supervisors[1].wait(timeout=5)
        assert unrelated.poll() is None
        checks.append("account process trees isolated; unrelated process preserved")
    run("accounts", "forget", "--uin", "10002")
    assert (machine_dir / "10002").exists()
    registry_path.write_text("{")
    run("accounts", "add", "--uin", "10003", success=False)
    assert registry_path.read_text() == "{"
    print(json.dumps({"passed": len(checks), "processOwnership": "verified" if os.name == "nt" else "not applicable", "realQQTouched": False, "fixture": str(ROOT)}))
finally:
    for worker in workers:
        if worker.poll() is None: worker.terminate()
    for server in servers:
        server.shutdown()
    for supervisor in supervisors:
        if supervisor.poll() is None:
            supervisor.terminate()
    if unrelated is not None and unrelated.poll() is None:
        unrelated.terminate()
