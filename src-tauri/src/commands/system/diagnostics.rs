use tauri::{command, State};
use crate::commands::AppState;
use crate::models::{ApiResponse, DependencyHealthReport, ReadyPath, StorageReady};

/// Full end-to-end link report for the frontend.
#[command]
pub async fn get_chain_status() -> Result<ApiResponse<serde_json::Value>, String> {
    crate::services::chain::record_ok(crate::services::chain::Link::Frontend, "界面交互连接正常");
    let links = crate::services::chain::snapshot();
    let first = crate::services::chain::first_break();

    Ok(ApiResponse::ok(serde_json::json!({
        "links": links,
        "firstBreak": first,
        "hasFailure": crate::services::chain::has_failure(),
        "uptimeSecs": crate::services::chain::uptime_secs(),
    })))
}

/// Dynamically probe for opencode binary from PATH or standard Node global installation.
fn detect_opencode_binary() -> Option<String> {
    // 1. Search PATH environment variable
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for ext in ["opencode.cmd", "opencode.exe", "opencode"] {
                let candidate = dir.join(ext);
                if candidate.is_file() {
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }
    }
    // 2. Search standard user profile APPDATA
    if let Ok(app_data) = std::env::var("APPDATA") {
        let p = std::path::PathBuf::from(app_data).join("npm").join("opencode.cmd");
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let p = std::path::PathBuf::from(local_app_data).join("npm").join("opencode.cmd");
        if p.is_file() {
            return Some(p.to_string_lossy().to_string());
        }
    }
    None
}

#[command]
pub async fn check_dependencies(state: State<'_, AppState>) -> Result<ApiResponse<DependencyHealthReport>, String> {
    // 1. QQNT client: probe dynamically via configured_qq_path
    let napcat_dir = crate::services::logging::workspace_root().join("napcat");
    let (qq_ready, qq_path) = match crate::services::protocol::patch::configured_qq_path(&napcat_dir) {
        Ok(p) => (true, p.to_string_lossy().to_string()),
        Err(e) => (false, e),
    };

    // 2. Local AI runtime: dynamic lookup without hardcoded paths
    let has_key = state
        .db
        .get_setting("tokenrhythm_api_key")
        .ok()
        .flatten()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("TOKENRHYTHM_API_KEY").is_ok();

    let opencode_found = detect_opencode_binary();
    let (ai_ready, ai_path) = match opencode_found {
        Some(p) => (true, p),
        None if has_key => (true, format!("云端模型 (API Key 已配置, {})", state.ai.model())),
        None => (false, "未检测到本地 OpenCode，且未配置 API Key".to_string()),
    };

    // 3. Storage: check writable status and free disk space
    let data_dir = crate::services::logging::data_dir();
    let writable = crate::services::diagnostics::is_dir_writable(&data_dir);
    let free_mb = crate::services::diagnostics::free_space_mb(&data_dir).unwrap_or(0);

    let napcat_ready = state.napcat.is_alive().await;
    let onebot_ready = state.onebot.get_login_info().await.is_ok();

    let is_all_ready = qq_ready && ai_ready && writable && napcat_ready && onebot_ready;

    tracing::info!(
        "health: qqnt={} ai={} storage={} napcat={} onebot={} => allReady={}",
        qq_ready,
        ai_ready,
        writable,
        napcat_ready,
        onebot_ready,
        is_all_ready
    );

    Ok(ApiResponse::ok(DependencyHealthReport {
        is_all_ready,
        qq_nt: ReadyPath {
            ready: qq_ready,
            path: qq_path,
        },
        open_code: ReadyPath {
            ready: ai_ready,
            path: ai_path,
        },
        storage: StorageReady {
            is_writable: writable,
            free_space_mb: free_mb,
        },
    }))
}

#[command]
pub async fn export_diagnostics_bundle(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let result = crate::services::diagnostics::export_bundle(
        &state.db,
        &state.ai,
        &state.napcat,
        &state.onebot,
    )
    .await?;

    Ok(ApiResponse::ok(serde_json::json!({
        "zipFilePath": result.zip_path.display().to_string(),
        "entries": result.entries,
        "bytes": result.bytes,
    })))
}
