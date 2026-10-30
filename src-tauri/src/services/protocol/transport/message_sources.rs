//! Read-only expansion for IDs carried by an authorized message.
use super::{validate_response, OneBotClient};
use serde_json::{json, Value};

impl OneBotClient {
    async fn read_source_action(&self, action: &str, body: &Value) -> Result<Value, String> {
        self.require_bound_account().await?;
        let mut response = self.client.post(format!("{}/{action}", self.http_base_url)).json(body).send().await.map_err(|error| error.to_string())?;
        let status = response.status();
        if response.content_length().is_some_and(|length| length > 2_000_000) { return Err("消息来源响应超过大小限制".into()); }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
            if bytes.len().saturating_add(chunk.len()) > 2_000_000 { return Err("消息来源响应超过大小限制".into()); }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        validate_response(status, &value)?;
        Ok(value["data"].clone())
    }

    pub async fn get_message_source_by_id(&self, id: &str) -> Result<Value, String> {
        let id = id.parse::<i64>().map_err(|_| "引用消息编号不是受支持的数字")?;
        self.read_source_action("get_msg", &json!({"message_id":id})).await
    }

    pub async fn get_forward_source(&self, id: &str) -> Result<Vec<Value>, String> {
        if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) { return Err("合并转发编号无效".into()); }
        let value = self.read_source_action("get_forward_msg", &json!({"id":id,"message_id":id})).await?;
        let values = value["messages"].as_array().or_else(|| value["message"].as_array()).or_else(|| value["nodes"].as_array()).ok_or("合并转发没有可解析的节点")?;
        Ok(values.iter().take(32).map(|node| {
            if node["type"] == "node" { return node.clone(); }
            let mut data = node.clone();
            if data["content"].is_null() { data["content"] = data["message"].clone(); }
            json!({"type":"node","data":data})
        }).collect())
    }

    pub async fn get_private_file_source_url(&self, id: &str) -> Result<String, String> {
        if id.is_empty() || id.len() > 1024 || id.chars().any(char::is_control) { return Err("文件编号无效".into()); }
        let value = self.read_source_action("get_private_file_url", &json!({"file_id":id})).await?;
        value["url"].as_str().map(str::to_string).ok_or_else(|| "文件接口未提供可读取地址".into())
    }
}
