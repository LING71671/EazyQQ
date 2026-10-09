"""Collect real QQ startup evidence on hosted CI, without claiming login success."""
import base64
import ctypes
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import tempfile
import time
from urllib.request import ProxyHandler, build_opener
from urllib.parse import urlsplit

if os.environ.get("GITHUB_ACTIONS") != "true":
    raise SystemExit("Real QQ evidence collection is restricted to hosted CI")
PROJECT = Path(__file__).resolve().parents[2]
CLI = PROJECT / "src-tauri/target/debug/eazyqq_cli.exe"
HTTP = build_opener(ProxyHandler({}))

class Inspector:
    def __init__(self, endpoint):
        url = urlsplit(endpoint)
        self.sock = socket.create_connection((url.hostname, url.port), timeout=5)
        self.sock.settimeout(10)
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall(f"GET {url.path} HTTP/1.1\r\nHost: {url.hostname}:{url.port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n".encode())
        response = b""
        while not response.endswith(b"\r\n\r\n"): response += self.sock.recv(1)
        if b"101" not in response.split(b"\r\n")[0]: raise RuntimeError("Inspector handshake failed")
        self.sequence = 0
    def read(self, size):
        result = b""
        while len(result) < size:
            part = self.sock.recv(size-len(result))
            if not part: raise EOFError("Inspector closed")
            result += part
        return result
    def send(self, opcode, data):
        mask = os.urandom(4)
        header = bytes([0x80|opcode,0x80|len(data)]) if len(data)<126 else bytes([0x80|opcode,0xfe])+struct.pack('!H',len(data))
        self.sock.sendall(header+mask+bytes(v^mask[i%4] for i,v in enumerate(data)))
    def call(self, method, params=None):
        self.sequence += 1
        self.send(1,json.dumps({"id":self.sequence,"method":method,"params":params or {}}).encode())
        while True:
            h = self.read(2); opcode,n = h[0]&15,h[1]&127
            if n==126: n=struct.unpack('!H',self.read(2))[0]
            elif n==127: n=struct.unpack('!Q',self.read(8))[0]
            mask = self.read(4) if h[1]&128 else None
            data = self.read(n)
            if mask: data=bytes(v^mask[i%4] for i,v in enumerate(data))
            if opcode==9: self.send(10,data); continue
            if opcode!=1: continue
            value=json.loads(data)
            if value.get('id')==self.sequence: return value

def owned_pids(private):
    receipt=json.loads((private/'eazyqq-process.json').read_text())
    k=ctypes.WinDLL('kernel32',use_last_error=True)
    k.OpenJobObjectW.argtypes=[ctypes.c_uint,ctypes.c_int,ctypes.c_wchar_p]; k.OpenJobObjectW.restype=ctypes.c_void_p
    k.QueryInformationJobObject.argtypes=[ctypes.c_void_p,ctypes.c_int,ctypes.c_void_p,ctypes.c_uint,ctypes.c_void_p]
    k.CloseHandle.argtypes=[ctypes.c_void_p]
    handle=k.OpenJobObjectW(4,False,receipt['job_name'])
    if not handle: return []
    try:
        buffer=ctypes.create_string_buffer(65536)
        if not k.QueryInformationJobObject(handle,3,buffer,len(buffer),None): return []
        n=ctypes.c_uint.from_buffer(buffer,4).value
        return list((ctypes.c_size_t*n).from_buffer(buffer,8))
    finally: k.CloseHandle(handle)

def error_dialogs(pids):
    u=ctypes.WinDLL('user32',use_last_error=True)
    callback=ctypes.WINFUNCTYPE(ctypes.c_bool,ctypes.c_void_p,ctypes.c_void_p)
    u.EnumWindows.argtypes=[callback,ctypes.c_void_p]; u.EnumChildWindows.argtypes=[ctypes.c_void_p,callback,ctypes.c_void_p]
    u.GetWindowThreadProcessId.argtypes=[ctypes.c_void_p,ctypes.POINTER(ctypes.c_uint)]
    u.GetWindowTextW.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_int]
    result=[]
    def text(window):
        buffer=ctypes.create_unicode_buffer(8192); u.GetWindowTextW(window,buffer,len(buffer)); return buffer.value
    def window(handle,_):
        pid=ctypes.c_uint(); u.GetWindowThreadProcessId(handle,ctypes.byref(pid))
        if pid.value in pids and text(handle)=='Error':
            children=[]
            def child(handle,_):
                value=text(handle)
                if value: children.append(value)
                return True
            u.EnumChildWindows(handle,callback(child),None)
            result.append({'pid':pid.value,'text':children})
        return True
    u.EnumWindows(callback(window),None)
    return result

results=[]
for name,qq in [('profile',Path(os.environ['EAZYQQ_QA_PROFILE_QQ'])),('program-files',Path(os.environ['EAZYQQ_QA_PROGRAM_QQ']))]:
    root=Path(tempfile.mkdtemp(prefix=f'qq-trace-{name}-',dir=PROJECT/'.test-runtime'))
    shutil.copytree(PROJECT/'napcat',root/'napcat')
    (root/'napcat/config').mkdir(exist_ok=True)
    (root/'napcat/config/qq_path.txt').write_text(str(qq),encoding='utf-8')
    env={**os.environ,'EAZYQQ_ROOT':str(root)}
    report={'case':name,'fixture':str(root),'qq':str(qq),'loginAttempted':False,'evidenceCollected':False}
    private=None; inspector=None; supervisor=None
    def call(*args):
        response=subprocess.run([str(CLI),*args,'--json'],env=env,capture_output=True,text=True,encoding='utf-8',errors='replace',timeout=40)
        if response.returncode: raise RuntimeError(response.stderr[-1000:])
        return json.loads(response.stdout)
    try:
        call('accounts','add','--uin','10001')
        report['launch']=call('accounts','start','--uin','10001')
        private=next((root/'EazyQQ_Data/accounts').glob('*/10001/protocol'))
        config=json.loads((private/'config/webui.json').read_text())
        port=config['port']; listening=False; dialogs=[]; deadline=time.monotonic()+45
        while time.monotonic()<deadline:
            dialogs=error_dialogs(owned_pids(private))
            if dialogs: break
            try:
                with socket.create_connection(('127.0.0.1',port),timeout=.3): listening=True; break
            except OSError: time.sleep(.5)
        report['webuiTransportListening']=listening
        report['dialogs']=dialogs
        report['startupTrace']=call('napcat-doctor','--account','10001')['startupTrace']
        call('accounts','stop','--uin','10001')

        # Inspect before application code starts. Never resume into a real login.
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1',0)); debug_port=reservation.getsockname()[1]
        plan=json.loads((private/'eazyqq-launch.json').read_text())
        plan['request_id']='paused-inspector-'+name
        plan['args']=[str(qq),str(private/'NapCatWinBootHook.dll'),f'--inspect-brk=127.0.0.1:{debug_port}']
        (private/'eazyqq-launch.json').write_text(json.dumps(plan))
        supervisor=subprocess.Popen([str(CLI),'--protocol-supervisor',str(private)],env=env,creationflags=subprocess.CREATE_NO_WINDOW)
        deadline=time.monotonic()+25; endpoint=None
        while time.monotonic()<deadline:
            try:
                with HTTP.open(f'http://127.0.0.1:{debug_port}/json/list',timeout=1) as response: pages=json.load(response)
                endpoint=pages[0]['webSocketDebuggerUrl']; break
            except Exception: time.sleep(.5)
        if endpoint:
            inspector=Inspector(endpoint)
            inspector.call('Debugger.enable')
            inspector.call('Runtime.runIfWaitingForDebugger')
            patch=json.loads((private/'qqnt.json').read_text())
            app=qq.parent/'versions'/patch['version']/'resources/app'
            expression='''(() => {const fs=process.getBuiltinModule('fs');const path=process.getBuiltinModule('path');const mod=process.getBuiltinModule('module');const app=APP;const entry=path.join(app,MAIN);const out={versions:process.versions,execPath:process.execPath,resourcesPath:process.resourcesPath,cwd:process.cwd(),entry};for(const [name,fn] of Object.entries({exists:()=>fs.existsSync(entry),stat:()=>({size:fs.statSync(entry).size}),realpath:()=>fs.realpathSync(entry),resolve:()=>mod.createRequire(path.join(app,'package.json')).resolve(entry),receivedPackage:()=>{const p=JSON.parse(fs.readFileSync(path.join(app,'package.json'),'utf8'));return {main:p.main,version:p.version};}})){try{out[name]={ok:true,value:fn()};}catch(e){out[name]={ok:false,code:e.code,message:e.message};}}return out;})()'''.replace('APP',json.dumps(str(app))).replace('MAIN',json.dumps(patch['main']))
            report['pausedInspector']=inspector.call('Runtime.evaluate',{'expression':expression,'returnByValue':True})
        else: report['pausedInspector']={'available':False,'dialogs':error_dialogs(owned_pids(private))}
        report['evidenceCollected']=True
    except Exception as error: report['collectionError']=str(error)
    finally:
        if inspector: inspector.sock.close()
        try: call('accounts','stop','--uin','10001')
        except Exception: pass
        if supervisor:
            try: supervisor.wait(timeout=8)
            except subprocess.TimeoutExpired: supervisor.terminate()
        (root/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
        results.append(report)
print(json.dumps({'purpose':'diagnostic evidence collection, not release validation','cases':[{'case':r['case'],'evidenceCollected':r['evidenceCollected'],'webuiTransportListening':r.get('webuiTransportListening'),'collectionError':r.get('collectionError')} for r in results]}))
if not all(r['evidenceCollected'] for r in results): raise SystemExit(1)
