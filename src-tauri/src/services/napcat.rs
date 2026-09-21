use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

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
            .build()
            .unwrap_or_default();

        // Dynamically resolve token from config/webui.json if available
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

    pub fn launch_if_needed(&self, target_uin: Option<&str>) -> Result<(), String> {
        let dir = Path::new(&self.napcat_dir);
        let launcher_bat = dir.join("launcher-user.bat");
        let exe_path = dir.join("NapCatWinBootMain.exe");

        let mut cmd = if launcher_bat.exists() {
            let mut c = Command::new("cmd.exe");
            c.arg("/c").arg("launcher-user.bat");
            if let Some(uin) = target_uin {
                if !uin.is_empty() {
                    c.arg(uin);
                }
            }
            c
        } else if exe_path.exists() {
            let mut c = Command::new(&exe_path);
            if let Some(uin) = target_uin {
                if !uin.is_empty() {
                    c.arg(uin);
                }
            }
            c
        } else {
            return Err(format!("找不到 NapCat 启动程序，检查目录: {:?}", dir));
        };

        cmd.current_dir(dir);

        info!("Starting NapCat daemon from {:?}", dir);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        cmd.spawn().map_err(|e| format!("Failed to spawn NapCat: {}", e))?;
        Ok(())
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
            // Invalidate cached credential and re-login
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

    pub async fn refresh_qrcode(&self) -> Result<String, String> {
        let body = self.post_authed("/api/QQLogin/GetQQLoginQrcode", &serde_json::json!({ "refresh": true })).await?;
        if let Some(qr) = body.get("data").and_then(|d| d.get("qrcode")).and_then(|v| v.as_str()) {
            if !qr.is_empty() {
                return Ok(qr.to_string());
            }
        }
        Err("NapCat 未返回全新有效二维码".to_string())
    }

    pub async fn get_qrcode(&self) -> Result<String, String> {
        // 1. Probe WebUI for currently active QR code URL
        if let Ok(body) = self.post_authed("/api/QQLogin/GetQQLoginQrcode", &serde_json::json!({})).await {
            if let Some(qr) = body.get("data").and_then(|d| d.get("qrcode")).and_then(|v| v.as_str()) {
                if !qr.is_empty() {
                    return Ok(qr.to_string());
                }
            }
        }

        // 2. If no QR exists yet, actively request a brand new one
        self.refresh_qrcode().await
    }

    pub async fn check_login(&self) -> Result<Value, String> {
        self.post_authed("/api/QQLogin/CheckLoginStatus", &serde_json::json!({})).await
    }

    /// Accounts NapCat can log into without a QR scan (Phase 1 acceptance item:
    /// "退出软件重开可实现无感快速登录").
    ///
    /// Returns `uin`, `nickName` and a real `faceUrl` for each remembered account.
    pub async fn get_quick_login_list(&self) -> Result<Value, String> {
        self.post_authed("/api/QQLogin/GetQuickLoginListNew", &serde_json::json!({}))
            .await
    }

    /// Trigger a quick login for a remembered account.
    pub async fn set_quick_login(&self, uin: &str) -> Result<Value, String> {
        self.post_authed("/api/QQLogin/SetQuickLogin", &serde_json::json!({ "uin": uin }))
            .await
    }

    /// Authoritative login info straight from the protocol backend, including a real
    /// avatar (`faceUrl`) and an `online` flag.
    pub async fn get_login_info(&self) -> Result<Value, String> {
        self.post_authed("/api/QQLogin/GetQQLoginInfo", &serde_json::json!({}))
            .await
    }
}
