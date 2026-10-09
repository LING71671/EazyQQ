"""Verify the real QQ loader only on an ephemeral hosted Windows runner."""
import ctypes
import ctypes.wintypes as wintypes
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time

if os.environ.get("GITHUB_ACTIONS") != "true":
    raise SystemExit("Real QQ QA runs only on hosted CI; do not launch it on the user's desktop")
PROJECT = Path(__file__).resolve().parents[2]
CLI = PROJECT / "src-tauri/target/release/eazyqq_cli.exe"
QQ = Path(os.environ["EAZYQQ_QA_QQ"])
ROOT = Path(tempfile.mkdtemp(prefix="qq-runtime-", dir=PROJECT / ".test-runtime"))
shutil.copytree(PROJECT / "src-tauri/resources/napcat", ROOT / "napcat")
(ROOT / "napcat/config").mkdir(exist_ok=True)
(ROOT / "napcat/config/qq_path.txt").write_text(str(QQ), encoding="utf-8")
ENV = dict(os.environ, EAZYQQ_ROOT=str(ROOT))

def call(*args):
    result = subprocess.run([str(CLI), *args, "--json"], env=ENV, cwd=PROJECT,
        capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=35)
    if result.returncode:
        raise RuntimeError(f"{args[0]} failed: {result.stderr[-800:]}")
    return json.loads(result.stdout)

def owned_pids(private):
    receipt = json.loads((private / "eazyqq-process.json").read_text())
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenJobObjectW.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.LPCWSTR]
    kernel.OpenJobObjectW.restype = wintypes.HANDLE
    kernel.QueryInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p]
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    handle = kernel.OpenJobObjectW(4, False, receipt["job_name"])
    if not handle: return []
    try:
        buffer = ctypes.create_string_buffer(65536)
        if not kernel.QueryInformationJobObject(handle, 3, buffer, len(buffer), None): return []
        count = ctypes.c_uint32.from_buffer(buffer, 4).value
        return list((ctypes.c_size_t * count).from_buffer(buffer, 8))
    finally:
        kernel.CloseHandle(handle)

report = {"fixture": str(ROOT), "qqVersion": "9.9.23-42430", "loggedIn": False}
try:
    call("accounts", "add", "--uin", "10001")
    call("accounts", "start", "--uin", "10001")
    accounts = call("accounts", "list")
    port = accounts[0]["instance"]["webuiPort"]
    private = next((ROOT / "EazyQQ_Data/accounts").glob("*/10001/protocol"))
    patch = json.loads((private / "qqnt.json").read_text())
    assert patch["main"].endswith(".cjs"), "Explicit CommonJS loader is required"
    report["entry"] = patch["main"]
    deadline = time.monotonic() + 80
    while time.monotonic() < deadline:
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=1): break
        except OSError: time.sleep(2)
    else: raise RuntimeError("Real QQ WebUI did not start")
    netstat = subprocess.run(["netstat", "-ano"], capture_output=True, text=True).stdout.splitlines()
    owners = [int(line.split()[-1]) for line in netstat if len(line.split()) >= 5
        and line.split()[0] == "TCP" and line.split()[1].endswith(f":{port}") and line.split()[3] == "LISTENING"]
    assert any(pid in owned_pids(private) for pid in owners), "WebUI must belong to this test's process tree"
    qr_deadline = time.monotonic() + 45
    while True:
        try:
            qr = call("--account", "10001", "qr")
            break
        except RuntimeError as error:
            if "生成二维码" not in str(error) or time.monotonic() >= qr_deadline: raise
            time.sleep(2)
    report["qrAvailable"] = any(bool(qr.get(key)) for key in ("qrcodeBase64", "qrcode", "qrCode", "qr"))
    report["webuiOwned"] = True
    report["passed"] = True
finally:
    try:
        report["stop"] = call("accounts", "stop", "--uin", "10001")
    finally:
        (ROOT / "verification.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(json.dumps(report))
