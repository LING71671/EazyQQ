use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use super::release::Artifact;

pub(super) const MAX_SIZE: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub phase: &'static str,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
}

pub struct PreparedUpdate {
    pub path: PathBuf,
    pub sha256: String,
    pub size: u64,
}

pub(super) fn verified_file(path: &Path, artifact: &Artifact) -> Result<(), String> {
    use std::io::Read;
    let mut input = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if input.metadata().map_err(|e| e.to_string())?.len() != artifact.size { return Err("安装包字节数不匹配".into()); }
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut first = true;
    loop {
        let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 { break; }
        if first && (count < 2 || &buffer[..2] != b"MZ") { return Err("下载内容不是 Windows 安装程序".into()); }
        first = false; hasher.update(&buffer[..count]);
    }
    if format!("{:x}", hasher.finalize()) != artifact.sha256 { return Err("安装包 SHA256 不匹配，已拒绝安装".into()); }
    Ok(())
}

pub(super) async fn download(client: &reqwest::Client, artifact: &Artifact, root: &Path,
    report: &(dyn Fn(UpdateProgress) + Send + Sync)) -> Result<PathBuf, String> {
    std::fs::create_dir_all(root).map_err(|e| format!("创建更新暂存目录失败：{e}"))?;
    let destination = root.join(&artifact.name);
    if verified_file(&destination, artifact).is_ok() {
        report(UpdateProgress { phase:"ready", downloaded_bytes:artifact.size, total_bytes:artifact.size });
        return Ok(destination);
    }
    let partial = root.join(format!("{}.{}.part", artifact.name, std::process::id()));
    let result = async {
        let mut response = client.get(&artifact.url).send().await.map_err(|e| format!("下载安装包失败：{e}"))?
            .error_for_status().map_err(|e| format!("下载安装包失败：{e}"))?;
        if response.content_length().is_some_and(|size| size != artifact.size || size > MAX_SIZE) {
            return Err("下载响应大小与发布记录不符".into());
        }
        let mut file = tokio::fs::File::create(&partial).await.map_err(|e| e.to_string())?;
        let mut received = 0u64;
        let mut last_report = std::time::Instant::now();
        report(UpdateProgress { phase:"downloading", downloaded_bytes:0, total_bytes:artifact.size });
        while let Some(chunk) = response.chunk().await.map_err(|e| format!("下载中断：{e}，可重试"))? {
            received += chunk.len() as u64;
            if received > artifact.size || received > MAX_SIZE { return Err("下载超过安装包大小上限".into()); }
            file.write_all(&chunk).await.map_err(|e| format!("保存安装包失败：{e}"))?;
            if last_report.elapsed().as_millis() >= 150 || received == artifact.size {
                report(UpdateProgress { phase:"downloading", downloaded_bytes:received, total_bytes:artifact.size });
                last_report = std::time::Instant::now();
            }
        }
        file.sync_all().await.map_err(|e| e.to_string())?; drop(file);
        report(UpdateProgress { phase:"verifying", downloaded_bytes:received, total_bytes:artifact.size });
        verified_file(&partial, artifact)?;
        std::fs::rename(&partial, &destination).map_err(|e| e.to_string())?;
        report(UpdateProgress { phase:"ready", downloaded_bytes:received, total_bytes:artifact.size });
        Ok(destination)
    }.await;
    if result.is_err() { let _ = std::fs::remove_file(&partial); }
    result
}

pub async fn prepare(download_url: Option<String>, report: &(dyn Fn(UpdateProgress) + Send + Sync)) -> Result<PreparedUpdate, String> {
    let root = super::staging_root();
    let _lock = crate::services::infra::persistence::FileLock::try_acquire(&root.join("download.lock"))
        .map_err(|_| "另一个更新任务正在执行，请等待完成".to_string())?;
    let client = super::client()?;
    let release = super::release::latest(&client).await?.ok_or("暂无稳定发布版本")?;
    let version = release["tag_name"].as_str().ok_or("发布版本无效")?;
    if !crate::services::infra::versions::newer(version, env!("CARGO_PKG_VERSION")) { return Err("当前已无更高的稳定版本，无需重复安装".into()); }
    let artifact = super::release::artifact(&client, &release).await?;
    if download_url.as_deref().is_some_and(|url| !url.trim().is_empty() && url != artifact.url) {
        return Err("下载链接已过期或不属于当前正式安装包，请重新检查更新".into());
    }
    let path = download(&client, &artifact, &root, report).await?;
    Ok(PreparedUpdate { path, sha256:artifact.sha256, size:artifact.size })
}
