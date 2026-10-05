use serde::{Deserialize, Serialize};
use tauri::{command, State};
use crate::commands::AppState;
use crate::models::ApiResponse;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_name: String,
    pub release_notes: String,
    pub html_url: String,
    pub download_url: Option<String>,
    pub published_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NapCatUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_name: String,
    pub release_notes: String,
    pub download_url: Option<String>,
}

#[command]
pub async fn check_app_update() -> Result<ApiResponse<AppUpdateInfo>, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client = reqwest::Client::builder()
        .user_agent("EazyQQ-Updater")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = "https://api.github.com/repos/LING71671/EazyQQ/releases?per_page=5";
    let resp = client.get(url).send().await
        .map_err(|e| format!("检查更新网络请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Ok(ApiResponse::ok(AppUpdateInfo {
            current_version: current_version.clone(),
            latest_version: current_version,
            has_update: false,
            release_name: "暂无线上发布版本".to_string(),
            release_notes: String::new(),
            html_url: "https://github.com/LING71671/EazyQQ/releases".to_string(),
            download_url: None,
            published_at: String::new(),
        }));
    }

    let releases: serde_json::Value = resp.json().await
        .map_err(|e| format!("解析发布数据失败: {}", e))?;

    let json = releases
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|r| {
                r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false) == false
            })
        });

    let json = match json {
        Some(j) => j,
        None => {
            return Ok(ApiResponse::ok(AppUpdateInfo {
                current_version: current_version.clone(),
                latest_version: current_version,
                has_update: false,
                release_name: "暂无线上发布版本".to_string(),
                release_notes: String::new(),
                html_url: "https://github.com/LING71671/EazyQQ/releases".to_string(),
                download_url: None,
                published_at: String::new(),
            }));
        }
    };

    let tag_name = json.get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();

    let tag_base = tag_name.split('-').next().unwrap_or(&tag_name).to_string();

    let release_name = json.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let release_notes = json.get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let html_url = json.get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://github.com/LING71671/EazyQQ/releases")
        .to_string();

    let published_at = json.get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let download_url = json.get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.ends_with(".exe") || name.ends_with(".msi") || name.ends_with(".zip") {
                    item.get("browser_download_url").and_then(|u| u.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    let has_update = !tag_base.is_empty() && tag_base != current_version;

    Ok(ApiResponse::ok(AppUpdateInfo {
        current_version,
        latest_version: if tag_name.is_empty() { env!("CARGO_PKG_VERSION").to_string() } else { tag_name },
        has_update,
        release_name,
        release_notes,
        html_url,
        download_url,
        published_at,
    }))
}

pub fn detect_local_napcat_version() -> String {
    let napcat_dir = crate::services::logging::workspace_root().join("napcat");
    let pkg = napcat_dir.join("package.json");
    if let Ok(content) = std::fs::read_to_string(&pkg) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(v) = val.get("version").and_then(|v| v.as_str()) {
                if !v.is_empty() && v != "0.0.1" {
                    return v.to_string();
                }
            }
        }
    }
    let ver_txt = napcat_dir.join("version.txt");
    if let Ok(content) = std::fs::read_to_string(&ver_txt) {
        let t = content.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    "2.7.3".to_string()
}

#[command]
pub async fn get_napcat_version(state: State<'_, AppState>) -> Result<ApiResponse<String>, String> {
    if let Ok(v) = state.onebot.get_version_info().await {
        if let Some(data) = v.get("data") {
            if let Some(ver) = data.get("app_version").and_then(|s| s.as_str()) {
                return Ok(ApiResponse::ok(ver.to_string()));
            }
        }
    }
    Ok(ApiResponse::ok(detect_local_napcat_version()))
}

#[command]
pub async fn check_napcat_update(state: State<'_, AppState>) -> Result<ApiResponse<NapCatUpdateInfo>, String> {
    let current_version = match state.onebot.get_version_info().await {
        Ok(v) => v
            .get("data")
            .and_then(|d| d.get("app_version"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(detect_local_napcat_version),
        Err(_) => detect_local_napcat_version(),
    };

    let client = reqwest::Client::builder()
        .user_agent("EazyQQ-NapCat-Updater")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = "https://api.github.com/repos/NapNeko/NapCatQQ/releases/latest";
    let resp = match client.get(url).send().await {
        Ok(r) => r,
        Err(e) => return Err(format!("检查 NapCat 更新网络失败: {}", e)),
    };

    if !resp.status().is_success() {
        return Ok(ApiResponse::ok(NapCatUpdateInfo {
            current_version: current_version.clone(),
            latest_version: current_version,
            has_update: false,
            release_name: "NapCat 官方 Release 通道响应受限".to_string(),
            release_notes: String::new(),
            download_url: None,
        }));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| format!("解析 NapCat 版本失败: {}", e))?;
    let tag_name = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();
    let release_name = json.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let release_notes = json.get("body").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let download_url = json.get("assets").and_then(|a| a.as_array()).and_then(|arr| {
        arr.iter().find_map(|item| {
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.contains("Framework") || name.contains("win32") || name.ends_with(".zip") {
                item.get("browser_download_url")
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
    });

    let current_clean = current_version.trim_start_matches('v');
    let has_update = !tag_name.is_empty() && tag_name != current_clean;

    Ok(ApiResponse::ok(NapCatUpdateInfo {
        current_version,
        latest_version: if tag_name.is_empty() { "latest".to_string() } else { tag_name },
        has_update,
        release_name,
        release_notes,
        download_url,
    }))
}

#[command]
pub async fn upgrade_napcat(download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    let napcat_dir = crate::services::logging::workspace_root().join("napcat");

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/IM", "QQ.exe"])
            .output();
    }
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let target_url = match download_url {
        Some(u) if !u.trim().is_empty() => u,
        _ => {
            let client = reqwest::Client::builder()
                .user_agent("EazyQQ-NapCat-Updater")
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| e.to_string())?;
            let resp = client
                .get("https://api.github.com/repos/NapNeko/NapCatQQ/releases/latest")
                .send()
                .await
                .map_err(|e| format!("获取下载源失败: {}", e))?;
            let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            json.get("assets")
                .and_then(|a| a.as_array())
                .and_then(|arr| {
                    arr.iter().find_map(|item| {
                        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        if name.contains("Framework") || name.ends_with(".zip") {
                            item.get("browser_download_url")
                                .and_then(|u| u.as_str())
                                .map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                })
                .ok_or_else(|| "未找到适用的 NapCat 升级压缩包".to_string())?
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(&target_url).send().await.map_err(|e| format!("下载 NapCat 失败: {}", e))?;
    let bytes = resp.bytes().await.map_err(|e| format!("读取升级包数据失败: {}", e))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let temp_zip = std::env::temp_dir().join(format!("napcat_upgrade_{}.zip", stamp));
    std::fs::write(&temp_zip, &bytes).map_err(|e| format!("写入临时升级包失败: {}", e))?;

    let output = std::process::Command::new("tar")
        .args(["-xf", &temp_zip.to_string_lossy(), "-C", &napcat_dir.to_string_lossy()])
        .output();

    let _ = std::fs::remove_file(&temp_zip);

    match output {
        Ok(out) if out.status.success() => {
            let _ = crate::services::napcat_boot::restart(&napcat_dir);
            Ok(ApiResponse::ok("NapCat 核心解压升级完成，已自动重启服务".to_string()))
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            Err(format!("解压覆盖失败: {}", err))
        }
        Err(e) => Err(format!("解压执行失败: {}", e)),
    }
}
