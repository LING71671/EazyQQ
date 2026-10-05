use tauri::{command, State};
use crate::commands::AppState;
use crate::models::ApiResponse;

#[command]
pub async fn test_ai_connection(
    state: State<'_, AppState>,
    _provider: String,
    _model_id: Option<String>,
) -> Result<ApiResponse<serde_json::Value>, String> {
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
