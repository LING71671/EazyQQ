use std::sync::RwLock;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{error, info};

use super::config::{is_local_endpoint, AiRuntimeConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct AiService {
    /// Honours the environment proxy - used for cloud providers.
    client_proxied: Client,
    /// Ignores the environment proxy - used for local endpoints.
    client_direct: Client,
    config: RwLock<AiRuntimeConfig>,
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

    /// Stream chat response token by token via Server-Sent Events.
    pub async fn stream_chat<F>(
        &self,
        mut payload: Value,
        mut on_chunk: F,
    ) -> Result<(String, Option<String>), String>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let cfg = self.current();
        cfg.validate()?;

        if cfg.requires_api_key() && cfg.api_key.trim().is_empty() {
            return Err(format!(
                "未配置大模型 API Key（provider={}, endpoint={}）。",
                cfg.provider, cfg.base_url
            ));
        }

        if payload.get("temperature").is_none() {
            payload["temperature"] = serde_json::json!(cfg.temperature);
        }
        payload["model"] = serde_json::json!(cfg.model);
        payload["stream"] = serde_json::json!(true);

        let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));

        let mut request = self
            .client_for(&cfg)
            .post(&url)
            .header("Content-Type", "application/json");

        if !cfg.api_key.trim().is_empty() {
            request = request.header("Authorization", format!("Bearer {}", cfg.api_key));
        }

        let resp = match request.json(&payload).send().await {
            Ok(r) => r,
            Err(e) => return Err(format!("大模型网络请求失败 ({}): {}", url, e)),
        };

        if !resp.status().is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            return Err(format!("大模型接口返回错误: {}", err_body));
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut full_content = String::new();
        let mut reasoning_content = String::new();
        let mut buffer = String::new();

        while let Some(item) = stream.next().await {
            let chunk = match item {
                Ok(bytes) => bytes,
                Err(e) => return Err(format!("流式读取异常: {}", e)),
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.drain(..=pos);

                if line.is_empty() || line.starts_with(':') {
                    continue;
                }

                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data == "[DONE]" {
                        break;
                    }
                    if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                        if let Some(delta) = parsed
                            .get("choices")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("delta"))
                        {
                            if let Some(c) = delta.get("content").and_then(|v| v.as_str()) {
                                full_content.push_str(c);
                                on_chunk(c);
                            }
                            if let Some(r) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                                reasoning_content.push_str(r);
                            }
                        }
                    }
                }
            }
        }

        let reasoning_opt = if reasoning_content.is_empty() {
            None
        } else {
            Some(reasoning_content)
        };

        Ok((full_content, reasoning_opt))
    }

    /// Generate a streaming AI reply given history and user input.
    pub async fn generate_reply_stream<F>(
        &self,
        history: &[ChatMessage],
        incoming: &str,
        instruction: &str,
        on_chunk: F,
    ) -> Result<(String, Option<String>), String>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let mut messages = Vec::new();
        if !instruction.trim().is_empty() {
            messages.push(serde_json::json!({
                "role": "system",
                "content": instruction
            }));
        }
        for m in history {
            messages.push(serde_json::json!({
                "role": m.role,
                "content": m.content
            }));
        }
        messages.push(serde_json::json!({
            "role": "user",
            "content": incoming
        }));

        self.stream_chat(serde_json::json!({ "messages": messages }), on_chunk)
            .await
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

    /// Generate a streaming completion with an explicit system prompt.
    pub async fn generate_with_system_stream<F>(
        &self,
        system_prompt: &str,
        messages: &[ChatMessage],
        temperature: f32,
        on_chunk: F,
    ) -> Result<(String, Option<String>), String>
    where
        F: FnMut(&str) + Send + 'static,
    {
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

        info!("Calling streaming LLM ({}) with custom system prompt", self.model());

        self.stream_chat(
            serde_json::json!({
                "messages": payload_messages,
                "temperature": temperature,
            }),
            on_chunk,
        )
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
