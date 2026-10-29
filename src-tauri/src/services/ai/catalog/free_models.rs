//! Current native catalogue plus explicit credential-free inference evidence.
use super::opencode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::Notify;

#[derive(Clone, Debug, Default)]
pub(crate) struct Cancellation(Arc<CancellationState>);
#[derive(Debug, Default)]
struct CancellationState { cancelled: AtomicBool, changed: Notify }
impl Cancellation {
    pub(crate) fn cancel(&self) { self.0.cancelled.store(true, Ordering::SeqCst); self.0.changed.notify_waiters(); }
    pub(crate) fn is_cancelled(&self) -> bool { self.0.cancelled.load(Ordering::SeqCst) }
    pub(crate) async fn cancelled(&self) {
        loop {
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.is_cancelled() { return; }
            changed.await;
        }
    }
}

struct RegisteredRequest { cancellation: Cancellation, expires: Option<Instant> }
type Requests = HashMap<(PathBuf, String), RegisteredRequest>;
fn requests() -> &'static Mutex<Requests> {
    static REQUESTS: OnceLock<Mutex<Requests>> = OnceLock::new();
    REQUESTS.get_or_init(Default::default)
}
fn valid_request_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
fn purge_pending(values: &mut Requests) { values.retain(|_, value| value.expires.is_none_or(|time| time > Instant::now())); }

/// Cancellation is scoped to the immutable account runtime and exact request ID.
pub fn cancel(root: &Path, request_id: &str) -> Result<bool, String> {
    if !valid_request_id(request_id) { return Err("检测请求编号格式无效".into()); }
    let root = crate::services::infra::filesystem::physical_path(root)
        .or_else(|_| std::path::absolute(root).map_err(|error| error.to_string()))?;
    let mut values = requests().lock().map_err(|_| "无法读取检测取消状态")?;
    purge_pending(&mut values);
    let key = (root, request_id.into());
    if let Some(value) = values.get(&key) { value.cancellation.cancel(); return Ok(value.expires.is_none()); }
    // A cleanup call can arrive before Tauri has polled the detection future.
    if values.values().filter(|value| value.expires.is_some()).count() >= 128 {
        return Err("待处理的检测取消请求过多".into());
    }
    let cancellation = Cancellation::default();
    cancellation.cancel();
    values.insert(key, RegisteredRequest { cancellation, expires: Some(Instant::now() + Duration::from_secs(60)) });
    Ok(false)
}

pub fn is_running(root: &Path) -> bool {
    let Ok(root) = crate::services::infra::filesystem::physical_path(root) else { return false; };
    requests().lock().map(|values| values.iter().any(|((account, _), value)| account == &root && value.expires.is_none())).unwrap_or(false)
}

/// Window shutdown must await this before destroying the process or its runtime.
pub async fn cancel_and_wait(root: &Path) -> Result<bool, String> {
    let root = match crate::services::infra::filesystem::physical_path(root) {
        Ok(root) => root,
        Err(_) => return Ok(false),
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut cancelled = false;
    loop {
        let running = {
            let values = requests().lock().map_err(|_| "无法读取检测取消状态")?;
            let mut count = 0;
            for ((account, _), value) in values.iter() {
                if account == &root && value.expires.is_none() { value.cancellation.cancel(); count += 1; }
            }
            count
        };
        if running == 0 { return Ok(cancelled); }
        cancelled = true;
        if Instant::now() >= deadline { return Err("检测进程尚未完成回收，已保留窗口，请重试关闭".into()); }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

struct DetectionRequest { key: (PathBuf, String), cancellation: Cancellation, lock: Option<crate::services::infra::persistence::FileLock> }
impl DetectionRequest {
    fn begin(root: &Path, request_id: &str) -> Result<Self, String> {
        if !valid_request_id(request_id) { return Err("检测请求编号格式无效".into()); }
        let root = crate::services::infra::filesystem::physical_directory(root)?;
        let lock = crate::services::infra::persistence::FileLock::try_acquire(&root.join("free-model-detection.lock"))
            .map_err(|_| "当前账号已有免费模型检测，请先停止或等待完成".to_string())?;
        let key = (root, request_id.into());
        let mut values = requests().lock().map_err(|_| "无法登记检测取消状态")?;
        purge_pending(&mut values);
        let cancellation = values.remove(&key).map(|value| value.cancellation).unwrap_or_default();
        values.insert(key.clone(), RegisteredRequest { cancellation: cancellation.clone(), expires: None });
        Ok(Self { key, cancellation, lock: Some(lock) })
    }
}
impl Drop for DetectionRequest {
    fn drop(&mut self) {
        self.lock.take();
        if let Ok(mut values) = requests().lock() { values.remove(&self.key); }
    }
}

async fn begin_after_cancelled_request(root: &Path, request_id: &str) -> Result<DetectionRequest, String> {
    let root = crate::services::infra::filesystem::physical_directory(root)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match DetectionRequest::begin(&root, request_id) {
            Ok(request) => return Ok(request),
            Err(error) => {
                let cancelled_request = requests().lock().map_err(|_| "无法读取检测取消状态")?.iter()
                    .any(|((account, _), value)| account == &root && value.expires.is_none() && value.cancellation.is_cancelled());
                if !cancelled_request || Instant::now() >= deadline { return Err(error); }
                // A remounted settings pane can briefly overlap the cancelled child being reaped.
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModel {
    pub id: String,
    pub name: String,
    pub state: String,
    pub detail: String,
    pub observed_at_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModelsReport {
    pub runtime_version: String,
    pub observed_at_ms: u64,
    pub catalogue_ids: Vec<String>,
    pub models: Vec<FreeModel>,
    pub credentials_used: bool,
    pub source: String,
    #[serde(default)]
    pub request_id: String,
    #[serde(default)]
    pub status: DetectionStatus,
    #[serde(default)]
    pub completed: usize,
    #[serde(default)]
    pub total: usize,
    #[serde(default)]
    pub catalogue_refreshed: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub cleanup_error: Option<String>,
    #[serde(default)]
    pub history: Vec<FreeModelsHistory>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionStatus { #[default] Completed, Cancelled, Failed }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModelsHistory {
    pub request_id: String,
    pub status: DetectionStatus,
    pub observed_at_ms: u64,
    pub models: Vec<FreeModel>,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModelProgress {
    pub request_id: String,
    pub completed: usize,
    pub total: usize,
    pub model: FreeModel,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn parse_catalogue(output: &str) -> Result<Vec<Value>, String> {
    let mut input = output;
    let mut values = Vec::new();
    while !input.trim().is_empty() {
        input = input.trim_start();
        let (header, tail) = input
            .split_once('\n')
            .ok_or("OpenCode 模型元数据格式不完整")?;
        let id = header
            .trim()
            .strip_prefix("opencode/")
            .ok_or("OpenCode 返回了意外的供应方")?;
        let mut parser = serde_json::Deserializer::from_str(tail).into_iter::<Value>();
        let value = parser
            .next()
            .ok_or("OpenCode 模型元数据缺失")?
            .map_err(|error| error.to_string())?;
        if value["id"].as_str() != Some(id) || value["providerID"] != "opencode" {
            return Err("OpenCode 模型标识与元数据不一致".into());
        }
        input = &tail[parser.byte_offset()..];
        values.push(value);
    }
    if values.is_empty() {
        return Err("OpenCode 当前目录没有模型，无法确认免费资格".into());
    }
    Ok(values)
}

fn zero_priced(value: &Value) -> bool {
    value["cost"]["input"].as_f64() == Some(0.0)
        && value["cost"]["output"].as_f64() == Some(0.0)
        && ["read", "write"].iter().all(|field| {
            value["cost"]["cache"][field].is_null()
                || value["cost"]["cache"][field].as_f64() == Some(0.0)
        })
}

fn failure_state(message: &str) -> &'static str {
    let message = message.to_lowercase();
    if ["deprecated", "retired", "no longer supported"]
        .iter()
        .any(|term| message.contains(term))
    {
        "retired"
    } else if [
        "api key",
        "sign in",
        "credential",
        "payment",
        "credits",
        "within opencode",
        "tool operation",
    ]
    .iter()
    .any(|term| message.contains(term))
    {
        "requires_conditions"
    } else {
        "unconfirmed"
    }
}

pub async fn catalogue(root: &Path) -> Result<Vec<Value>, String> {
    catalogue_with_key(root, None).await
}

pub async fn catalogue_with_key(root: &Path, key: Option<&str>) -> Result<Vec<Value>, String> {
    catalogue_cancellable(root, key, &Cancellation::default(), opencode::command()?).await
}

async fn catalogue_cancellable(root: &Path, key: Option<&str>, cancellation: &Cancellation, mut command: tokio::process::Command) -> Result<Vec<Value>, String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    command
        .args(["models", "opencode", "--refresh", "--verbose"])
        .current_dir(root)
        .stderr(std::process::Stdio::null());
    opencode::anonymous_environment(&mut command, root);
    if let Some(key) = key.filter(|key| !key.trim().is_empty()) {
        command.env("OPENCODE_CONFIG_CONTENT",serde_json::json!({"enabled_providers":["opencode"],"provider":{"opencode":{"options":{"apiKey":key}}}}).to_string());
    }
    let output = run_owned_command(command, None, Duration::from_secs(40), cancellation, 4_000_000).await?;
    if !output.status.success() {
        return Err("OpenCode 实时目录读取失败，请检查原生运行时与网络".into());
    }
    if output.stdout.len() > 4_000_000 {
        return Err("OpenCode 模型目录超过大小限制".into());
    }
    parse_catalogue(&String::from_utf8_lossy(&output.stdout))
}

/// Every child is either reaped normally or killed and reaped before cancellation completes.
pub(crate) async fn run_owned_command(
    mut command: tokio::process::Command,
    input: Option<&[u8]>,
    timeout: Duration,
    cancellation: &Cancellation,
    output_limit: usize,
) -> Result<std::process::Output, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    if cancellation.is_cancelled() { return Err("检测已停止".into()); }
    command.kill_on_drop(true).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    if input.is_some() { command.stdin(std::process::Stdio::piped()); }
    let mut child = command.spawn().map_err(|error| format!("无法启动 OpenCode 检测进程: {error}"))?;
    let operation = async {
        if let Some(input) = input {
            let mut stdin = child.stdin.take().ok_or("OpenCode 检测输入不可用")?;
            stdin.write_all(input).await.map_err(|error| error.to_string())?;
        }
        let output = child.stdout.take().ok_or("OpenCode 检测输出不可用")?;
        let mut bytes = Vec::new();
        output.take(output_limit as u64 + 1).read_to_end(&mut bytes).await.map_err(|error| error.to_string())?;
        if bytes.len() > output_limit { return Err("OpenCode 检测输出超过大小限制".into()); }
        let status = child.wait().await.map_err(|error| error.to_string())?;
        Ok(std::process::Output { status, stdout: bytes, stderr: Vec::new() })
    };
    let result = tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err("检测已停止".into()),
        result = tokio::time::timeout(timeout, operation) => result.unwrap_or_else(|_| Err(format!("OpenCode 检测超过 {} 秒，当前结果未确认", timeout.as_secs()))),
    };
    if result.is_err() { let _ = child.kill().await; }
    result
}

const DIRECTORY_PREFIX: &str = "free-model-detection-";
const OWNER_FILE: &str = ".eazyqq-free-detection.json";
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DirectoryOwner { schema_version: u8, directory_id: String, request_id: String }
fn directory_id(name: &str) -> Option<&str> {
    name.strip_prefix(DIRECTORY_PREFIX).filter(|id| id.len() == 16 && id.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()))
}
fn is_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))] { metadata.file_type().is_symlink() }
}
fn verify_private_tree(path: &Path, root: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if is_link(&metadata) || !crate::services::infra::filesystem::physical_path(path)?.starts_with(root) {
        return Err("检测目录包含链接或越界路径，已保留原文件".into());
    }
    if metadata.is_dir() {
        for child in std::fs::read_dir(path).map_err(|error| error.to_string())? {
            verify_private_tree(&child.map_err(|error| error.to_string())?.path(), root)?;
        }
    }
    Ok(())
}
fn cleanup_owned_directories(root: &Path) -> Result<(), String> {
    for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        let Some(id) = name.to_str().and_then(directory_id) else { continue; };
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| error.to_string())?;
        if !metadata.is_dir() || is_link(&metadata) { continue; }
        let marker = path.join(OWNER_FILE);
        let Ok(metadata) = std::fs::symlink_metadata(&marker) else { continue; };
        if !metadata.is_file() || is_link(&metadata) || metadata.len() > 1024 { continue; }
        let owner = std::fs::read(&marker).ok().and_then(|bytes| serde_json::from_slice::<DirectoryOwner>(&bytes).ok());
        let Some(owner) = owner.filter(|owner| owner.schema_version == 1 && owner.directory_id == id && valid_request_id(&owner.request_id)) else { continue; };
        let _ = owner;
        if path.parent() != Some(root) { return Err("检测目录清理目标越界".into()); }
        verify_private_tree(&path, root)?;
        // The resolved absolute target and every descendant were checked above.
        std::fs::remove_dir_all(&path).map_err(|error| format!("检测临时目录清理失败: {error}"))?;
    }
    Ok(())
}
fn create_directory(root: &Path, request_id: &str) -> Result<PathBuf, String> {
    let mut random = [0u8; 8];
    getrandom::fill(&mut random).map_err(|error| error.to_string())?;
    let id: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let directory = root.join(format!("{DIRECTORY_PREFIX}{id}"));
    std::fs::create_dir(&directory).map_err(|error| error.to_string())?;
    crate::services::infra::persistence::write_json(&directory.join(OWNER_FILE), &DirectoryOwner { schema_version: 1, directory_id: id, request_id: request_id.into() })?;
    Ok(directory)
}
fn read_report(path: &Path) -> Option<FreeModelsReport> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || is_link(&metadata) || metadata.len() > 4_000_000 { return None; }
    serde_json::from_slice::<FreeModelsReport>(&std::fs::read(path).ok()?).ok().filter(|report| !report.credentials_used)
}
fn history(root: &Path) -> Vec<FreeModelsHistory> {
    let mut reports: Vec<_> = ["free-model-last-attempt.json", "free-model-report.json"].iter()
        .filter_map(|name| read_report(&root.join(name))).collect();
    reports.sort_by_key(|report| std::cmp::Reverse(report.observed_at_ms));
    reports.dedup_by(|left, right| left.observed_at_ms == right.observed_at_ms && left.request_id == right.request_id);
    reports.into_iter().map(|report| FreeModelsHistory { request_id: report.request_id, status: report.status, observed_at_ms: report.observed_at_ms, models: report.models }).collect()
}

pub async fn detect<F>(
    root: &Path,
    probe: bool,
    request_id: &str,
    progress: F,
) -> Result<FreeModelsReport, String>
where
    F: FnMut(FreeModelProgress) + Send,
{
    detect_with_commands(root, probe, request_id, progress, opencode::command).await
}

async fn detect_with_commands<F, C>(root: &Path, probe: bool, request_id: &str, mut progress: F, mut command: C) -> Result<FreeModelsReport, String>
where F: FnMut(FreeModelProgress) + Send,
      C: FnMut() -> Result<tokio::process::Command, String> + Send {
    let request = begin_after_cancelled_request(root, request_id).await?;
    let root = &request.key.0;
    let mut report = FreeModelsReport {
        runtime_version: String::new(),
        observed_at_ms: now(),
        catalogue_ids: Vec::new(),
        models: Vec::new(),
        credentials_used: false,
        source:
            "native_opencode_catalogue_refresh_requested; anonymous_data_config_and_environment"
                .into(),
        request_id: request_id.into(), status: DetectionStatus::Completed, completed: 0, total: 0,
        catalogue_refreshed: false, error: None, cleanup_error: None, history: history(root),
    };
    if let Err(error) = cleanup_owned_directories(root) {
        report.status = DetectionStatus::Failed;
        report.error = Some("旧检测临时目录未能安全清理，本次未启动检测".into());
        report.cleanup_error = Some(error);
        return Ok(report);
    }
    let result = detect_current(root, probe, &request.cancellation, &mut report, &mut progress, &mut command).await;
    if request.cancellation.is_cancelled() { report.status = DetectionStatus::Cancelled; }
    else if let Err(error) = result {
        report.status = DetectionStatus::Failed;
        report.error = Some(crate::services::diagnostics::redact(&error, &[]));
    }
    report.observed_at_ms = now();
    report.cleanup_error = cleanup_owned_directories(root).err();
    if probe {
        let mut stored = report.clone();
        stored.history.clear();
        let write_result = (|| {
            crate::services::infra::persistence::write_json(&root.join("free-model-last-attempt.json"), &stored)?;
            if stored.status == DetectionStatus::Completed {
                crate::services::infra::persistence::write_json(&root.join("free-model-report.json"), &stored)?;
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = write_result {
            report.error = Some(format!("检测证据保存失败: {}", crate::services::diagnostics::redact(&error, &[])));
            report.status = DetectionStatus::Failed;
        }
    }
    Ok(report)
}

async fn detect_current<F, C>(root: &Path, probe: bool, cancellation: &Cancellation, report: &mut FreeModelsReport, progress: &mut F, command: &mut C) -> Result<(), String>
where F: FnMut(FreeModelProgress) + Send,
      C: FnMut() -> Result<tokio::process::Command, String> + Send {
    if cancellation.is_cancelled() { return Ok(()); }
    let directory = create_directory(root, &report.request_id)?;
    let values = catalogue_cancellable(&directory.join("catalogue"), None, cancellation, command()?).await?;
    report.catalogue_ids = values.iter().filter_map(|value| value["id"].as_str().map(str::to_string)).collect();
    report.catalogue_refreshed = true;
    let mut version_command = command()?;
    version_command.arg("--version").current_dir(&directory);
    opencode::anonymous_environment(&mut version_command, &directory);
    let version = run_owned_command(version_command, None, Duration::from_secs(5), cancellation, 4096).await?;
    if !version.status.success() { return Err("读取 OpenCode 版本失败".into()); }
    report.runtime_version = String::from_utf8_lossy(&version.stdout).trim().into();
    let candidates: Vec<_> = values.iter().filter(|v| zero_priced(v)).collect();
    let total = candidates.len();
    report.total = total;
    for (index, value) in candidates.into_iter().enumerate() {
        if cancellation.is_cancelled() { return Ok(()); }
        let id = value["id"].as_str().unwrap().to_string();
        let mut model = FreeModel {
            name: value["name"].as_str().unwrap_or(&id).into(),
            id,
            state: "candidate".into(),
            detail: "目录标价为零；本次目录刷新尚未进行免凭据调用验证".into(),
            observed_at_ms: now(),
        };
        if value["status"] == "deprecated" || value["status"] == "retired" {
            model.state = "retired".into();
            model.detail = "原生模型元数据明确标记停用".into();
        } else if probe {
            let attempt = directory.join(format!("probe-{index}"));
            match opencode::probe_without_credentials_command(&attempt, &model.id, cancellation, command()?).await {
                Ok(()) => {
                    model.state = "available".into();
                    model.detail = "本次隔离请求未使用账号凭据或 API Key，已返回模型文本".into();
                }
                Err(error) => {
                    if cancellation.is_cancelled() { return Ok(()); }
                    model.state = failure_state(&error).into();
                    model.detail = crate::services::diagnostics::redact(&error, &[]);
                }
            }
        }
        model.observed_at_ms = now();
        report.models.push(model.clone());
        report.completed = index + 1;
        progress(FreeModelProgress {
            request_id: report.request_id.clone(),
            completed: index + 1,
            total,
            model,
        });
    }
    if let Some(previous) = read_report(&root.join("free-model-report.json")) {
        for model in previous.models {
            if !report.catalogue_ids.contains(&model.id) {
                report.models.push(FreeModel {
                    state: "removed_from_catalogue".into(),
                    detail: "本次原生目录已没有此模型；不以网络失败推断下架".into(),
                    observed_at_ms: now(),
                    ..model
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root(tag: &str) -> PathBuf {
        let path = crate::services::logging::workspace_root().join(format!("free-lifecycle-{tag}-{}", now()));
        crate::services::infra::filesystem::physical_directory(&path).unwrap()
    }

    #[test]
    fn cancellation_is_bound_to_account_and_request_and_handles_early_cleanup() {
        let first = root("cancel-first");
        let second = root("cancel-second");
        let running = DetectionRequest::begin(&first, "running").unwrap();
        assert!(!cancel(&second, "running").unwrap());
        assert!(!cancel(&first, "other").unwrap());
        assert!(!running.cancellation.is_cancelled());
        assert!(cancel(&first, "running").unwrap());
        assert!(running.cancellation.is_cancelled());
        drop(running);
        assert!(!cancel(&first, "before-start").unwrap());
        let early = DetectionRequest::begin(&first, "before-start").unwrap();
        assert!(early.cancellation.is_cancelled());
        assert!(DetectionRequest::begin(&first, "duplicate").is_err());
        assert!(DetectionRequest::begin(&second, "independent").is_ok());
    }

    #[test]
    fn cleanup_removes_only_marked_detection_directories_and_keeps_user_state() {
        let root = root("cleanup");
        let owned = create_directory(&root, "owned").unwrap();
        std::fs::create_dir_all(owned.join("probe-0/cache")).unwrap();
        std::fs::write(owned.join("probe-0/cache/data"), "temporary").unwrap();
        for name in ["auth", "cache", "no-key", "free-model-detection-0123456789abcdef", "free-model-detection-user"] {
            std::fs::create_dir(root.join(name)).unwrap();
            std::fs::write(root.join(name).join("keep"), "user-state").unwrap();
        }
        cleanup_owned_directories(&root).unwrap();
        assert!(!owned.exists());
        for name in ["auth", "cache", "no-key", "free-model-detection-0123456789abcdef", "free-model-detection-user"] {
            assert_eq!(std::fs::read_to_string(root.join(name).join("keep")).unwrap(), "user-state");
        }
    }

    #[test]
    fn cleanup_refuses_a_nested_link_without_touching_its_target() {
        let root = root("linked-cleanup");
        let outside = root.join("user-auth");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("auth.json"), "private").unwrap();
        let owned = create_directory(&root, "linked").unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&outside, owned.join("linked-auth")).unwrap();
        #[cfg(not(windows))]
        std::os::unix::fs::symlink(&outside, owned.join("linked-auth")).unwrap();
        assert!(cleanup_owned_directories(&root).is_err());
        assert!(owned.exists());
        assert_eq!(std::fs::read_to_string(outside.join("auth.json")).unwrap(), "private");
    }

    #[tokio::test]
    async fn cancelled_before_start_never_constructs_a_runtime_command() {
        let root = root("early-cancel");
        cancel(&root, "early").unwrap();
        let report = detect_with_commands(&root, true, "early", |_| {}, || panic!("An early cancellation must not launch a runtime")).await.unwrap();
        assert_eq!(report.status, DetectionStatus::Cancelled);
        assert!(!report.catalogue_refreshed);
        assert_eq!(report.completed, 0);
        assert_eq!(history(&root)[0].status, DetectionStatus::Cancelled);
        assert!(!root.join("free-model-report.json").exists());
    }

    #[tokio::test]
    async fn window_shutdown_waits_asynchronously_for_only_its_fixed_account() {
        let first_root = root("window-first");
        let second_root = root("window-second");
        let first = DetectionRequest::begin(&first_root, "window-request").unwrap();
        let second = DetectionRequest::begin(&second_root, "other-request").unwrap();
        let shutdown = cancel_and_wait(&first_root);
        let finish_cleanup = async {
            tokio::time::sleep(Duration::from_millis(30)).await;
            assert!(first.cancellation.is_cancelled());
            assert!(!second.cancellation.is_cancelled());
            drop(first);
        };
        let (result, ()) = tokio::join!(shutdown, finish_cleanup);
        assert!(result.unwrap());
        assert!(!is_running(&first_root));
        assert!(is_running(&second_root));
        assert!(!cancel_and_wait(&first_root).await.unwrap());
    }

    #[tokio::test]
    async fn failed_refresh_reports_failure_and_retains_separate_history() {
        let root = root("failed-refresh");
        let mut previous = detect_with_commands(&root, true, "seed", |_| {}, || Err("fixture unavailable".into())).await.unwrap();
        previous.status = DetectionStatus::Completed;
        previous.request_id = "past".into();
        previous.observed_at_ms = 1;
        previous.models.push(FreeModel { id: "past".into(), name: "Past".into(), state: "available".into(), detail: "historical proof".into(), observed_at_ms: 1 });
        crate::services::infra::persistence::write_json(&root.join("free-model-report.json"), &previous).unwrap();
        let report = detect_with_commands(&root, false, "failure", |_| {}, || Err("fixture unavailable".into())).await.unwrap();
        assert_eq!(report.status, DetectionStatus::Failed);
        assert!(report.error.as_deref().unwrap().contains("fixture unavailable"));
        assert!(!report.catalogue_refreshed);
        assert!(report.models.is_empty());
        assert!(report.history.iter().any(|entry| entry.models.iter().any(|model| model.id == "past")));
        assert_eq!(std::fs::read_dir(&root).unwrap().filter_map(Result::ok).filter(|entry| entry.file_name().to_str().and_then(directory_id).is_some()).count(), 0);
    }

    #[cfg(windows)]
    fn fixture(root: &Path) -> PathBuf {
        let script = root.join("fixture.ps1");
        std::fs::write(&script, r#"
if ($env:OPENCODE_API_KEY) { exit 9 }
if ($args[0] -eq 'models') {
    [Console]::WriteLine('opencode/first')
    [Console]::WriteLine('{"id":"first","providerID":"opencode","cost":{"input":0,"output":0}}')
    [Console]::WriteLine('opencode/second')
    [Console]::WriteLine('{"id":"second","providerID":"opencode","cost":{"input":0,"output":0}}')
} elseif ($args[0] -eq '--version') {
    [Console]::WriteLine('fixture-runtime')
} elseif ($args[0] -eq 'run') {
    if ($env:EAZYQQ_FREE_FIXTURE_BLOCK -eq '1') {
        Set-Content -LiteralPath $env:EAZYQQ_FREE_FIXTURE_PID -Value $PID
        Start-Sleep -Seconds 30
    }
    [Console]::WriteLine('{"type":"text","part":{"text":"OK"}}')
} else { exit 8 }
"#).unwrap();
        script
    }
    #[cfg(windows)]
    fn fixture_command(script: &Path, block: bool, pid: &Path) -> tokio::process::Command {
        let mut command = tokio::process::Command::new("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"]).arg(script)
            .env("EAZYQQ_FREE_FIXTURE_BLOCK", if block { "1" } else { "0" })
            .env("EAZYQQ_FREE_FIXTURE_PID", pid)
            .env("OPENCODE_API_KEY", "must-be-removed-by-anonymous-environment");
        command.creation_flags(0x08000000);
        command
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn metadata_has_no_probe_and_partial_cancel_keeps_complete_history() {
        let root = root("partial");
        let script = fixture(&root);
        let pid = root.join("child.pid");
        let mut count = 0;
        let metadata = detect_with_commands(&root, false, "metadata", |_| {}, || { count += 1; Ok(fixture_command(&script, false, &pid)) }).await.unwrap();
        assert_eq!(count, 2, "Metadata refresh must not construct an inference child");
        assert!(metadata.models.iter().all(|model| model.state == "candidate"));
        assert!(!root.join("free-model-report.json").exists());
        let complete = detect_with_commands(&root, true, "complete", |_| {}, || Ok(fixture_command(&script, false, &pid))).await.unwrap();
        assert_eq!(complete.status, DetectionStatus::Completed);
        assert_eq!(complete.completed, 2);
        let completed_bytes = std::fs::read(root.join("free-model-report.json")).unwrap();
        let partial = detect_with_commands(&root, true, "partial", |event| { if event.completed == 1 { cancel(&root, "partial").unwrap(); } }, || Ok(fixture_command(&script, false, &pid))).await.unwrap();
        assert_eq!(partial.status, DetectionStatus::Cancelled);
        assert_eq!((partial.completed, partial.total), (1, 2));
        assert_eq!(partial.models.len(), 1);
        assert_eq!(partial.models[0].state, "available");
        assert_eq!(std::fs::read(root.join("free-model-report.json")).unwrap(), completed_bytes);
        assert_eq!(history(&root).len(), 2);
        assert!(history(&root).iter().any(|entry| entry.status == DetectionStatus::Cancelled));
        assert!(!std::fs::read_dir(root).unwrap().filter_map(Result::ok).any(|entry| entry.file_name().to_str().and_then(directory_id).is_some()));
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn cancellation_kills_and_reaps_only_the_owned_probe_child() {
        #[link(name = "kernel32")]
        extern "system" {
            fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
            fn WaitForSingleObject(handle: *mut std::ffi::c_void, timeout: u32) -> u32;
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }
        let root = root("owned-child");
        let script = fixture(&root);
        let pid_file = root.join("child.pid");
        let mut unrelated_command = tokio::process::Command::new("powershell.exe");
        unrelated_command.args(["-NoProfile", "-NonInteractive", "-Command", "Start-Sleep -Seconds 30"])
            .creation_flags(0x08000000).kill_on_drop(true)
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let mut unrelated = unrelated_command.spawn().unwrap();
        let task_root = root.clone(); let task_script = script.clone(); let task_pid = pid_file.clone();
        let detection = tokio::spawn(async move { detect_with_commands(&task_root, true, "owned-child", |_| {}, || Ok(fixture_command(&task_script, true, &task_pid))).await });
        let pid = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(text) = std::fs::read_to_string(&pid_file) {
                    if let Ok(pid) = text.trim().parse::<u32>() { break pid; }
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await.unwrap();
        let handle = unsafe { OpenProcess(0x00100000, 0, pid) };
        assert!(!handle.is_null());
        assert!(tokio::time::timeout(Duration::from_secs(3), cancel_and_wait(&root)).await.unwrap().unwrap());
        let report = tokio::time::timeout(Duration::from_secs(3), detection).await.unwrap().unwrap().unwrap();
        assert_eq!(report.status, DetectionStatus::Cancelled);
        assert_eq!(report.completed, 0);
        assert_eq!(unsafe { WaitForSingleObject(handle, 0) }, 0, "The owned child must have exited before reporting cancellation");
        unsafe { CloseHandle(handle); }
        assert!(report.cleanup_error.is_none());
        assert!(unrelated.try_wait().unwrap().is_none(), "Cancelling a probe must preserve another process");
        unrelated.kill().await.unwrap();
    }
    #[test]
    fn free_candidates_need_explicit_zero_prices_not_a_name_suffix() {
        assert!(zero_priced(
            &serde_json::json!({"id":"ordinary","cost":{"input":0,"output":0}})
        ));
        assert!(!zero_priced(
            &serde_json::json!({"id":"paid-free","cost":{"input":1,"output":0}})
        ));
        assert!(!zero_priced(&serde_json::json!({"id":"missing-free"})));
        assert!(!zero_priced(
            &serde_json::json!({"cost":{"input":0,"output":0,"cache":{"read":1}}})
        ));
    }
    #[test]
    fn native_records_are_bound_to_their_provider_and_exact_id() {
        let valid="opencode/plain\n{\"id\":\"plain\",\"providerID\":\"opencode\",\"cost\":{\"input\":0,\"output\":0}}\n";
        assert_eq!(parse_catalogue(valid).unwrap().len(), 1);
        assert!(parse_catalogue(&valid.replace("opencode/plain", "opencode/other")).is_err());
        assert!(parse_catalogue("opencode/partial\n{").is_err());
    }
    #[test]
    fn transient_failures_do_not_claim_retirement_or_free_access() {
        assert_eq!(
            failure_state("request timed out after 25 seconds"),
            "unconfirmed"
        );
        assert_eq!(failure_state("429 rate limited"), "unconfirmed");
        assert_eq!(failure_state("API key required"), "requires_conditions");
        assert_eq!(failure_state("model has been deprecated"), "retired");
        assert_eq!(
            failure_state("Model not found in local catalogue"),
            "unconfirmed"
        );
    }
}
