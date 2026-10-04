use std::time::Duration;
use reqwest::Client;
use serde_json::Value;

use super::config::is_local_endpoint;

/// Dynamically fetch supported models from an OpenAI-compatible `/models` endpoint.
pub async fn fetch_models_from_endpoint(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>, String> {
    let client = if is_local_endpoint(base_url) {
        Client::builder()
            .timeout(Duration::from_secs(15))
            .no_proxy()
            .build()
            .unwrap_or_default()
    } else {
        Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default()
    };

    let base = base_url.trim_end_matches('/');
    let url = if base.ends_with("/models") {
        base.to_string()
    } else {
        format!("{}/models", base)
    };

    let mut req = client.get(&url);
    if let Some(key) = api_key.filter(|k| !k.trim().is_empty()) {
        req = req.header("Authorization", format!("Bearer {}", key.trim()));
    }

    let resp_res = req.send().await;
    let mut model_ids = Vec::new();

    if let Ok(resp) = resp_res {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    for item in data {
                        if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                            model_ids.push(id.to_string());
                        }
                    }
                } else if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                    for item in models {
                        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                            model_ids.push(name.to_string());
                        }
                    }
                }
            }
        }
    }

    // Graceful fallback for OpenCode / Zen endpoint if offline or blocked
    if model_ids.is_empty() && base_url.to_lowercase().contains("opencode") {
        model_ids = vec![
            "qwen3.8-flash".to_string(),
            "glm-4-flash".to_string(),
            "deepseek-chat".to_string(),
            "gpt-4o-mini".to_string(),
        ];
    }

    if model_ids.is_empty() {
        return Err("未从端点解析到可用模型，请检查网络或地址".to_string());
    }

    // Deduplicate
    model_ids.sort();
    model_ids.dedup();

    // Prioritize free / flash / zen models at the top for beginners
    model_ids.sort_by(|a, b| {
        let a_low = a.to_lowercase();
        let b_low = b.to_lowercase();
        let a_free = a_low.contains("free") || a_low.contains("flash") || a_low.contains("zen");
        let b_free = b_low.contains("free") || b_low.contains("flash") || b_low.contains("zen");
        match (a_free, b_free) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.cmp(b),
        }
    });

    Ok(model_ids)
}
