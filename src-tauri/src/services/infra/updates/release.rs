use serde::{Deserialize, Serialize};
use serde_json::Value;

const RELEASES_URL: &str = "https://api.github.com/repos/LING71671/EazyQQ/releases?per_page=100";
const REPO: &str = "https://github.com/LING71671/EazyQQ";

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
    pub status: String,
    pub installer_name: Option<String>,
    pub download_size: Option<u64>,
    pub checksum_sha256: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct Artifact {
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
}

fn text(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_string()
}

pub(super) fn select(releases: &Value) -> Option<&Value> {
    releases.as_array()?.iter().filter(|r| {
        !r["draft"].as_bool().unwrap_or(false) && !r["prerelease"].as_bool().unwrap_or(false)
    }).filter_map(|r| {
        let version = semver::Version::parse(r["tag_name"].as_str()?.trim_start_matches('v')).ok()?;
        version.pre.is_empty().then_some((version, r))
    }).max_by(|a, b| a.0.cmp(&b.0)).map(|(_, release)| release)
}

pub(super) fn installer(release: &Value) -> Option<&Value> {
    let tag = release["tag_name"].as_str()?;
    let version = semver::Version::parse(tag.strip_prefix('v')?).ok()?;
    let name = format!("EazyQQ_{}_x64-setup.exe", version);
    release["assets"].as_array()?.iter().find(|asset| {
        asset["name"].as_str() == Some(&name)
            && asset["browser_download_url"].as_str() == Some(&format!("{REPO}/releases/download/{tag}/{name}"))
    })
}

pub(super) fn valid_hash(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn checksum_record(body: &str, name: &str) -> Option<String> {
    body.lines().find_map(|line| {
        let (hash, file) = line.trim().split_once("  ")?;
        (file.trim_start_matches('*') == name && valid_hash(hash)).then(|| hash.to_ascii_lowercase())
    })
}

async fn resolve(client: &reqwest::Client, release: &Value, asset: &Value) -> Result<Artifact, String> {
    let name = text(asset, "name");
    let size = asset["size"].as_u64().filter(|size| *size > 0 && *size <= super::download::MAX_SIZE)
        .ok_or("安装包大小无效或超过 256 MiB")?;
    let digest = asset["digest"].as_str().and_then(|value| value.strip_prefix("sha256:"))
        .filter(|hash| valid_hash(hash)).map(str::to_ascii_lowercase);
    let sha256 = match digest {
        Some(hash) => hash,
        None => {
            let tag = text(release, "tag_name");
            let url = format!("{REPO}/releases/download/{tag}/SHA256SUMS.txt");
            let exists = release["assets"].as_array().is_some_and(|assets| assets.iter().any(|a| {
                a["name"].as_str() == Some("SHA256SUMS.txt") && a["browser_download_url"].as_str() == Some(&url)
            }));
            if !exists { return Err("发布缺少安装包 SHA256，暂不能自动安装；可打开发布页检查".into()); }
            let response = client.get(url).send().await.map_err(|e| format!("获取校验和失败：{e}"))?
                .error_for_status().map_err(|e| e.to_string())?;
            if response.content_length().is_some_and(|size| size > 64 * 1024) { return Err("校验和文件过大".into()); }
            let mut response = response;
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                if bytes.len() + chunk.len() > 64 * 1024 { return Err("校验和文件过大".into()); }
                bytes.extend_from_slice(&chunk);
            }
            checksum_record(&String::from_utf8(bytes).map_err(|e| e.to_string())?, &name)
                .ok_or("校验和文件缺少对应安装包")?
        }
    };
    Ok(Artifact { name, url: text(asset, "browser_download_url"), size, sha256 })
}

pub(super) async fn latest(client: &reqwest::Client) -> Result<Option<Value>, String> {
    let response = client.get(RELEASES_URL).send().await.map_err(|e| format!("检查更新网络失败：{e}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
    if !response.status().is_success() { return Err(format!("检查更新失败：HTTP {}，请稍后重试", response.status())); }
    let releases: Value = response.json().await.map_err(|e| format!("发布数据无效：{e}"))?;
    if !releases.is_array() { return Err("发布数据不是版本列表".into()); }
    Ok(select(&releases).cloned())
}

pub(super) async fn artifact(client: &reqwest::Client, release: &Value) -> Result<Artifact, String> {
    resolve(client, release, installer(release).ok_or("该版本没有 Windows x64 桌面安装包")?).await
}

pub async fn check() -> Result<AppUpdateInfo, String> {
    let client = super::client()?;
    let current = env!("CARGO_PKG_VERSION").to_string();
    let Some(release) = latest(&client).await? else {
        return Ok(AppUpdateInfo { current_version: current.clone(), latest_version: current, has_update: false,
            release_name: "暂无稳定发布版本".into(), release_notes: String::new(), html_url: format!("{REPO}/releases"),
            download_url: None, published_at: String::new(), status: "no_release".into(), installer_name: None,
            download_size: None, checksum_sha256: None });
    };
    let version = text(&release, "tag_name").trim_start_matches('v').to_string();
    let has_update = crate::services::infra::versions::newer(&version, &current);
    let artifact = if installer(&release).is_some() { Some(artifact(&client, &release).await?) } else { None };
    let status = if artifact.is_none() { "installer_missing" } else if has_update { "available" }
        else if version == current { "up_to_date" } else { "newer_local" };
    Ok(AppUpdateInfo { current_version: current, latest_version: version, has_update,
        release_name: text(&release,"name"), release_notes: text(&release,"body"), html_url: text(&release,"html_url"),
        published_at: text(&release,"published_at"), status: status.into(),
        download_url: artifact.as_ref().map(|a| a.url.clone()), installer_name: artifact.as_ref().map(|a| a.name.clone()),
        download_size: artifact.as_ref().map(|a| a.size), checksum_sha256: artifact.map(|a| a.sha256) })
}
