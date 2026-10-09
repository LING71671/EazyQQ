use crate::commands::AppState;
use crate::models::{ApiResponse, DependencyHealthReport, ReadyPath, StorageReady};
use tauri::{command, State};

/// Full end-to-end link report for the frontend.
#[command]
pub async fn get_chain_status(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    crate::services::protocol::health::refresh(&state.napcat, &state.onebot).await;
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

#[command]
pub async fn check_dependencies(
    state: State<'_, AppState>,
) -> Result<ApiResponse<DependencyHealthReport>, String> {
    // 1. QQNT client: probe dynamically via configured_qq_path
    let napcat_dir = crate::services::napcat_boot::locate_napcat_dir();
    let (qq_ready, qq_path) =
        match crate::services::protocol::patch::configured_qq_path(&napcat_dir) {
            Ok(p) => (true, p.to_string_lossy().to_string()),
            Err(e) => (false, e),
        };

    // 2. Local AI runtime: dynamic lookup without hardcoded paths
    let has_key = !state.ai.current().api_key.trim().is_empty()
        || state
            .db
            .get_setting("ai_api_key")
            .ok()
            .flatten()
            .map(|k| !k.trim().is_empty())
            .unwrap_or(false)
        || std::env::var("LLM_API_KEY").is_ok()
        || std::env::var("OPENAI_API_KEY").is_ok()
        || std::env::var("AI_API_KEY").is_ok();

    let ai_cfg = state.ai.current();
    let has_valid_key = !ai_cfg.api_key.trim().is_empty()
        || crate::services::ai::detect_api_key().is_some()
        || has_key;
    let is_local_llm = crate::services::ai::is_local_endpoint(&ai_cfg.base_url);

    let (ai_ready, ai_path) = if ai_cfg.uses_opencode_runtime() {
        match crate::services::ai::opencode::binary() {
            Some(path) => (
                true,
                format!(
                    "OpenCode runtime: {} (inference not yet tested)",
                    path.display()
                ),
            ),
            None => (
                false,
                "OpenCode native runtime was not found on PATH".into(),
            ),
        }
    } else {
        (
            is_local_llm || has_valid_key,
            format!("Configured endpoint: {}", ai_cfg.base_url),
        )
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

#[command]
pub async fn repair_chain(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiResponse<crate::services::protocol::health::RepairReport>, String> {
    let report = crate::services::protocol::health::repair(&state.napcat, &state.onebot).await;
    if report.ok { crate::commands::protocol::auth::reload_relocated_runtime(app, &state); }
    Ok(ApiResponse::ok(report))
}
