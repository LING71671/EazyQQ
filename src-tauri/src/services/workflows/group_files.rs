//! Group file silent sync + document summarization (ROADMAP Phase 4).

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::services::db::{Database, GroupFileRecord};
use crate::services::logging;
use crate::services::onebot::OneBotClient;

pub use super::file_summary::*;

const DEFAULT_MAX_FILE_MB: i64 = 100;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn group_dir(group_id: &str) -> PathBuf {
    logging::data_dir().join("group_files").join(group_id)
}

pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').to_string();
    if trimmed.is_empty() {
        "unnamed_file".to_string()
    } else if trimmed.chars().count() > 120 {
        trimmed.chars().take(120).collect()
    } else {
        trimmed
    }
}

fn read_config_i64(db: &Database, key_path: &[&str], fallback: i64) -> i64 {
    let raw = match db.get_setting("app_config") {
        Ok(Some(v)) => v,
        _ => return fallback,
    };
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return fallback,
    };
    let mut cursor = &parsed;
    for key in key_path {
        match cursor.get(key) {
            Some(next) => cursor = next,
            None => return fallback,
        }
    }
    cursor.as_i64().unwrap_or(fallback)
}

pub fn max_file_bytes(db: &Database) -> u64 {
    let mb = read_config_i64(db, &["storage", "maxFileSizeMb"], DEFAULT_MAX_FILE_MB).clamp(1, 4096);
    (mb as u64) * 1024 * 1024
}

/// Mirror the remote group file list into SQLite, preserving local download state.
pub async fn sync_group_files(
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
    group_id: &str,
) -> Result<Vec<GroupFileRecord>, String> {
    let remote = onebot.get_group_root_files(group_id).await?;
    tracing::info!(
        "group files: OneBot reported {} file(s) in group {}",
        remote.len(),
        group_id
    );

    let all_existing = db.get_group_files(group_id).unwrap_or_default();
    let mut records = Vec::new();
    for f in remote {
        let existing = all_existing
            .iter()
            .find(|r| r.file_id == f.file_id || r.file_name == f.file_name);

        if let Some(old) = existing {
            if old.file_id != f.file_id {
                let _ = db.delete_group_file(group_id, &old.file_id);
            }
        }

        let is_downloaded = existing.map(|e| e.download_status.as_str() == "downloaded").unwrap_or(false);
        let local_path = existing.and_then(|e| e.local_path.clone());
        let uploader_name = f.uploader_name.unwrap_or_default();
        let final_uploader = if !uploader_name.is_empty() && !uploader_name.chars().all(|c| c.is_ascii_digit()) {
            uploader_name
        } else if let Some(old) = existing {
            if !old.uploader_name.chars().all(|c| c.is_ascii_digit()) && !old.uploader_name.is_empty() {
                old.uploader_name.clone()
            } else {
                uploader_name
            }
        } else {
            uploader_name
        };

        let record = GroupFileRecord {
            file_id: f.file_id,
            group_id: group_id.to_string(),
            file_name: f.file_name,
            file_size: f.file_size.unwrap_or(0),
            busid: f.busid.unwrap_or(0),
            uploader_name: final_uploader,
            upload_time: f.upload_time.unwrap_or(0),
            local_path,
            download_status: if is_downloaded {
                "downloaded".to_string()
            } else {
                "remote".to_string()
            },
            updated_at: now_ms(),
        };
        if let Err(e) = db.upsert_group_file(&record) {
            tracing::error!("group files: cannot persist {}: {}", record.file_name, e);
        }
        records.push(record);
    }
    Ok(records)
}

/// Download one group file into the workspace and update its record.
pub async fn download_group_file(
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
    group_id: &str,
    file_id: &str,
) -> Result<PathBuf, String> {
    let mut record = db
        .find_group_file(group_id, file_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            format!(
                "本地没有该文件记录，请先同步群文件列表 (group={}, file={})",
                group_id, file_id
            )
        })?;

    let url = onebot
        .get_group_file_url(group_id, file_id, record.busid)
        .await?;

    tracing::info!("group files: downloading {} from OneBot", record.file_name);
    let bytes = onebot.download_bytes(&url, max_file_bytes(db)).await?;

    let dir = group_dir(group_id);
    logging::ensure_dir(&dir);
    let path = dir.join(sanitize_file_name(&record.file_name));

    std::fs::write(&path, &bytes).map_err(|e| format!("写入 {} 失败: {}", path.display(), e))?;

    record.local_path = Some(path.display().to_string());
    record.download_status = "downloaded".to_string();
    record.file_size = bytes.len() as i64;
    record.updated_at = now_ms();
    let _ = db.upsert_group_file(&record);

    tracing::info!(
        "group files: saved {} ({} bytes) -> {}",
        record.file_name,
        bytes.len(),
        path.display()
    );

    Ok(path)
}

pub fn auto_sync_enabled(db: &Database) -> bool {
    read_config_bool(db, &["storage", "autoSyncFiles"], true)
}

fn read_config_bool(db: &Database, key_path: &[&str], fallback: bool) -> bool {
    let raw = match db.get_setting("app_config") {
        Ok(Some(v)) => v,
        _ => return fallback,
    };
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(_) => return fallback,
    };
    let mut cursor = &parsed;
    for key in key_path {
        match cursor.get(key) {
            Some(next) => cursor = next,
            None => return fallback,
        }
    }
    cursor.as_bool().unwrap_or(fallback)
}

pub async fn handle_upload_notice(
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
    group_id: &str,
    file_id: &str,
    file_name: &str,
    file_size: i64,
    busid: i64,
    uploader_name: &str,
) {
    let policy = crate::services::policy::load(db, group_id);
    if !policy.tracked() {
        tracing::debug!(
            "group upload in {} ignored - group not whitelisted [{}]",
            group_id,
            policy.describe()
        );
        return;
    }

    let all_existing = db.get_group_files(group_id).unwrap_or_default();
    let existing = all_existing.iter().find(|r| r.file_id == file_id || r.file_name == file_name);
    if let Some(old) = existing {
        if old.file_id != file_id {
            let _ = db.delete_group_file(group_id, &old.file_id);
        }
    }

    let resolved_uploader = if uploader_name.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(rules) = db.get_all_rules() {
            rules
                .into_iter()
                .find(|r| r.target_id == uploader_name)
                .map(|r| r.name)
                .unwrap_or_else(|| uploader_name.to_string())
        } else {
            uploader_name.to_string()
        }
    } else {
        uploader_name.to_string()
    };

    let record = GroupFileRecord {
        file_id: file_id.to_string(),
        group_id: group_id.to_string(),
        file_name: file_name.to_string(),
        file_size,
        busid,
        uploader_name: resolved_uploader,
        upload_time: now_ms(),
        local_path: existing.and_then(|e| e.local_path.clone()),
        download_status: existing
            .map(|e| e.download_status.clone())
            .unwrap_or_else(|| "remote".to_string()),
        updated_at: now_ms(),
    };
    let _ = db.upsert_group_file(&record);

    if !auto_sync_enabled(db) {
        tracing::info!(
            "group upload {} indexed but not downloaded (storage.autoSyncFiles is off)",
            file_name
        );
        return;
    }

    match download_group_file(db, onebot, group_id, file_id).await {
        Ok(path) => tracing::info!("group upload auto-saved to {}", path.display()),
        Err(e) => tracing::warn!("group upload download failed for {}: {}", file_name, e),
    }
}
