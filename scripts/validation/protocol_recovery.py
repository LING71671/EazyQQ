"""Reproduce a live damaged legacy Job using a dummy launcher; never execute QQ."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

PROJECT = Path(__file__).resolve().parents[2]
CLI = Path(os.environ.get("EAZYQQ_TEST_CLI", PROJECT / "src-tauri/target/release/eazyqq_cli.exe"))
ROOT = Path(tempfile.mkdtemp(prefix="protocol-recovery-", dir=PROJECT / ".test-runtime"))
ENV = {**os.environ, "EAZYQQ_ROOT": str(ROOT)}
supervisor = None
unrelated = None

def call(*args, success=True):
    result = subprocess.run([str(CLI), *args, "--json"], env=ENV, capture_output=True, text=True, encoding="utf-8", timeout=40)
    assert (result.returncode == 0) == success, (args, result.stdout, result.stderr)
    return json.loads(result.stdout) if success else result

try:
    call("accounts", "add", "--uin", "10001")
    registry_path = next((ROOT / "EazyQQ_Data/accounts").glob("*/instances.json"))
    registry = json.loads(registry_path.read_text())
    legacy = ROOT / "resources/napcat"
    legacy.mkdir(parents=True)
    registry["instances"][0]["runtime_dir"] = str(legacy)
    registry["instances"][0]["auto_start"] = False
    registry_path.write_text(json.dumps(registry))
    (ROOT / "EazyQQ_Data/bootstrap.json").write_text(json.dumps({"lastAccount":"10001"}))
    executable = legacy / "NapCatWinBootMain.exe"
    shutil.copy2(Path(os.environ["SystemRoot"]) / "System32/cmd.exe", executable)
    plan = {"request_id":"legacy-fixture", "executable":str(executable), "args":["/d","/c","start /b ping -n 60 127.0.0.1 >nul"], "environment":[], "workdir":str(legacy), "output_log":str(legacy / "fixture.log")}
    (legacy / "eazyqq-launch.json").write_text(json.dumps(plan))
    supervisor = subprocess.Popen([str(CLI)], env={**ENV,"EAZYQQ_PROTOCOL_SUPERVISOR":str(legacy)}, creationflags=subprocess.CREATE_NO_WINDOW)
    deadline = time.monotonic() + 10
    while not (legacy / "eazyqq-launch-result.json").exists() and time.monotonic() < deadline: time.sleep(0.05)
    assert json.loads((legacy / "eazyqq-launch-result.json").read_text())["ok"]
    unrelated = subprocess.Popen(["ping","-n","60","127.0.0.1"], stdout=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)

    source = ROOT / "napcat"
    (source / "config").mkdir(parents=True)
    subprocess.run(["pwsh","-NoProfile","-File",str(PROJECT / "scripts/validation/build-launcher.ps1"),"-Target",str(source / "NapCatWinBootMain.exe")], check=True, timeout=60)
    (source / "NapCatWinBootHook.dll").write_bytes(b"fixture is never loaded")
    qq = ROOT / "dummy-qq/QQ.exe"
    qq.parent.mkdir()
    qq.write_bytes(b"fixture is never executed")
    metadata = qq.parent / "resources/app"
    metadata.mkdir(parents=True)
    (metadata / "package.json").write_text(json.dumps({"name":"fixture","version":"9.9.99","main":"original.js"}))
    (source / "config/qq_path.txt").write_text(str(qq))
    (source / "config/webui.json").write_text(json.dumps({"token":"fixture-token","port":6099}))
    call("--account","10001","restart",success=False)
    assert supervisor.poll() is None and unrelated.poll() is None
    (source / "napcat.mjs").write_text("export const fixture = true;")
    result = call("--account","10001","restart")
    assert result["ok"]
    supervisor.wait(timeout=10)
    updated = json.loads(registry_path.read_text())["instances"][0]
    private = Path(updated["runtime_dir"])
    assert private == registry_path.parent / "10001/protocol"
    assert [updated[k] for k in ("uin","http_port","ws_port","webui_port","auto_start")] == [registry["instances"][0][k] for k in ("uin","http_port","ws_port","webui_port","auto_start")]
    deadline = time.monotonic() + 5
    while not (private / "fixture-launched.txt").exists() and time.monotonic() < deadline: time.sleep(0.05)
    assert (private / "fixture-launched.txt").exists()
    assert (private / "loadNapCat.cjs").is_file()
    assert (private / "qqnt.json").is_file()
    assert json.loads((private / "config/webui.json").read_text())["port"] == updated["webui_port"]
    call("--account","10001","stop")
    assert unrelated.poll() is None
    print(json.dumps({"passed":True,"damagedLiveJobRecovered":True,"incompleteSourcePreservedOldJob":True,"registrationAndPortsPreserved":True,"unrelatedProcessPreserved":True,"realQQTouched":False,"fixture":str(ROOT)}))
finally:
    try: call("--account","10001","stop")
    except Exception: pass
    if supervisor and supervisor.poll() is None: supervisor.terminate()
    if unrelated and unrelated.poll() is None: unrelated.terminate()
