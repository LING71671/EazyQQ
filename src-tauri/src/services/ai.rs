//! AI provider access.
//!
//! Every call goes through an OpenAI-compatible `/chat/completions` endpoint, so the
//! provider is just a base URL + model + key. The configuration is held behind a lock so
//! changing it in the settings UI takes effect immediately, without restarting the app.
//!
//! Before this, `app_config.ai` was purely decorative: `lib.rs` always constructed the
//! service against a hardcoded TokenRhythm endpoint and model, so selecting "local
//! OpenCode" or "DeepSeek" in the UI changed nothing - and the user could not tell that
//! their traffic was still going to the cloud.

use std::sync::RwLock;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info};

/// Default endpoints per provider. Any explicit `baseUrl` in the config overrides these.
///
/// Only genuinely OpenAI-compatible endpoints belong here. OpenCode was originally
/// assumed to expose one; probing a live `opencode serve` shows `/v1/*` and `/api/*`
/// fall through to its HTML web UI, so it is an agent framework rather than an inference
/// server and has deliberately been removed from this list. Local inference should go
/// through Ollama / LM Studio / llama.cpp / vLLM, all of which do implement the protocol.
const PRESET_OLLAMA: &str = "http://127.0.0.1:11434/v1";
const PRESET_LM_STUDIO: &str = "http://127.0.0.1:1234/v1";
const PRESET_LLAMACPP: &str = "http://127.0.0.1:8080/v1";
const PRESET_VLLM: &str = "http://127.0.0.1:8000/v1";
const PRESET_DEEPSEEK: &str = "https://api.deepseek.com/v1";
const PRESET_OPENAI: &str = "https://api.openai.com/v1";
const PRESET_TOKENRHYTHM: &str = "https://tokenrhythm.studio/v1";

const DEFAULT_MODEL_OLLAMA: &str = "qwen2.5:7b";
const DEFAULT_MODEL_LM_STUDIO: &str = "local-model";
const DEFAULT_MODEL_DEEPSEEK: &str = "deepseek-chat";
const DEFAULT_MODEL_OPENAI: &str = "gpt-4o-mini";
const DEFAULT_MODEL_TOKENRHYTHM: &str = "qwen3.8-flash";

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
            provider: "tokenrhythm".to_string(),
            base_url: PRESET_TOKENRHYTHM.to_string(),
            api_key: String::new(),
            model: DEFAULT_MODEL_TOKENRHYTHM.to_string(),
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

/// Preset endpoint for a provider, or `None` when the provider is a custom one whose
/// endpoint the user must supply.
fn preset_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "ollama" => Some(PRESET_OLLAMA),
        "lmstudio" | "lm-studio" => Some(PRESET_LM_STUDIO),
        "llamacpp" | "llama.cpp" => Some(PRESET_LLAMACPP),
        "vllm" => Some(PRESET_VLLM),
        "deepseek" => Some(PRESET_DEEPSEEK),
        "openai" => Some(PRESET_OPENAI),
        "tokenrhythm" => Some(PRESET_TOKENRHYTHM),
        _ => None,
    }
}

fn preset_model(provider: &str) -> Option<&'static str> {
    match provider {
        "ollama" => Some(DEFAULT_MODEL_OLLAMA),
        "lmstudio" | "lm-studio" => Some(DEFAULT_MODEL_LM_STUDIO),
        "llamacpp" | "llama.cpp" | "vllm" => Some(DEFAULT_MODEL_LM_STUDIO),
        "deepseek" => Some(DEFAULT_MODEL_DEEPSEEK),
        "openai" => Some(DEFAULT_MODEL_OPENAI),
        "tokenrhythm" => Some(DEFAULT_MODEL_TOKENRHYTHM),
        _ => None,
    }
}

/// Public view of a provider's defaults. `None` values mean "the user must supply it".
///
/// Callers that change the provider **must** use this to refresh the stored `baseUrl`
/// and `model`, otherwise a previously saved explicit `baseUrl` keeps winning and the
/// switch silently does nothing - the user picks a local endpoint and traffic still goes
/// to the cloud.
pub fn provider_preset(provider: &str) -> (Option<String>, Option<String>) {
    let p = provider.trim().to_lowercase();
    (
        preset_base_url(&p).map(|s| s.to_string()),
        preset_model(&p).map(|s| s.to_string()),
    )
}

/// Every provider with a built-in default, as
/// `(id, base_url, model, is_local)`.
pub fn known_providers() -> Vec<(String, String, String, bool)> {
    [
        "ollama",
        "lmstudio",
        "llamacpp",
        "vllm",
        "deepseek",
        "openai",
        "tokenrhythm",
    ]
    .iter()
    .filter_map(|p| {
        let url = preset_base_url(p)?;
        let model = preset_model(p)?;
        let local = is_local_endpoint(url);
        Some((p.to_string(), url.to_string(), model.to_string(), local))
    })
    .collect()
}

impl AiRuntimeConfig {
    /// Build a runtime config from the persisted `app_config.ai` block.
    ///
    /// Explicit `baseUrl` / `model` always win; otherwise the provider preset is used.
    /// An empty `provider` falls back to TokenRhythm so an incomplete config still works.
    pub fn from_app_config(ai: Option<&Value>, fallback_key: &str) -> Self {
        let mut cfg = AiRuntimeConfig::default();

        let Some(ai) = ai else {
            cfg.api_key = fallback_key.to_string();
            return cfg;
        };

        let provider = ai
            .get("activeProvider")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| cfg.provider.clone());

        cfg.provider = provider.clone();
        cfg.base_url = ai
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| preset_base_url(&provider).map(|s| s.to_string()))
            .unwrap_or_default();

        cfg.model = ai
            .get("model")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| preset_model(&provider).map(|s| s.to_string()))
            .unwrap_or_default();

        cfg.api_key = ai
            .get("apiKey")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| fallback_key.to_string());

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
        !is_local_endpoint(&self.base_url)
    }

    /// A configuration is only usable with both an endpoint and a model. Custom
    /// providers have no preset, so a missing value must be reported, not silently
    /// turned into a request to nowhere.
    pub fn validate(&self) -> Result<(), String> {
        if self.base_url.trim().is_empty() {
            return Err(format!(
                "供应商「{}」没有内置默认接口地址，请在配置中显式填写 baseUrl",
                self.provider
            ));
        }
        if self.model.trim().is_empty() {
            return Err(format!(
                "供应商「{}」没有内置默认模型，请在配置中显式填写 model",
                self.provider
            ));
        }
        Ok(())
    }

    pub fn describe(&self) -> String {
        format!(
            "provider={}, model={}, endpoint={}{}",
            self.provider,
            if self.model.is_empty() { "<未设置>" } else { &self.model },
            if self.base_url.is_empty() { "<未设置>" } else { &self.base_url },
            if self.requires_api_key() { "" } else { " (本地端点，无需 Key)" }
        )
    }
}

pub struct AiService {
    /// Honours the environment proxy - used for cloud providers.
    client_proxied: Client,
    /// Ignores the environment proxy - used for local endpoints.
    ///
    /// Without this, a machine with `http_proxy` set (a very common setup, and this one
    /// has it) routes `http://127.0.0.1:11434` through the proxy too, which either fails
    /// outright or adds a pointless hop to a loopback request.
    client_direct: Client,
    config: RwLock<AiRuntimeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl AiService {
    pub fn new(config: AiRuntimeConfig) -> Self {
        let client_proxied = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .unwrap_or_default();
        let client_direct = Client::builder()
            .timeout(Duration::from_secs(120))
            .no_proxy()
            .build()
            .unwrap_or_default();

        info!("AI service configured: {}", config.describe());

        Self {
            client_proxied,
            client_direct,
            config: RwLock::new(config),
        }
    }

    /// The client appropriate for the given endpoint.
    fn client_for(&self, cfg: &AiRuntimeConfig) -> &Client {
        if is_local_endpoint(&cfg.base_url) {
            &self.client_direct
        } else {
            &self.client_proxied
        }
    }

    /// Apply a new configuration in place. Takes effect on the next call.
    pub fn reconfigure(&self, config: AiRuntimeConfig) {
        info!("AI service reconfigured: {}", config.describe());
        if let Ok(mut guard) = self.config.write() {
            *guard = config;
        } else {
            error!("AI config lock poisoned; keeping the previous configuration");
        }
    }

    pub fn current(&self) -> AiRuntimeConfig {
        self.config
            .read()
            .map(|c| c.clone())
            .unwrap_or_default()
    }

    pub fn model(&self) -> String {
        self.current().model
    }

    pub fn has_api_key(&self) -> bool {
        let cfg = self.current();
        !cfg.api_key.trim().is_empty() || !cfg.requires_api_key()
    }

    /// Single place that talks to the OpenAI-compatible endpoint.
    ///
    /// Returns `(content, reasoning_content)`.
    async fn post_chat(&self, mut payload: Value) -> Result<(String, Option<String>), String> {
        let cfg = self.current();
        cfg.validate()?;

        if cfg.requires_api_key() && cfg.api_key.trim().is_empty() {
            return Err(format!(
                "未配置大模型 API Key（provider={}, endpoint={}）。\
                 可在「系统设置 → 大模型推理供应源」中填写，或改用本地端点（如 OpenCode / Ollama）。",
                cfg.provider, cfg.base_url
            ));
        }

        // Respect the configured temperature unless the caller overrode it.
        if payload.get("temperature").is_none() {
            payload["temperature"] = serde_json::json!(cfg.temperature);
        }
        payload["model"] = serde_json::json!(cfg.model);

        let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));

        let mut request = self
            .client_for(&cfg)
            .post(&url)
            .header("Content-Type", "application/json");

        // Local servers frequently reject a stray Authorization header.
        if !cfg.api_key.trim().is_empty() {
            request = request.header("Authorization", format!("Bearer {}", cfg.api_key));
        }

        let resp = request
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("大模型网络请求失败 ({}): {}", url, e))?;

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            error!("LLM error {} from {}: {}", status, url, err_body);
            return Err(format!(
                "大模型接口返回错误 ({} @ {}): {}",
                status,
                cfg.provider,
                err_body
            ));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("解析大模型响应失败: {}", e))?;

        let message = body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .ok_or_else(|| format!("大模型未返回有效的回复内容: {}", body))?;

        let content = message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();

        let reasoning = message
            .get("reasoning_content")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if content.is_empty() && reasoning.is_none() {
            return Err("大模型返回了空内容".to_string());
        }

        Ok((content, reasoning))
    }

    /// Generate an AI reply given the incoming message and context.
    pub async fn generate_reply(
        &self,
        history: &[ChatMessage],
        incoming: &str,
        target_name: &str,
    ) -> Result<(String, Option<String>), String> {
        let system_prompt = format!(
            "你是一个亲切、高效、得体的个人 QQ 专属智能助理。你正在协助主人回复好友「{}」的消息。\
             请根据上下文用自然口吻拟写回复，不要使用机械人说辞，语言简练生动、得体自然。",
            target_name
        );

        let mut messages = vec![serde_json::json!({
            "role": "system",
            "content": system_prompt
        })];

        for msg in history {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        messages.push(serde_json::json!({
            "role": "user",
            "content": incoming
        }));

        info!(
            "Calling LLM ({}) for target {}",
            self.model(),
            target_name
        );

        self.post_chat(serde_json::json!({
            "messages": messages,
            "stream": false
        }))
        .await
    }

    /// Generate a completion with an explicit system prompt.
    ///
    /// Used by the summarizer and the document extractor, which need strict structured
    /// output and therefore cannot rely on the conversational system prompt baked into
    /// `generate_reply`.
    pub async fn generate_with_system(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        temperature: f32,
    ) -> Result<(String, Option<String>), String> {
        let mut payload_messages = vec![serde_json::json!({
            "role": "system",
            "content": system_prompt
        })];

        for msg in messages {
            payload_messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        info!("Calling LLM ({}) with custom system prompt", self.model());

        self.post_chat(serde_json::json!({
            "messages": payload_messages,
            "temperature": temperature,
            "stream": false
        }))
        .await
    }

    /// Regenerate a reply with a human refinement instruction.
    pub async fn regenerate_with_instruction(
        &self,
        incoming: &str,
        previous_reply: &str,
        instruction: &str,
    ) -> Result<(String, Option<String>), String> {
        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": "你是一个亲切的个人 QQ 专属智能助理。用户正在对上一版的拟回复进行微调指导，请根据指令重新构思一条更贴切的回复。"
            }),
            serde_json::json!({
                "role": "user",
                "content": format!("对方发来的消息是：\"{}\"\n上一版拟答是：\"{}\"\n我的微调指令要求是：\"{}\"\n请重新生成一条回复，直接输出回复内容。", incoming, previous_reply, instruction)
            }),
        ];

        self.post_chat(serde_json::json!({
            "messages": messages,
            "stream": false
        }))
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_config_points_at_tokenrhythm() {
        let cfg = AiRuntimeConfig::default();
        assert_eq!(cfg.provider, "tokenrhythm");
        assert_eq!(cfg.base_url, PRESET_TOKENRHYTHM);
        assert!(cfg.requires_api_key());
    }

    #[test]
    fn provider_preset_is_used_when_no_base_url_is_given() {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({ "activeProvider": "deepseek" })),
            "",
        );
        assert_eq!(cfg.provider, "deepseek");
        assert_eq!(cfg.base_url, PRESET_DEEPSEEK);
        assert_eq!(cfg.model, DEFAULT_MODEL_DEEPSEEK);
    }

    #[test]
    fn explicit_base_url_and_model_override_the_preset() {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({
                "activeProvider": "ollama",
                "baseUrl": "http://127.0.0.1:9999/v1",
                "model": "my-local-model"
            })),
            "",
        );
        assert_eq!(cfg.base_url, "http://127.0.0.1:9999/v1");
        assert_eq!(cfg.model, "my-local-model");
    }

    #[test]
    fn local_endpoints_do_not_require_a_key() {
        for url in [
            "http://127.0.0.1:4096/v1",
            "http://localhost:11434/v1",
            "http://0.0.0.0:1234/v1",
        ] {
            let cfg = AiRuntimeConfig {
                base_url: url.to_string(),
                api_key: String::new(),
                ..AiRuntimeConfig::default()
            };
            assert!(!cfg.requires_api_key(), "{url} should not need a key");

            let service = AiService::new(cfg);
            assert!(service.has_api_key(), "a local endpoint must be usable");
        }
    }

    #[test]
    fn remote_endpoints_do_require_a_key() {
        let cfg = AiRuntimeConfig {
            base_url: PRESET_DEEPSEEK.to_string(),
            api_key: String::new(),
            ..AiRuntimeConfig::default()
        };
        assert!(cfg.requires_api_key());
        assert!(!AiService::new(cfg).has_api_key());
    }

    #[test]
    fn fallback_key_is_used_when_the_config_has_none() {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({ "activeProvider": "openai", "apiKey": "   " })),
            "sk-from-settings",
        );
        assert_eq!(cfg.api_key, "sk-from-settings");
    }

    #[test]
    fn config_api_key_wins_over_the_fallback() {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({ "activeProvider": "openai", "apiKey": "sk-explicit" })),
            "sk-from-settings",
        );
        assert_eq!(cfg.api_key, "sk-explicit");
    }

    #[test]
    fn temperature_and_context_are_clamped() {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({ "temperature": 99.0, "maxContextMessages": 100000 })),
            "",
        );
        assert_eq!(cfg.temperature, 2.0);
        assert_eq!(cfg.max_context_messages, 100);

        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({ "temperature": -5.0, "maxContextMessages": 0 })),
            "",
        );
        assert_eq!(cfg.temperature, 0.0);
        assert_eq!(cfg.max_context_messages, 1);
    }

    #[test]
    fn missing_config_falls_back_to_defaults_with_the_fallback_key() {
        let cfg = AiRuntimeConfig::from_app_config(None, "sk-env");
        assert_eq!(cfg.provider, "tokenrhythm");
        assert_eq!(cfg.api_key, "sk-env");
    }

    #[test]
    fn blank_provider_falls_back_to_tokenrhythm() {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({ "activeProvider": "   " })),
            "",
        );
        assert_eq!(cfg.provider, "tokenrhythm");
        assert_eq!(cfg.base_url, PRESET_TOKENRHYTHM);
    }

    #[test]
    fn reconfigure_swaps_the_active_configuration() {
        let service = AiService::new(AiRuntimeConfig::default());
        assert_eq!(service.model(), DEFAULT_MODEL_TOKENRHYTHM);

        service.reconfigure(AiRuntimeConfig {
            provider: "ollama".to_string(),
            base_url: PRESET_OLLAMA.to_string(),
            model: "local-llama".to_string(),
            ..AiRuntimeConfig::default()
        });

        assert_eq!(service.model(), "local-llama");
        assert_eq!(service.current().base_url, PRESET_OLLAMA);
        assert!(service.has_api_key(), "local endpoint needs no key");
    }

    #[test]
    fn describe_marks_local_endpoints() {
        let local = AiRuntimeConfig {
            provider: "ollama".to_string(),
            base_url: PRESET_OLLAMA.to_string(),
            ..AiRuntimeConfig::default()
        };
        assert!(local.describe().contains("无需 Key"));
        assert!(AiRuntimeConfig::default().describe().contains("tokenrhythm"));
    }

    #[test]
    fn opencode_is_not_offered_as_a_provider() {
        // Probing a live `opencode serve` showed /v1/* and /api/* return its HTML web UI,
        // so it is not an OpenAI-compatible endpoint and must not be advertised as one.
        let ids: Vec<String> = known_providers().into_iter().map(|(id, ..)| id).collect();
        assert!(!ids.contains(&"opencode".to_string()), "got {ids:?}");
        assert!(ids.contains(&"ollama".to_string()));
        assert!(ids.contains(&"tokenrhythm".to_string()));
    }

    #[test]
    fn local_providers_are_flagged_in_the_catalogue() {
        for (id, url, model, local) in known_providers() {
            assert!(!url.is_empty(), "{id} must have a preset endpoint");
            assert!(!model.is_empty(), "{id} must have a preset model");
            assert_eq!(local, is_local_endpoint(&url), "mismatch for {id}");
        }
    }

    #[test]
    fn unknown_provider_has_no_preset_and_fails_validation() {
        let (url, model) = provider_preset("my-custom-gateway");
        assert!(url.is_none());
        assert!(model.is_none());

        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({ "activeProvider": "my-custom-gateway" })),
            "",
        );
        let err = cfg.validate().unwrap_err();
        assert!(err.contains("baseUrl"), "got: {err}");
    }

    #[test]
    fn unknown_provider_works_once_an_endpoint_is_supplied() {
        let cfg = AiRuntimeConfig::from_app_config(
            Some(&json!({
                "activeProvider": "my-custom-gateway",
                "baseUrl": "https://gateway.internal/v1",
                "model": "internal-model",
                "apiKey": "sk-x"
            })),
            "",
        );
        assert!(cfg.validate().is_ok());
        assert!(cfg.requires_api_key());
        assert_eq!(cfg.model, "internal-model");
    }
}
