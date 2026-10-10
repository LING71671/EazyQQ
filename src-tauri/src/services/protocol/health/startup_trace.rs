//! Read-only startup provenance; filesystem evidence is never runtime readiness.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub code: String,
    pub state: String,
    pub detail: String,
    pub evidence: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupTrace {
    pub schema_version: u32,
    pub observed_at_ms: u64,
    pub attempt_id: Option<String>,
    pub attempt_origin: String,
    pub runtime_dir: String,
    pub stages: Vec<Stage>,
    pub first_failure: Option<String>,
    pub first_unconfirmed: Option<String>,
    pub ready: bool,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn stage(code: &str, state: &str, detail: &str, evidence: Value) -> Stage {
    Stage { code: code.into(), state: state.into(), detail: detail.into(), evidence }
}

fn file_evidence(path: &Path) -> Value {
    use std::io::Read;
    match std::fs::File::open(path) {
        Ok(mut file) => {
            let mut hash = Sha256::new(); let mut size = 0u64; let mut buffer = [0u8;65536];
            loop {
                match file.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {hash.update(&buffer[..n]);size += n as u64;},
                    Err(error) => return json!({"path":path,"exists":true,"readable":false,"errorKind":format!("{:?}",error.kind())}),
                }
            }
            json!({"path":path,"exists":true,"readable":true,"size":size,"sha256":format!("{:x}",hash.finalize())})
        },
        Err(error) => json!({"path":path,"exists":false,"errorKind":format!("{:?}",error.kind())}),
    }
}

pub fn begin(dir: &Path) -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    crate::services::infra::persistence::write_json(&dir.join("eazyqq-startup-attempt.json"),
        &json!({"attemptId":id,"startedAtMs":now_ms(),"appVersion":env!("CARGO_PKG_VERSION")}))?;
    Ok(id)
}

pub fn record_launch(dir: &Path, id: &str, ok: bool, detail: &str) -> Result<(), String> {
    crate::services::infra::persistence::write_json(&dir.join("eazyqq-startup-launch.json"),
        &json!({"attemptId":id,"observedAtMs":now_ms(),"ok":ok,"detail":detail}))
}

pub fn loader_source(dir: &Path, module_url: &str, id: &str) -> String {
    let receipt = serde_json::to_string(&dir.join("eazyqq-loader-witness.json")).unwrap();
    let id = serde_json::to_string(id).unwrap();
    let module_url = serde_json::to_string(module_url).unwrap();
    format!(r#"const fs = require('node:fs');
const witness = {{attemptId:{id},pid:process.pid,nodeVersion:process.versions.node,electronVersion:process.versions.electron||null}};
function record(stage,error) {{
 try {{
  const value = {{...witness,stage,observedAtMs:Date.now(),errorName:error?.name||null,errorCode:error?.code||null}};
  const pending = {receipt} + '.pending';
  fs.writeFileSync(pending,JSON.stringify(value),{{mode:0o600}});
  fs.renameSync(pending,{receipt});
 }} catch(writeError) {{console.error('[EazyQQ startup trace] witness write failed',writeError.code||'UNKNOWN');}}
}}
record('loader_entered');
(async () => {{try {{await import({module_url});record('module_imported');}} catch(error) {{record('module_failed',error);throw error;}}}})();
"#)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        let root = crate::services::logging::workspace_root().join(name);
        std::fs::create_dir_all(&root).unwrap(); root
    }

    #[test]
    fn stale_witness_and_listening_port_never_claim_authenticated_readiness() {
        let dir = fixture("startup-stale-witness");
        let id = begin(&dir).unwrap();
        std::fs::write(dir.join("eazyqq-loader-witness.json"), br#"{"attemptId":"older","stage":"module_imported"}"#).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        std::fs::create_dir_all(dir.join("config")).unwrap();
        std::fs::write(dir.join("config/webui.json"),json!({"port":listener.local_addr().unwrap().port(),"token":"never-export-this-token"}).to_string()).unwrap();
        let trace = capture(&dir);
        assert_eq!(trace.attempt_id.as_deref(),Some(id.as_str()));
        assert_eq!(trace.stages.iter().find(|s|s.code=="qq_module_load").unwrap().state,"unknown");
        assert_eq!(trace.stages.iter().find(|s|s.code=="webui_transport").unwrap().state,"passed");
        assert!(!trace.ready);
        assert!(!serde_json::to_string(&trace).unwrap().contains("never-export-this-token"));
    }

    #[test]
    fn legacy_launch_receipts_are_correlated_without_creating_new_attempts() {
        let dir = fixture("startup-legacy-launch");
        std::fs::write(dir.join("eazyqq-launch.json"),br#"{"request_id":"legacy-fixture"}"#).unwrap();
        std::fs::write(dir.join("eazyqq-launch-result.json"),br#"{"request_id":"legacy-fixture","ok":true,"detail":"accepted"}"#).unwrap();
        let trace = capture(&dir);
        assert_eq!(trace.attempt_id.as_deref(),Some("legacy:legacy-fixture"));
        assert_eq!(trace.attempt_origin,"legacy_launch_request");
        assert_eq!(trace.stages.iter().find(|s|s.code=="launch_request").unwrap().state,"passed");
        assert_eq!(trace.stages.iter().find(|s|s.code=="loader_execution").unwrap().state,"unknown");
        assert!(!dir.join("eazyqq-startup-attempt.json").exists());
    }

    #[test]
    fn real_node_execution_distinguishes_import_success_and_failure() {
        for (name,code,expected) in [("startup-import-ok","export const fixture = true;","module_imported"),
            ("startup-import-failed","throw Object.assign(new Error('private-secret'), {code:'FIXTURE_IMPORT_FAILURE'});","module_failed")]
        {
            let dir = fixture(name); let id = begin(&dir).unwrap();
            let module = dir.join("module.mjs"); std::fs::write(&module,code).unwrap();
            let url = reqwest::Url::from_file_path(&module).unwrap();
            let loader = dir.join("loadNapCat.cjs");
            std::fs::write(&loader,loader_source(&dir,url.as_str(),&id)).unwrap();
            let result = std::process::Command::new("node").arg(&loader).output().unwrap();
            assert_eq!(result.status.success(),expected=="module_imported");
            let witness = read_json(&dir.join("eazyqq-loader-witness.json")).unwrap();
            assert_eq!(witness["attemptId"],id); assert_eq!(witness["stage"],expected);
            assert!(!witness.to_string().contains("private-secret"));
        }
    }
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

pub fn capture(dir: &Path) -> StartupTrace {
    let mut stages = Vec::new();
    let source = super::boot::locate_napcat_dir();
    let payload: Vec<_> = ["NapCatWinBootMain.exe","NapCatWinBootHook.dll","napcat.mjs"].iter()
        .map(|name| {
            let mut evidence = file_evidence(&dir.join(name));
            let original = file_evidence(&source.join(name));
            evidence["source"] = original.clone();
            evidence["matchesSource"] = (evidence["exists"] == true && original["exists"] == true
                && evidence["sha256"] == original["sha256"]).into();
            evidence
        }).collect();
    stages.push(stage("private_payload",if payload.iter().all(|v|v["exists"]==true) {"passed"} else {"failed"},
        "仅检查私有载荷与来源散列，不代表 QQ 已执行代码",json!(payload)));

    let patch = read_json(&dir.join("qqnt.json"));
    let main = patch.as_ref().and_then(|p|p["main"].as_str());
    let app_dir = super::patch::configured_qq_path(dir).ok()
        .and_then(|qq|super::patch::package_metadata_path(&qq).ok())
        .and_then(|p|p.parent().and_then(|dir|crate::services::infra::filesystem::physical_path(dir).ok()));
    let expected = dir.join("loadNapCat.cjs");
    let resolved = app_dir.as_ref().zip(main).map(|(app,main)|app.join(main));
    let actual = resolved.as_ref().and_then(|path|std::fs::canonicalize(path).ok());
    let expected_real = std::fs::canonicalize(&expected).ok();
    let entry_matches = actual.is_some() && actual == expected_real;
    // Resolve the bridge root before following its final junction into another drive.
    let logical_bridge_root = resolved.as_ref().and_then(|p|std::path::absolute(p).ok())
        .and_then(|p|p.parent().and_then(Path::parent).map(Path::to_path_buf));
    let physical_bridge_root = logical_bridge_root.as_ref().and_then(|p|crate::services::infra::filesystem::physical_path(p).ok());
    let namespace_redirected = logical_bridge_root.as_ref().zip(physical_bridge_root.as_ref())
        .and_then(|(a,b)|crate::services::infra::filesystem::process_path(a).ok()
            .map(|a|!a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())));
    stages.push(stage("entry_resolution",if entry_matches {"passed"} else {"failed"},
        "普通文件系统解析结果；QQ 内部模块解析仍需运行时证据",
        json!({"appDir":app_dir,"packageMain":main,"resolvedPath":resolved,"canonicalPath":actual,"expectedLoader":expected,"matchesPrivateLoader":entry_matches,
            "logicalBridgeRoot":logical_bridge_root,"physicalBridgeRoot":physical_bridge_root,"callerNamespaceRedirected":namespace_redirected,
            "scope":"Caller file lookup only; QQ visibility is confirmed by the loader witness"})));
    stages.push(stage("private_loader",if expected.is_file() {"passed"} else {"failed"},"私有加载文件及散列",file_evidence(&expected)));

    let attempt = read_json(&dir.join("eazyqq-startup-attempt.json"));
    let mut id = attempt.as_ref().and_then(|v|v["attemptId"].as_str()).map(str::to_owned);
    let mut origin = if id.is_some() {"startup_attempt"} else {"missing"};
    let launch = read_json(&dir.join("eazyqq-startup-launch.json"));
    let mut launch = launch.filter(|v| id.is_some() && v["attemptId"].as_str() == id.as_deref());
    if id.is_none() {
        let plan = read_json(&dir.join("eazyqq-launch.json"));
        let result = read_json(&dir.join("eazyqq-launch-result.json"));
        if let Some(request_id) = plan.as_ref().and_then(|v|v["request_id"].as_str()) {
            if let Some(result) = result.filter(|v|v["request_id"].as_str()==Some(request_id)) {
                id = Some(format!("legacy:{request_id}")); origin = "legacy_launch_request";
                launch = Some(json!({"ok":result["ok"],"detail":result["detail"],"legacyRequestId":request_id}));
            }
        }
    }
    stages.push(stage("launch_request",match launch.as_ref().and_then(|v|v["ok"].as_bool()) {Some(true)=>"passed",Some(false)=>"failed",None=>"unknown"},
        "启动请求接受不等于模块加载成功",launch.unwrap_or(json!({"reason":"No matching launch receipt"}))));
    let witness = read_json(&dir.join("eazyqq-loader-witness.json"));
    let witness = witness.filter(|v|id.is_some() && v["attemptId"].as_str()==id.as_deref());
    stages.push(stage("loader_execution",if witness.is_some() {"passed"} else {"unknown"},
        "仅本次加载器写入的凭证证明入口曾执行",json!({"witnessMatched":witness.is_some()})));
    stages.push(stage("qq_module_load",match witness.as_ref().and_then(|v|v["stage"].as_str()) {Some("module_imported")=>"passed",Some("module_failed")=>"failed",_=>"unknown"},
        "由本次加载器执行写入；缺少或陈旧凭证不能当作成功",witness.unwrap_or(json!({"reason":"No matching loader witness"}))));
    stages.push(stage("owned_process",if super::ownership::is_running(dir) {"passed"} else {"unknown"},
        "只检查受验证的 Job，进程存活不等于协议就绪",json!({"managed":super::ownership::is_running(dir)})));
    let config = read_json(&dir.join("config/webui.json"));
    let port = config.and_then(|v|v["port"].as_u64()).and_then(|v|u16::try_from(v).ok());
    let listening = port.is_some_and(|port|std::net::TcpStream::connect_timeout(&([127,0,0,1],port).into(),std::time::Duration::from_millis(100)).is_ok());
    stages.push(stage("webui_transport",if listening {"passed"} else {"unknown"},
        "仅 TCP 探测；认证、二维码和账号身份尚需分别验证",json!({"port":port,"listening":listening})));
    stages.push(stage("authenticated_session","unknown","此文件链路快照不执行登录；由只读账号探测确认身份",json!({})));
    let first_failure = stages.iter().find(|s|s.state=="failed").map(|s|s.code.clone());
    let first_unconfirmed = stages.iter().find(|s|s.state=="unknown").map(|s|s.code.clone());
    StartupTrace {schema_version:1,observed_at_ms:now_ms(),attempt_id:id,attempt_origin:origin.into(),runtime_dir:dir.display().to_string(),stages,first_failure,first_unconfirmed,ready:false}
}
