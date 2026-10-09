use crate::models::ApiResponse;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NapCatUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_name: String,
    pub release_notes: String,
    pub download_url: Option<String>,
    pub status: String,
}

pub fn detect_local_napcat_version() -> String {
    let source = crate::services::napcat_boot::locate_napcat_dir();
    crate::services::infra::versions::napcat(&source)
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

pub async fn check(
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
    describe(current_version, &json)
}

pub(super) fn describe(current_version: String, json: &serde_json::Value) -> Result<ApiResponse<NapCatUpdateInfo>, String> {
    let tag = json["tag_name"].as_str().ok_or("协议发布缺少版本")?;
    let version = semver::Version::parse(tag.trim_start_matches('v')).map_err(|_| "协议发布版本无效")?;
    if json["draft"].as_bool().unwrap_or(false) || json["prerelease"].as_bool().unwrap_or(false) || !version.pre.is_empty() {
        return Err("自动升级仅支持稳定协议版本".into());
    }
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
                if name == "NapCat.Shell.zip" && item["browser_download_url"].as_str() == Some(&format!("https://github.com/NapNeko/NapCatQQ/releases/download/{tag}/NapCat.Shell.zip")) {
                    item.get("browser_download_url")
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    let current_clean = current_version.trim_start_matches('v');
    let has_update = current_version == "unknown" || crate::services::infra::versions::newer(&tag_name, current_clean);
    let status = if download_url.is_none() { "asset_missing" } else if current_version == "unknown" { "version_unknown" } else if has_update { "available" } else if current_clean == tag_name { "up_to_date" } else { "newer_local" };

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
        status: status.into(),
    }))
}

pub async fn upgrade(download_url: Option<String>, report: &(dyn Fn(super::UpdateProgress) + Send + Sync)) -> Result<ApiResponse<String>, String> {
    let _update = crate::services::infra::persistence::FileLock::try_acquire(&super::staging_root().join("download.lock"))
        .map_err(|_| "另一个更新任务正在执行，请等待完成".to_string())?;
    report(super::UpdateProgress { phase:"checking", downloaded_bytes:0, total_bytes:0 });
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
            "当前协议会话仍在运行，请先在账号管理中停止受管理账号；已有 QQ 会话已保留"
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
                "账号 {} 仍在运行，请先停止该账号后升级共享协议资源；未终止任何 QQ",
                instance.uin
            ));
        }
    }
    let client = crate::services::infra::updates::client()?;
    let release: serde_json::Value = client.get("https://api.github.com/repos/NapNeko/NapCatQQ/releases/latest")
        .timeout(std::time::Duration::from_secs(15)).send().await
        .map_err(|e| format!("获取协议发布失败：{e}"))?.error_for_status().map_err(|e| e.to_string())?
        .json().await.map_err(|e| e.to_string())?;
    describe(detect_local_napcat_version(), &release)?;
    let tag = release["tag_name"].as_str().ok_or("协议发布缺少版本")?;
    let expected_url = format!("https://github.com/NapNeko/NapCatQQ/releases/download/{tag}/NapCat.Shell.zip");
    let asset = release["assets"].as_array().and_then(|assets| assets.iter().find(|asset| {
        asset["name"].as_str() == Some("NapCat.Shell.zip")
            && asset["browser_download_url"].as_str() == Some(&expected_url)
    })).ok_or("协议发布缺少官方 NapCat.Shell.zip")?;
    if download_url.as_deref().is_some_and(|url| !url.trim().is_empty() && url != expected_url) {
        return Err("协议下载链接已过期或不是官方 Shell 包，请重新检查更新".into());
    }
    let expected_size = asset["size"].as_u64().filter(|size| *size > 0 && *size <= 256 * 1024 * 1024)
        .ok_or("协议包大小无效或超过 256 MiB")?;
    let expected_hash = asset["digest"].as_str().and_then(|digest| digest.strip_prefix("sha256:"))
        .filter(|hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("官方协议包没有有效 SHA256，已拒绝覆盖安装")?;
    let mut response = client.get(&expected_url).send().await.map_err(|e| format!("下载协议失败：{e}"))?
        .error_for_status().map_err(|e| e.to_string())?;
    if response.content_length().is_some_and(|size| size != expected_size) {
        return Err("协议响应大小不匹配".into());
    }
    report(super::UpdateProgress { phase:"downloading", downloaded_bytes:0, total_bytes:expected_size });
    let mut last_report = std::time::Instant::now();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| format!("协议下载中断：{e}"))? {
        if bytes.len() as u64 + chunk.len() as u64 > expected_size { return Err("协议下载超过大小上限".into()); }
        bytes.extend_from_slice(&chunk);
        if last_report.elapsed().as_millis() >= 150 || bytes.len() as u64 == expected_size {
            report(super::UpdateProgress { phase:"downloading", downloaded_bytes:bytes.len() as u64, total_bytes:expected_size });
            last_report=std::time::Instant::now();
        }
    }
    report(super::UpdateProgress { phase:"verifying", downloaded_bytes:bytes.len() as u64, total_bytes:expected_size });
    use sha2::Digest;
    if bytes.len() as u64 != expected_size || format!("{:x}", sha2::Sha256::digest(&bytes)) != expected_hash.to_ascii_lowercase() {
        return Err("协议包大小或 SHA256 不匹配，原资源保持不变".into());
    }

    report(super::UpdateProgress { phase:"installing", downloaded_bytes:expected_size, total_bytes:expected_size });
    let staging = crate::services::infra::updates::staging_root().join("napcat");
    let target = staging.clone();
    tokio::task::spawn_blocking(move || {
        crate::services::infra::protocol_update::stage_and_install(&bytes, &target, &napcat_dir)
    })
    .await
    .map_err(|e| e.to_string())??;
    crate::services::protocol::boot::set_auto_start(false);
    report(super::UpdateProgress { phase:"ready", downloaded_bytes:expected_size, total_bytes:expected_size });
    Ok(ApiResponse::ok(
        "协议包 SHA256 已校验并更新，账号配置与回滚备份保留；请启动所需账号加载新版本。".into(),
    ))
}
