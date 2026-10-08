use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

use super::config::is_local_endpoint;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfoDto {
    pub id: String,
    pub name: String,
    pub is_free: bool,
    pub cost_input: Option<f64>,
    pub cost_output: Option<f64>,
}

/// Dynamically fetch or load the OpenCode models metadata registry.
/// Checks local cache `~/.cache/opencode/models.json` first, falls back to `https://models.opencode.ai/api.json`.
async fn load_opencode_models_registry() -> Option<Value> {
    // 1. Try local cache ~/.cache/opencode/models.json
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok();

    if let Some(ref h) = home {
        let cache_path = std::path::PathBuf::from(h)
            .join(".cache")
            .join("opencode")
            .join("models.json");
        if cache_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&cache_path) {
                if let Ok(json) = serde_json::from_str::<Value>(&content) {
                    return Some(json);
                }
            }
        }
    }

    // 2. Fetch from upstream https://models.opencode.ai/api.json with a strict timeout
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .ok()?;
    if let Ok(resp) = client
        .get("https://models.opencode.ai/api.json")
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<Value>().await {
                // Try writing back to local cache for subsequent instant queries
                if let Some(ref h) = home {
                    let cache_dir = std::path::PathBuf::from(h).join(".cache").join("opencode");
                    let _ = std::fs::create_dir_all(&cache_dir);
                    let _ = std::fs::write(
                        cache_dir.join("models.json"),
                        serde_json::to_string(&json).unwrap_or_default(),
                    );
                }
                return Some(json);
            }
        }
    }

    None
}

/// Dynamically fetch supported models with pricing metadata from an OpenAI-compatible endpoint or provider registry.
pub async fn fetch_models_with_metadata(
    provider: &str,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<ModelInfoDto>, String> {
    let is_local = is_local_endpoint(base_url);

    // 1. If provider is OpenCode or base_url targets opencode.ai, leverage dynamic models.json metadata
    let is_opencode = provider == "opencode"
        || provider == "opencode-go"
        || base_url.to_lowercase().contains("opencode.ai");

    let registry = if is_opencode {
        load_opencode_models_registry().await
    } else {
        None
    };

    let mut result_models = Vec::new();

    if is_opencode {
        if let Some(reg) = registry {
            let provider_data = reg
                .get("opencode")
                .or_else(|| reg.get("opencode-go"))
                .or_else(|| reg.get("zenifra"));

            if let Some(p_obj) = provider_data {
                if let Some(models_map) = p_obj.get("models").and_then(|m| m.as_object()) {
                    for (m_key, m_val) in models_map {
                        let id = m_val
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or(m_key)
                            .to_string();
                        let name = m_val
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&id)
                            .to_string();

                        let cost_obj = m_val.get("cost");
                        let cost_input = cost_obj
                            .and_then(|c| c.get("input"))
                            .and_then(|v| v.as_f64());
                        let cost_output = cost_obj
                            .and_then(|c| c.get("output"))
                            .and_then(|v| v.as_f64());

                        let is_free = cost_input == Some(0.0) && cost_output == Some(0.0);

                        result_models.push(ModelInfoDto {
                            id,
                            name,
                            is_free,
                            cost_input,
                            cost_output,
                        });
                    }
                }
            }
        }
    }

    // 2. If models were not loaded from registry (or for any generic provider), query the endpoint directly
    if result_models.is_empty() {
        let client = if is_local {
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

        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<Value>().await {
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        for item in data {
                            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                                let lower = id.to_lowercase();
                                let is_free = is_local
                                    || lower.contains("-free")
                                    || lower.contains(":free")
                                    || lower.contains("_free");
                                result_models.push(ModelInfoDto {
                                    id: id.to_string(),
                                    name: id.to_string(),
                                    is_free,
                                    cost_input: if is_local || is_free { Some(0.0) } else { None },
                                    cost_output: if is_local || is_free { Some(0.0) } else { None },
                                });
                            }
                        }
                    } else if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                        for item in models {
                            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                                let lower = name.to_lowercase();
                                let is_free = is_local
                                    || lower.contains("-free")
                                    || lower.contains(":free")
                                    || lower.contains("_free");
                                result_models.push(ModelInfoDto {
                                    id: name.to_string(),
                                    name: name.to_string(),
                                    is_free,
                                    cost_input: if is_local || is_free { Some(0.0) } else { None },
                                    cost_output: if is_local || is_free { Some(0.0) } else { None },
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    if result_models.is_empty() {
        return Err("未从端点或元数据中心解析到可用模型，请检查网络或地址".to_string());
    }

    if is_opencode {
        let available = super::opencode::models().await?;
        result_models.retain(|m| available.contains(&m.id));
        for id in available {
            if !result_models.iter().any(|m| m.id == id) {
                result_models.push(ModelInfoDto {
                    name: id.clone(),
                    is_free: false,
                    id,
                    cost_input: None,
                    cost_output: None,
                });
            }
        }
    }

    // Deduplicate by model ID
    result_models.sort_by(|a, b| a.id.cmp(&b.id));
    result_models.dedup_by(|a, b| a.id == b.id);

    // Prioritize free models at the top, then alphabetically
    result_models.sort_by(|a, b| match (a.is_free, b.is_free) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(result_models)
}
