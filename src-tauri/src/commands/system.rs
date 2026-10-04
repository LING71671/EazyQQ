use serde::{Deserialize, Serialize};
use tauri::{command, State};
use crate::commands::AppState;
use crate::models::{ApiResponse, DependencyHealthReport, ReadyPath, StorageReady};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_name: String,
    pub release_notes: String,
    pub html_url: String,
    pub download_url: Option<String>,
    pub published_at: String,
}

/// Full end-to-end link report for the frontend.
///
/// Returns every hop with its health, what breaks if it is down, and - most usefully -
/// the first break in pipeline order, since in a chain everything after the first failure
/// is a consequence rather than a separate fault.
#[command]
pub async fn get_chain_status() -> Result<ApiResponse<serde_json::Value>, String> {
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
pub async fn get_config(state: State<'_, AppState>) -> Result<ApiResponse<serde_json::Value>, String> {
    if let Ok(Some(val)) = state.db.get_setting("app_config") {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&val) {
            return Ok(ApiResponse::ok(parsed));
        }
    }

    // Single source of truth, shared with the audit test in `services::config`.
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

    // Hot-apply the AI provider so switching to a local model takes effect immediately
    // instead of requiring an app restart.
    let fallback_key = state
        .db
        .get_setting("tokenrhythm_api_key")
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
pub async fn test_ai_connection(state: State<'_, AppState>, _provider: String, _model_id: Option<String>) -> Result<ApiResponse<serde_json::Value>, String> {
    let cfg = state.ai.current();
    tracing::info!("AI connectivity test against {}", cfg.describe());

    let start = std::time::Instant::now();
    match state.ai.generate_reply(&[], "ping", "系统连通性自检").await {
        Ok((reply, _)) => {
            let latency = start.elapsed().as_millis();
            Ok(ApiResponse::ok(serde_json::json!({
                "isSuccess": true,
                "latencyMs": latency,
                "provider": cfg.provider,
                "model": cfg.model,
                "endpoint": cfg.base_url,
                "reply": reply,
            })))
        }
        Err(e) => Ok(ApiResponse::err(
            1003,
            format!("大模型连通测试失败 [{}]: {}", cfg.describe(), e),
            None,
        )),
    }
}

#[command]
pub async fn fetch_provider_models(
    provider: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<ApiResponse<Vec<String>>, String> {
    let (preset_url, _) = crate::services::ai::provider_preset(&provider);
    let target_url = base_url
        .filter(|u| !u.trim().is_empty())
        .or(preset_url)
        .ok_or_else(|| "缺少 base_url".to_string())?;

    let key = if let Some(k) = api_key.filter(|s| !s.trim().is_empty()) {
        Some(k)
    } else if provider == "opencode" || provider == "opencode-go" {
        crate::services::ai::detect_opencode_auth_key()
    } else {
        None
    };

    match crate::services::ai::fetch_models_from_endpoint(&target_url, key.as_deref()).await {
        Ok(models) => Ok(ApiResponse::ok(models)),
        Err(e) => Ok(ApiResponse::err(1003, format!("获取模型失败: {}", e), None)),
    }
}

#[command]
pub async fn check_dependencies(state: State<'_, AppState>) -> Result<ApiResponse<DependencyHealthReport>, String> {
    // Real probes - this used to return hardcoded "everything is fine" values, which
    // made the health panel useless for diagnosing an actual problem.

    // 1. QQNT client: try the paths Tencent uses plus whatever NapCat was configured with.
    let qq_candidates = [
        r"C:\Program Files\Tencent\QQNT\QQ.exe",
        r"C:\Program Files (x86)\Tencent\QQNT\QQ.exe",
        r"D:\Program Files\Tencent\QQNT\QQ.exe",
    ];
    let (qq_ready, qq_path) = qq_candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| (true, p.to_string()))
        .unwrap_or_else(|| (false, qq_candidates[0].to_string()));

    // 2. Local AI runtime: a configured key or a local opencode binary both count.
    let has_key = state
        .db
        .get_setting("tokenrhythm_api_key")
        .ok()
        .flatten()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("TOKENRHYTHM_API_KEY").is_ok();
    let opencode_candidates = [
        r"A:\DevEnv\SDKs\Node\npm-global\opencode.cmd",
        r"C:\Users\www17\AppData\Roaming\npm\opencode.cmd",
    ];
    let opencode_found = opencode_candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists());
    let (ai_ready, ai_path) = match opencode_found {
        Some(p) => (true, p.to_string()),
        None if has_key => (true, format!("云端模型 (API Key 已配置, {})", state.ai.model())),
        None => (false, "未检测到本地 OpenCode，且未配置 API Key".to_string()),
    };

    // 3. Storage: really write a probe file, and report actual free space.
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
pub async fn check_app_update() -> Result<ApiResponse<AppUpdateInfo>, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client = reqwest::Client::builder()
        .user_agent("EazyQQ-Updater")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    // Use /releases (not /releases/latest) because all releases are currently marked as
    // prerelease, and GitHub's /releases/latest endpoint silently skips prereleases.
    let url = "https://api.github.com/repos/LING71671/EazyQQ/releases?per_page=5";
    let resp = client.get(url).send().await
        .map_err(|e| format!("检查更新网络请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Ok(ApiResponse::ok(AppUpdateInfo {
            current_version: current_version.clone(),
            latest_version: current_version,
            has_update: false,
            release_name: "暂无线上发布版本".to_string(),
            release_notes: String::new(),
            html_url: "https://github.com/LING71671/EazyQQ/releases".to_string(),
            download_url: None,
            published_at: String::new(),
        }));
    }

    let releases: serde_json::Value = resp.json().await
        .map_err(|e| format!("解析发布数据失败: {}", e))?;

    // Pick the first non-draft release (prereleases are included).
    let json = releases
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|r| {
                r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false) == false
            })
        });

    let json = match json {
        Some(j) => j,
        None => {
            return Ok(ApiResponse::ok(AppUpdateInfo {
                current_version: current_version.clone(),
                latest_version: current_version,
                has_update: false,
                release_name: "暂无线上发布版本".to_string(),
                release_notes: String::new(),
                html_url: "https://github.com/LING71671/EazyQQ/releases".to_string(),
                download_url: None,
                published_at: String::new(),
            }));
        }
    };

    let tag_name = json.get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();

    // Strip prerelease suffixes (e.g. "0.1.1-beta" -> "0.1.1") for semver comparison.
    let tag_base = tag_name.split('-').next().unwrap_or(&tag_name).to_string();

    let release_name = json.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let release_notes = json.get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let html_url = json.get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://github.com/LING71671/EazyQQ/releases")
        .to_string();

    let published_at = json.get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let download_url = json.get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.ends_with(".exe") || name.ends_with(".msi") || name.ends_with(".zip") {
                    item.get("browser_download_url").and_then(|u| u.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    let has_update = !tag_base.is_empty() && tag_base != current_version;

    Ok(ApiResponse::ok(AppUpdateInfo {
        current_version,
        latest_version: if tag_name.is_empty() { env!("CARGO_PKG_VERSION").to_string() } else { tag_name },
        has_update,
        release_name,
        release_notes,
        html_url,
        download_url,
        published_at,
    }))
}
