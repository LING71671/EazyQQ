use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

fn compute_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}.napcat", token).as_bytes());
    format!("{:x}", hasher.finalize())
}

pub struct NapCatService {
    client: Client,
    webui_base_url: String,
    token: String,
    napcat_dir: String,
    credential: Arc<RwLock<Option<String>>>,
}

impl NapCatService {
    pub fn new(webui_base_url: String, fallback_token: String, napcat_dir: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .no_proxy()
            .build()
            .unwrap_or_default();

        let webui_json_path = Path::new(&napcat_dir).join("config").join("webui.json");
        let token = if let Ok(content) = std::fs::read_to_string(&webui_json_path) {
            if let Ok(val) = serde_json::from_str::<Value>(&content) {
                val.get("token")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or(fallback_token)
            } else {
                fallback_token
            }
        } else {
            fallback_token
        };

        Self {
            client,
            webui_base_url,
            token,
            napcat_dir,
            credential: Arc::new(RwLock::new(None)),
        }
    }

    async fn get_credential(&self) -> Result<String, String> {
        {
            let lock = self.credential.read().await;
            if let Some(ref cred) = *lock {
                return Ok(cred.clone());
            }
        }

        let hash = compute_hash(&self.token);
        let url = format!("{}/api/auth/login", self.webui_base_url);
        let resp = self.client
            .post(&url)
            .json(&serde_json::json!({ "hash": hash }))
            .send()
            .await
            .map_err(|e| format!("WebUI 认证网络错误: {}", e))?;

        let body: Value = resp.json().await.map_err(|e| format!("解析认证返回失败: {}", e))?;
        if let Some(cred) = body.get("data").and_then(|d| d.get("Credential")).and_then(|c| c.as_str()) {
            let mut lock = self.credential.write().await;
            *lock = Some(cred.to_string());
            Ok(cred.to_string())
        } else {
            let msg = body.get("message").and_then(|m| m.as_str()).unwrap_or("未知错误");
            Err(format!("WebUI 认证失败: {}", msg))
        }
    }

    pub async fn is_alive(&self) -> bool {
        let url = format!("{}/webui", self.webui_base_url);
        self.client.get(&url).send().await.map(|r| r.status().is_success()).unwrap_or(false)
    }

    pub fn launch_if_needed(&self) -> Result<(), String> {
        let dir = Path::new(&self.napcat_dir);
        let outcome = super::boot::start(dir);
        if outcome.ok {
            Ok(())
        } else {
            Err(outcome.detail)
        }
    }

    async fn post_authed(&self, path: &str, json_body: &Value) -> Result<Value, String> {
        let cred = self.get_credential().await?;
        let url = format!("{}{}", self.webui_base_url, path);

        let resp = self.client.post(&url)
            .header("Authorization", format!("Bearer {}", cred))
            .json(json_body)
            .send()
            .await
            .map_err(|e| format!("请求 WebUI 失败 ({}): {}", path, e))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            {
                let mut lock = self.credential.write().await;
                *lock = None;
            }
            let fresh_cred = self.get_credential().await?;
            let resp2 = self.client.post(&url)
                .header("Authorization", format!("Bearer {}", fresh_cred))
                .json(json_body)
                .send()
                .await
                .map_err(|e| format!("重试 WebUI 失败 ({}): {}", path, e))?;
            return resp2.json::<Value>().await.map_err(|e| format!("解析响应失败: {}", e));
        }

        resp.json::<Value>().await.map_err(|e| format!("解析响应失败: {}", e))
    }

    pub async fn get_qrcode_info(&self) -> Result<(String, Option<String>), String> {
        let fetch = |body: Value| -> Option<(String, Option<String>)> {
            let data = body.get("data")?;
            let qr = data.get("qrcode").and_then(|v| v.as_str())?.to_string();
            if qr.is_empty() {
                return None;
            }
            let url = data.get("url").and_then(|v| v.as_str()).map(|s| s.to_string());
            Some((qr, url))
        };

        if let Ok(body) = self.post_authed("/api/QQLogin/GetQQLoginQrcode", &serde_json::json!({})).await {
            if let Some(res) = fetch(body) {
                return Ok(res);
            }
        }

        let body = self.post_authed("/api/QQLogin/GetQQLoginQrcode", &serde_json::json!({ "refresh": true })).await?;
        fetch(body).ok_or_else(|| "NapCat 未返回全新有效二维码".to_string())
    }

    pub async fn refresh_qrcode(&self) -> Result<String, String> {
        let (qr, _) = self.get_qrcode_info().await?;
        Ok(qr)
    }

    pub async fn get_qrcode(&self) -> Result<String, String> {
        let (qr, _) = self.get_qrcode_info().await?;
        Ok(qr)
    }

    pub async fn check_login(&self) -> Result<Value, String> {
        self.post_authed("/api/QQLogin/CheckLoginStatus", &serde_json::json!({})).await
    }

    pub async fn get_quick_login_list(&self) -> Result<Value, String> {
        self.post_authed("/api/QQLogin/GetQuickLoginListNew", &serde_json::json!({}))
            .await
    }

    pub async fn set_quick_login(&self, uin: &str) -> Result<Value, String> {
        self.post_authed("/api/QQLogin/SetQuickLogin", &serde_json::json!({ "uin": uin }))
            .await
    }

    pub async fn get_login_info(&self) -> Result<Value, String> {
        self.post_authed("/api/QQLogin/GetQQLoginInfo", &serde_json::json!({}))
            .await
    }

    pub fn napcat_dir(&self) -> &str {
        &self.napcat_dir
    }
}
