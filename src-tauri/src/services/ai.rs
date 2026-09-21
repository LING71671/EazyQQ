use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::{error, info};

pub struct AiService {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl AiService {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_default();

        Self {
            client,
            base_url,
            api_key,
            model,
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn has_api_key(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    /// Single place that talks to the OpenAI-compatible endpoint.
    ///
    /// Returns `(content, reasoning_content)`.
    async fn post_chat(&self, payload: Value) -> Result<(String, Option<String>), String> {
        if !self.has_api_key() {
            return Err("未配置大模型 API Key".to_string());
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("大模型网络请求失败: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let err_body = resp.text().await.unwrap_or_default();
            error!("LLM error {}: {}", status, err_body);
            return Err(format!("大模型接口返回错误 ({}): {}", status, err_body));
        }

        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("解析大模型响应失败: {}", e))?;

        let message = body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .ok_or_else(|| "大模型未返回有效的回复内容".to_string())?;

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

        info!("Calling LLM ({}) for target {}", self.model, target_name);

        self.post_chat(serde_json::json!({
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "stream": false
        }))
        .await
    }

    /// Generate a completion with an explicit system prompt.
    ///
    /// Used by the summarizer, which needs strict structured output and therefore cannot
    /// rely on the conversational system prompt baked into `generate_reply`.
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

        info!("Calling LLM ({}) with custom system prompt", self.model);

        self.post_chat(serde_json::json!({
            "model": self.model,
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
            "model": self.model,
            "messages": messages,
            "temperature": 0.7,
            "stream": false
        }))
        .await
    }
}
