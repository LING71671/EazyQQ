use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::auth::detect_api_key;

pub const PRESET_OPENCODE: &str = "https://opencode.ai/zen/v1";
pub const DEFAULT_MODEL_OPENCODE: &str = "qwen3.8-flash";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRuntimeConfig {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_context_messages: usize,
}

impl Default for AiRuntimeConfig {
    fn default() -> Self {
        Self {
            provider: "opencode".to_string(),
            base_url: PRESET_OPENCODE.to_string(),
            api_key: String::new(),
            model: DEFAULT_MODEL_OPENCODE.to_string(),
            temperature: 0.7,
            max_context_messages: 10,
        }
    }
}

/// Is this endpoint local? Local endpoints do not need an API key.
pub fn is_local_endpoint(base_url: &str) -> bool {
    let lower = base_url.to_lowercase();
    lower.contains("127.0.0.1")
        || lower.contains("localhost")
        || lower.contains("0.0.0.0")
        || lower.contains("[::1]")
        || lower.contains("host.docker.internal")
}

pub fn preset_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "opencode" | "opencode-go" => Some(PRESET_OPENCODE),
        _ => None,
    }
}

pub fn preset_model(provider: &str) -> Option<&'static str> {
    match provider {
        "opencode" | "opencode-go" => Some(DEFAULT_MODEL_OPENCODE),
        _ => None,
    }
}

pub fn provider_preset(provider: &str) -> (Option<String>, Option<String>) {
    let p = provider.trim().to_lowercase();
    (
        preset_base_url(&p).map(|s| s.to_string()),
        preset_model(&p).map(|s| s.to_string()),
    )
}

pub fn known_providers() -> Vec<(String, String, String, bool)> {
    vec![(
        "opencode".to_string(),
        PRESET_OPENCODE.to_string(),
        DEFAULT_MODEL_OPENCODE.to_string(),
        false,
    )]
}

impl AiRuntimeConfig {
    /// Build a runtime config from the persisted `app_config.ai` block.
    /// Preserves OpenCode integration while omitting third-party commercial advertisements.
    pub fn from_app_config(ai: Option<&Value>, fallback_key: &str) -> Self {
        let mut cfg = AiRuntimeConfig::default();

        let Some(ai) = ai else {
            cfg.api_key = if fallback_key.trim().is_empty() {
                detect_api_key().unwrap_or_default()
            } else {
                fallback_key.to_string()
            };
            return cfg;
        };

        let provider = ai
            .get("activeProvider")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| cfg.provider.clone());

        let (p_base_url, p_model, p_key) = if let Some(p) = ai.get("providers").and_then(|m| m.get(&provider)) {
            (
                p.get("baseUrl").and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                p.get("model").and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                p.get("apiKey").and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
            )
        } else {
            (None, None, None)
        };

        cfg.provider = provider.clone();
        cfg.base_url = ai
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or(p_base_url)
            .or_else(|| preset_base_url(&provider).map(|s| s.to_string()))
            .unwrap_or_default();

        cfg.model = ai
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or(p_model)
            .or_else(|| preset_model(&provider).map(|s| s.to_string()))
            .unwrap_or_default();

        let mut key = ai
            .get("apiKey")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or(p_key)
            .unwrap_or_else(|| fallback_key.to_string());

        if key.is_empty() {
            if let Some(auto_k) = detect_api_key() {
                key = auto_k;
            }
        }
        cfg.api_key = key;

        cfg.temperature = ai
            .get("temperature")
            .and_then(|v| v.as_f64())
            .map(|t| t.clamp(0.0, 2.0) as f32)
            .unwrap_or(cfg.temperature);

        cfg.max_context_messages = ai
            .get("maxContextMessages")
            .and_then(|v| v.as_i64())
            .map(|n| n.clamp(1, 100) as usize)
            .unwrap_or(cfg.max_context_messages);

        cfg
    }

    pub fn requires_api_key(&self) -> bool {
        if self.provider == "opencode" || self.provider == "opencode-go" {
            return detect_api_key().is_none();
        }
        !is_local_endpoint(&self.base_url)
    }

    /// A configuration is only usable with both an endpoint and a model.
    pub fn validate(&self) -> Result<(), String> {
        if self.base_url.trim().is_empty() {
            return Err("未配置大模型接口地址，请在系统设置中填写 baseUrl（兼容 OpenAI 规范）".to_string());
        }
        if self.model.trim().is_empty() {
            return Err("未配置模型名称，请在系统设置中填写 model".to_string());
        }
        Ok(())
    }

    pub fn describe(&self) -> String {
        format!(
            "provider={}, model={}, endpoint={}{}",
            self.provider,
            if self.model.is_empty() { "<未设置>" } else { &self.model },
            if self.base_url.is_empty() { "<未设置>" } else { &self.base_url },
            if self.requires_api_key() { "" } else { " (无需手动填写 Key)" }
        )
    }
}
