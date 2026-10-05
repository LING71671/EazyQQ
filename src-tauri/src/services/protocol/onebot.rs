use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

pub struct OneBotClient {
    client: Client,
    http_base_url: String,
    /// Separate client for file transfers: downloads legitimately take far longer than
    /// control-plane calls.
    download_client: Client,
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

/// A file entry as returned by OneBot's `get_group_root_files`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObGroupFileInfo {
    pub file_id: String,
    pub file_name: String,
    #[serde(default)]
    pub busid: Option<i64>,
    #[serde(default)]
    pub file_size: Option<i64>,
    #[serde(default)]
    pub upload_time: Option<i64>,
    #[serde(default)]
    pub uploader: Option<i64>,
    #[serde(default)]
    pub uploader_name: Option<String>,
}

impl OneBotClient {
    pub fn new(http_base_url: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(6))
            .no_proxy()
            .build()
            .unwrap_or_default();
        let download_client = Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .unwrap_or_default();
        Self {
            client,
            http_base_url,
            download_client,
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

    pub async fn get_version_info(&self) -> Result<Value, String> {
        let url = format!("{}/get_version_info", self.http_base_url);
        let resp = self.client.post(&url)
            .send()
            .await
            .map_err(|e| format!("Network error querying version info: {}", e))?;
        
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

    /// List the files sitting in a group's root folder.
    pub async fn get_group_root_files(&self, group_id: &str) -> Result<Vec<ObGroupFileInfo>, String> {
        let url = format!("{}/get_group_root_files", self.http_base_url);
        let group_id_value = group_id
            .parse::<i64>()
            .map(|n| Value::from(n))
            .unwrap_or_else(|_| Value::String(group_id.to_string()));

        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({ "group_id": group_id_value }))
            .send()
            .await
            .map_err(|e| format!("Network error listing group files: {}", e))?;

        let json: Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;

        let mut list = Vec::new();
        if let Some(files) = json.get("data").and_then(|d| d.get("files")).and_then(|f| f.as_array()) {
            for item in files {
                if let Ok(info) = serde_json::from_value::<ObGroupFileInfo>(item.clone()) {
                    list.push(info);
                }
            }
        }
        Ok(list)
    }

    /// Resolve a temporary download URL for one group file.
    pub async fn get_group_file_url(
        &self,
        group_id: &str,
        file_id: &str,
        busid: i64,
    ) -> Result<String, String> {
        let url = format!("{}/get_group_file_url", self.http_base_url);
        let group_id_value = group_id
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(group_id.to_string()));

        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "group_id": group_id_value,
                "file_id": file_id,
                "busid": busid,
            }))
            .send()
            .await
            .map_err(|e| format!("Network error resolving file url: {}", e))?;

        let json: Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
        json.get("data")
            .and_then(|d| d.get("url"))
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("OneBot 未返回文件下载地址: {}", json))
    }

    /// Download a file with a hard size cap so a huge group file cannot exhaust memory.
    pub async fn download_bytes(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
        let resp = self
            .download_client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Network error downloading file: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("下载失败，HTTP {}", resp.status()));
        }

        if let Some(len) = resp.content_length() {
            if len > max_bytes {
                return Err(format!(
                    "文件过大（{} MB），超过当前上限 {} MB",
                    len / 1024 / 1024,
                    max_bytes / 1024 / 1024
                ));
            }
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("读取文件内容失败: {}", e))?;

        if bytes.len() as u64 > max_bytes {
            return Err(format!(
                "文件过大（{} MB），超过当前上限 {} MB",
                bytes.len() / 1024 / 1024,
                max_bytes / 1024 / 1024
            ));
        }

        Ok(bytes.to_vec())
    }

    /// Pull roaming message history for a friend
    pub async fn get_friend_msg_history(&self, user_id: &str, count: i32) -> Result<Vec<Value>, String> {
        let url = format!("{}/get_friend_msg_history", self.http_base_url);
        let uid = user_id.parse::<i64>().unwrap_or(0);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "user_id": uid,
                "count": count,
            }))
            .send()
            .await
            .map_err(|e| format!("Network error querying friend history: {}", e))?;
        let json: Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
        if let Some(msgs) = json.get("data").and_then(|d| d.get("messages")).and_then(|m| m.as_array()) {
            return Ok(msgs.clone());
        }
        Ok(vec![])
    }

    /// Pull roaming message history for a group
    pub async fn get_group_msg_history(&self, group_id: &str, count: i32) -> Result<Vec<Value>, String> {
        let url = format!("{}/get_group_msg_history", self.http_base_url);
        let gid = group_id.parse::<i64>().unwrap_or(0);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "group_id": gid,
                "count": count,
            }))
            .send()
            .await
            .map_err(|e| format!("Network error querying group history: {}", e))?;
        let json: Value = resp.json().await.map_err(|e| format!("Parse error: {}", e))?;
        if let Some(msgs) = json.get("data").and_then(|d| d.get("messages")).and_then(|m| m.as_array()) {
            return Ok(msgs.clone());
        }
        Ok(vec![])
    }
}
