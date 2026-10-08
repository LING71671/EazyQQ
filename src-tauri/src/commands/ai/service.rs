use crate::commands::AppState;
use crate::models::ApiResponse;
use tauri::{command, State};

#[command]
pub async fn test_ai_connection(
    state: State<'_, AppState>,
    provider: Option<String>,
    model_id: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    prompt: Option<String>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let mut cfg = state.ai.current();
    if let Some(p) = provider.filter(|s| !s.trim().is_empty()) {
        cfg.provider = p;
    }
    if let Some(m) = model_id.filter(|s| !s.trim().is_empty()) {
        cfg.model = m;
    }
    if let Some(u) = base_url.filter(|s| !s.trim().is_empty()) {
        cfg.base_url = u;
    }
    if let Some(k) = api_key.filter(|s| !s.trim().is_empty()) {
        cfg.api_key = k;
    } else if cfg.provider == "opencode" || cfg.provider == "opencode-go" {
        cfg.api_key = crate::services::ai::detect_opencode_auth_key().unwrap_or_default();
    }
    tracing::info!("AI connectivity test against {}", cfg.describe());

    let test_prompt = prompt
        .filter(|p| !p.trim().is_empty())
        .unwrap_or_else(|| "你好".to_string());
    let start = std::time::Instant::now();
    match state.ai.test_connection(&cfg, &test_prompt).await {
        Ok((reply, reasoning)) => {
            let latency = start.elapsed().as_millis();
            crate::services::chain::record_ok(
                crate::services::chain::Link::AiProvider,
                format!("{} inference verified", cfg.model),
            );
            Ok(ApiResponse::ok(serde_json::json!({
                "isSuccess": true,
                "latencyMs": latency,
                "provider": cfg.provider,
                "model": cfg.model,
                "endpoint": cfg.base_url,
                "prompt": test_prompt,
                "reply": reply,
                "reasoning": reasoning,
            })))
        }
        Err(e) => {
            crate::services::chain::record_error(crate::services::chain::Link::AiProvider, &e);
            Ok(ApiResponse::err(
                1003,
                format!("大模型连通测试失败 [{}]: {}", cfg.describe(), e),
                None,
            ))
        }
    }
}

#[command]
pub async fn fetch_provider_models(
    provider: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<ApiResponse<Vec<crate::services::ai::ModelInfoDto>>, String> {
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

    match crate::services::ai::fetch_models_with_metadata(&provider, &target_url, key.as_deref())
        .await
    {
        Ok(models) => Ok(ApiResponse::ok(models)),
        Err(e) => Ok(ApiResponse::err(1003, format!("获取模型失败: {}", e), None)),
    }
}
