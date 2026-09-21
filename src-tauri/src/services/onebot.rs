use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub struct OneBotClient {
    client: Client,
    http_base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObFriendInfo {
    pub user_id: i64,
    pub nickname: String,
    pub remark: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObGroupInfo {
    pub group_id: i64,
    pub group_name: String,
    pub member_count: Option<i32>,
    pub max_member_count: Option<i32>,
}

impl OneBotClient {
    pub fn new(http_base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(6))
            .build()
            .unwrap_or_default();
        Self {
            client,
            http_base_url,
        }
    }

    pub async fn get_login_info(&self) -> Result<Value, String> {
        let url = format!("{}/get_login_info", self.http_base_url);
        let resp = self.client.post(&url)
            .send()
            .await
            .map_err(|e| format!("Network error querying login info: {}", e))?;
        
        let json: Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
        Ok(json)
    }

    pub async fn get_friend_list(&self) -> Result<Vec<ObFriendInfo>, String> {
        let url = format!("{}/get_friend_list", self.http_base_url);
        let resp = self.client.post(&url)
            .send()
            .await
            .map_err(|e| format!("Network error querying friend list: {}", e))?;
        
        let json: Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            let mut list = Vec::new();
            for item in data {
                if let Ok(info) = serde_json::from_value::<ObFriendInfo>(item.clone()) {
                    list.push(info);
                }
            }
            return Ok(list);
        }
        Ok(vec![])
    }

    pub async fn get_group_list(&self) -> Result<Vec<ObGroupInfo>, String> {
        let url = format!("{}/get_group_list", self.http_base_url);
        let resp = self.client.post(&url)
            .send()
            .await
            .map_err(|e| format!("Network error querying group list: {}", e))?;
        
        let json: Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
            let mut list = Vec::new();
            for item in data {
                if let Ok(info) = serde_json::from_value::<ObGroupInfo>(item.clone()) {
                    list.push(info);
                }
            }
            return Ok(list);
        }
        Ok(vec![])
    }

    pub async fn send_msg(&self, target_type: &str, target_id: &str, message: &str) -> Result<Value, String> {
        let url = format!("{}/send_msg", self.http_base_url);
        let mut body = serde_json::json!({
            "message": message
        });
        if target_type == "group" {
            if let Ok(gid) = target_id.parse::<i64>() {
                body["group_id"] = serde_json::json!(gid);
            } else {
                body["group_id"] = Value::String(target_id.to_string());
            }
        } else {
            if let Ok(uid) = target_id.parse::<i64>() {
                body["user_id"] = serde_json::json!(uid);
            } else {
                body["user_id"] = Value::String(target_id.to_string());
            }
        }

        let resp = self.client.post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network error sending message: {}", e))?;
        
        let json: Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
        Ok(json)
    }
}
