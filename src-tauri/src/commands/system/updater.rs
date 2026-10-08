use crate::commands::AppState;
use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};
use tauri::{command, State};

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
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("检查更新网络请求失败: {}", e))?;

    if !resp.status().is_success() {
        if resp.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(format!("Release check failed: HTTP {}", resp.status()));
        }
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

    let releases: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析发布数据失败: {}", e))?;

    let json = releases.as_array().and_then(|arr| {
        arr.iter()
            .find(|r| r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false) == false)
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

    let tag_name = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();

    let release_name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let release_notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let html_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://github.com/LING71671/EazyQQ/releases")
        .to_string();

    let published_at = json
        .get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let download_url = json
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.ends_with(".exe") {
                    item.get("browser_download_url")
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    let has_update = crate::services::infra::versions::newer(&tag_name, &current_version);

    Ok(ApiResponse::ok(AppUpdateInfo {
        current_version,
        latest_version: if tag_name.is_empty() {
            env!("CARGO_PKG_VERSION").to_string()
        } else {
            tag_name
        },
        has_update,
        release_name,
        release_notes,
        html_url,
        download_url,
        published_at,
    }))
}

#[command]
pub async fn upgrade_app(download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    let client = reqwest::Client::builder()
        .user_agent("EazyQQ-App-Updater")
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let target_url = match download_url {
        Some(u) if !u.trim().is_empty() => u,
        _ => {
            let resp = client
                .get("https://api.github.com/repos/LING71671/EazyQQ/releases?per_page=5")
                .send()
                .await
                .map_err(|e| format!("获取 Release 列表失败: {}", e))?;
            let releases: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
            releases
                .as_array()
                .and_then(|arr| {
                    arr.iter().find_map(|r| {
                        r.get("assets")
                            .and_then(|a| a.as_array())
                            .and_then(|assets| {
                                assets.iter().find_map(|item| {
                                    let name =
                                        item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                                    if name.ends_with(".exe") || name.ends_with(".msi") {
                                        item.get("browser_download_url")
                                            .and_then(|u| u.as_str())
                                            .map(|s| s.to_string())
                                    } else {
                                        None
                                    }
                                })
                            })
                    })
                })
                .ok_or_else(|| "未找到适用的安装包下载链接".to_string())?
        }
    };

    let resp = client
        .get(&target_url)
        .send()
        .await
        .map_err(|e| format!("下载安装包失败: {}", e))?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取安装包数据失败: {}", e))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let update_dir = crate::services::logging::workspace_root().join("EazyQQ_Data/updates");
    std::fs::create_dir_all(&update_dir).map_err(|e| e.to_string())?;
    let temp_exe = update_dir.join(format!("EazyQQ_Update_Setup_{}.exe", stamp));
    std::fs::write(&temp_exe, &bytes).map_err(|e| format!("写入临时安装包失败: {}", e))?;

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new(&temp_exe)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| format!("Cannot launch installer: {}", e))?;
    }

    Ok(ApiResponse::ok(
        "安装包已验证下载并启动，请关闭 EazyQQ 后完成安装；QQ 会话保持运行。".to_string(),
    ))
}

pub fn detect_local_napcat_version() -> String {
    let source = crate::services::napcat_boot::locate_napcat_dir();
    let (dir, _, _, _) = crate::services::protocol::layout::selected(&source);
    crate::services::infra::versions::napcat(&dir)
}

pub async fn current_napcat_version(onebot: &crate::services::onebot::OneBotClient) -> String {
    onebot
        .get_version_info()
        .await
        .ok()
        .and_then(|value| value["data"]["app_version"].as_str().map(str::to_string))
        .filter(|version| !version.is_empty())
        .unwrap_or_else(detect_local_napcat_version)
}

#[command]
pub async fn get_napcat_version(state: State<'_, AppState>) -> Result<ApiResponse<String>, String> {
    Ok(ApiResponse::ok(current_napcat_version(&state.onebot).await))
}

#[command]
pub async fn check_napcat_update(
    state: State<'_, AppState>,
) -> Result<ApiResponse<NapCatUpdateInfo>, String> {
    check_napcat_update_for(&state.onebot).await
}

pub async fn check_napcat_update_for(
    onebot: &crate::services::onebot::OneBotClient,
) -> Result<ApiResponse<NapCatUpdateInfo>, String> {
    let current_version = match onebot.get_version_info().await {
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
        return Err(format!(
            "NapCat release check failed: HTTP {}",
            resp.status()
        ));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("解析 NapCat 版本失败: {}", e))?;
    let tag_name = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();
    let release_name = json
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let release_notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let download_url = json
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| {
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
    let has_update = crate::services::infra::versions::newer(&tag_name, current_clean);

    Ok(ApiResponse::ok(NapCatUpdateInfo {
        current_version,
        latest_version: if tag_name.is_empty() {
            "latest".to_string()
        } else {
            tag_name
        },
        has_update,
        release_name,
        release_notes,
        download_url,
    }))
}

#[command]
pub async fn upgrade_napcat(download_url: Option<String>) -> Result<ApiResponse<String>, String> {
    let maintenance_path = crate::services::accounts::data_root().join("protocol-maintenance.lock");
    let _maintenance = tokio::task::spawn_blocking(move || {
        crate::services::infra::persistence::FileLock::acquire(&maintenance_path)
    })
    .await
    .map_err(|e| e.to_string())??;
    let napcat_dir = crate::services::napcat_boot::locate_napcat_dir();
    let (active_dir, _, _, webui_port) = crate::services::protocol::layout::selected(&napcat_dir);
    let active_webui = crate::services::napcat::NapCatService::new(
        format!("http://127.0.0.1:{}", webui_port),
        String::new(),
        active_dir.to_string_lossy().into_owned(),
    );
    if active_webui.is_alive().await
        || crate::services::protocol::ownership::is_running(&active_dir)
    {
        return Err(
            "Stop the active protocol session before upgrading. Running QQ sessions were preserved"
                .into(),
        );
    }
    for instance in crate::services::instances::load_registry().instances {
        let (napcat, _) = crate::services::protocol::accounts::clients(&instance);
        if napcat.is_alive().await
            || crate::services::protocol::ownership::is_running(
                &crate::services::protocol::layout::runtime_dir(&instance),
            )
        {
            return Err(format!(
                "Stop account {} before upgrading the shared protocol resources",
                instance.uin
            ));
        }
    }
    crate::services::protocol::boot::set_auto_start(false);

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
    let resp = client
        .get(&target_url)
        .send()
        .await
        .map_err(|e| format!("下载 NapCat 失败: {}", e))?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取升级包数据失败: {}", e))?;

    let staging = crate::services::logging::workspace_root().join("EazyQQ_Data/updates/napcat");
    let target = staging.clone();
    tokio::task::spawn_blocking(move || {
        crate::services::infra::protocol_update::stage_and_install(&bytes, &target, &napcat_dir)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(ApiResponse::ok(
        "NapCat resources updated. Start the required accounts to load the new version".into(),
    ))
}
