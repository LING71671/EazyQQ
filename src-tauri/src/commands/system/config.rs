use tauri::{command, State};
use crate::commands::AppState;
use crate::models::ApiResponse;

#[command]
pub async fn get_config(state: State<'_, AppState>) -> Result<ApiResponse<serde_json::Value>, String> {
    if let Ok(Some(val)) = state.db.get_setting("app_config") {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&val) {
            return Ok(ApiResponse::ok(parsed));
        }
    }

    let default_config = crate::services::config::default_app_config();
    Ok(ApiResponse::ok(default_config))
}

#[command]
pub async fn update_config(
    state: State<'_, AppState>,
    config: serde_json::Value,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let serialized = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    state.db.set_setting("app_config", &serialized).map_err(|e| e.to_string())?;

    // Hot-apply the AI provider so switching model takes effect immediately
    let fallback_key = state
        .db
        .get_setting("ai_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let ai_cfg =
        crate::services::ai::AiRuntimeConfig::from_app_config(config.get("ai"), &fallback_key);
    state.ai.reconfigure(ai_cfg.clone());

    tracing::info!("config updated; active AI provider -> {}", ai_cfg.describe());
    Ok(ApiResponse::ok(config))
}

#[command]
pub async fn get_qq_path() -> Result<ApiResponse<String>, String> {
    let napcat_dir = crate::services::logging::workspace_root().join("napcat");
    match crate::services::protocol::patch::configured_qq_path(&napcat_dir) {
        Ok(p) => Ok(ApiResponse::ok(p.to_string_lossy().to_string())),
        Err(e) => Ok(ApiResponse::err(1005, e, None)),
    }
}

#[command]
pub async fn set_qq_path(path: String) -> Result<ApiResponse<String>, String> {
    let candidate = std::path::PathBuf::from(path.trim());
    if !candidate.is_file() {
        return Ok(ApiResponse::err(
            1004,
            "指定的 QQ.exe 路径不存在或不是有效文件",
            None,
        ));
    }
    let napcat_dir = crate::services::logging::workspace_root().join("napcat");
    let cfg_dir = napcat_dir.join("config");
    let _ = std::fs::create_dir_all(&cfg_dir);
    let cfg_file = cfg_dir.join("qq_path.txt");
    let path_str = candidate.to_string_lossy().to_string();
    std::fs::write(&cfg_file, &path_str).map_err(|e| format!("写入 qq_path.txt 失败: {}", e))?;

    let patch_pkg = napcat_dir.join("qqnt.json");
    crate::services::protocol::patch::sync_qqnt_patch(&candidate, &patch_pkg);

    Ok(ApiResponse::ok(path_str))
}
