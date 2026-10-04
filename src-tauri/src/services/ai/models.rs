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

    let resp = req.send().await.map_err(|e| format!("请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("接口返回错误码: {}", resp.status()));
    }

    let json: Value = resp.json().await.map_err(|e| format!("解析响应失败: {e}"))?;
    let mut model_ids = Vec::new();

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

    if model_ids.is_empty() {
        return Err("未在响应中解析到任何模型".to_string());
    }

    Ok(model_ids)
}
