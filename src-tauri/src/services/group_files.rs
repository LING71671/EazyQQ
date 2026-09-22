//! Group file silent sync + document summarization (ROADMAP Phase 4).
//!
//! * `sync_group_files` mirrors the remote file list into SQLite.
//! * `download_group_file` resolves the OneBot download URL, enforces a size cap and
//!   writes the bytes into `EazyQQ_Data/group_files/<group_id>/`.
//! * `summarize_file` extracts text from plain-text formats and asks the model for a
//!   structured summary with key takeaways and action items.
//!
//! Formats that need a real parser (pdf / docx / xlsx) are reported as unsupported
//! instead of silently returning an empty summary.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::models::FileSummaryResultDto;
use crate::services::ai::{AiService, ChatMessage};
use crate::services::db::{Database, GroupFileRecord};
use crate::services::logging;
use crate::services::onebot::OneBotClient;

const DEFAULT_MAX_FILE_MB: i64 = 100;
/// Upper bound on how much document text we send to the model.
const MAX_PROMPT_CHARS: usize = 20_000;

/// Plain-text formats we can read directly, no parser required.
const TEXT_EXTS: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "log", "xml", "yaml", "yml", "ini", "toml",
    "rs", "ts", "tsx", "js", "jsx", "py", "java", "c", "cpp", "h", "hpp", "go", "sql", "html",
    "htm", "css", "sh", "bat", "ps1",
];

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn group_dir(group_id: &str) -> PathBuf {
    logging::data_dir().join("group_files").join(group_id)
}

/// Strip path separators and reserved characters so a remote filename cannot escape the
/// workspace or break the filesystem.
/// Public because it is a safety boundary, not an implementation detail: it is what stops
/// a remote filename from escaping the workspace. The runtime self-check exercises it.
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

fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
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
        // Match existing record by file_id OR by file_name in the same group
        let existing = all_existing
            .iter()
            .find(|r| r.file_id == f.file_id || r.file_name == f.file_name);

        // If an older duplicate row existed under a different file_id (e.g. from group_upload event), clean it up
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

/// Extract plain text from a local document.
///
/// Plain-text formats are read directly; Office formats go through the ZIP + XML
/// extractor; everything else (including PDFs whose text cannot be recovered) returns an
/// explicit error so the caller can surface an honest message instead of an empty summary.
pub fn extract_text(path: &Path) -> Result<String, String> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let ext = extension_of(&name);

    if !path.exists() {
        return Err(format!("文件不存在: {}", path.display()));
    }

    let bytes = std::fs::read(path).map_err(|e| format!("读取文件失败: {}", e))?;

    match ext.as_str() {
        "docx" => {
            tracing::info!("extract_text: {} -> docx extractor", name);
            return crate::services::document::extract_docx(&bytes);
        }
        "xlsx" => {
            tracing::info!("extract_text: {} -> xlsx extractor", name);
            return crate::services::document::extract_xlsx(&bytes);
        }
        "pptx" => {
            tracing::info!("extract_text: {} -> pptx extractor", name);
            return crate::services::document::extract_pptx(&bytes);
        }
        "pdf" => {
            tracing::info!("extract_text: {} -> best-effort pdf extractor", name);
            return crate::services::document::extract_pdf(&bytes);
        }
        "doc" | "xls" | "ppt" => {
            return Err(format!(
                "旧版二进制 .{} 格式需要专用解析器，请另存为 .{}x 后重试",
                ext, ext
            ));
        }
        _ => {}
    }

    if !TEXT_EXTS.contains(&ext.as_str()) {
        return Err(format!(
            "暂不支持解析 .{} 格式（支持纯文本类文档、docx/xlsx/pptx、以及部分未压缩的 pdf）",
            if ext.is_empty() { "<无扩展名>" } else { &ext }
        ));
    }

    // Lossy decode: group files are frequently GBK-encoded, but a best-effort UTF-8 read
    // still yields usable content rather than failing outright.
    let text = String::from_utf8_lossy(&bytes).to_string();

    if text.trim().is_empty() {
        return Err("文件内容为空或无法解码为文本".to_string());
    }

    Ok(text)
}

/// Summarize a downloaded document with the model.
pub async fn summarize_file(
    ai: &Arc<AiService>,
    local_path: &str,
) -> Result<FileSummaryResultDto, String> {
    let mut resolved_path = PathBuf::from(local_path);
    if !resolved_path.exists() {
        let candidate = Path::new("EazyQQ_Data").join("group_files").join(local_path);
        if candidate.exists() {
            resolved_path = candidate;
        } else if let Ok(entries) = std::fs::read_dir("EazyQQ_Data/group_files") {
            for entry in entries.flatten() {
                let sub = entry.path().join(local_path);
                if sub.exists() {
                    resolved_path = sub;
                    break;
                }
            }
        }
    }
    let path = &resolved_path;

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未知文档")
        .to_string();
    let file_size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);

    let text = extract_text(path)?;
    let total_chars = text.chars().count();
    let truncated: String = text.chars().take(MAX_PROMPT_CHARS).collect();

    tracing::info!(
        "file summary: {} ({} chars, sending {} chars to the model)",
        file_name,
        total_chars,
        truncated.chars().count()
    );

    let system = "你是一个文档提炼助手。必须只输出一个 JSON 对象，不要输出解释或 Markdown 代码块，格式为：\
                  {\"summary\":\"150-300 字的整体摘要\",\"key_takeaways\":[\"核心论点\"],\"action_items\":[\"待办事项及责任人\"]}。\
                  没有内容的字段返回空数组。不要编造文档中没有的信息。";

    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: format!(
            "文件名：{}\n字符数：{}\n\n以下是文档内容：\n{}",
            file_name, total_chars, truncated
        ),
    }];

    let (raw, _thinking) = ai
        .generate_with_system(system, &messages, 0.3)
        .await
        .map_err(|e| format!("文档摘要生成失败: {}", e))?;

    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    let (summary_text, key_takeaways, action_items) =
        match serde_json::from_str::<Value>(&cleaned) {
            Ok(v) => {
                let summary = v
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| cleaned.clone());
                let takeaways = string_vec(v.get("key_takeaways"));
                let actions = string_vec(v.get("action_items"));
                (summary, takeaways, actions)
            }
            Err(e) => {
                tracing::warn!(
                    "file summary: model returned non-JSON ({}), storing raw text",
                    e
                );
                (cleaned.clone(), Vec::new(), Vec::new())
            }
        };

    Ok(FileSummaryResultDto {
        file_name,
        file_size,
        total_chars: total_chars.min(i32::MAX as usize) as i32,
        summary_text,
        key_takeaways,
        action_items,
    })
}

fn string_vec(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| match item {
                    Value::String(s) => Some(s.trim().to_string()),
                    Value::Object(o) => o
                        .get("text")
                        .or_else(|| o.get("content"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim().to_string()),
                    _ => None,
                })
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Is automatic download of uploaded group files enabled?
///
/// `storage.autoSyncFiles` was previously declared in the settings UI and read by nothing,
/// so the toggle had no effect either way.
fn auto_sync_enabled(db: &Database) -> bool {
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

/// Handle a live `group_upload` notice: record the file and download it if the group is
/// on the summary whitelist (i.e. the user explicitly opted in to tracking it).
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

    // The file is always indexed so the list stays accurate; whether we pull the bytes is
    // what the user's `storage.autoSyncFiles` setting controls.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_path_separators() {
        // A malicious remote filename must not be able to escape the group directory.
        assert_eq!(sanitize_file_name("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(sanitize_file_name("..\\..\\windows\\system32"), ".._.._windows_system32");
        assert_eq!(sanitize_file_name("a/b/c.txt"), "a_b_c.txt");
    }

    #[test]
    fn sanitize_strips_leading_dots_and_reserved_chars() {
        assert_eq!(sanitize_file_name(".hidden"), "hidden");
        assert_eq!(sanitize_file_name("re:port?.txt"), "re_port_.txt");
        assert_eq!(sanitize_file_name("a<b>c|d\"e"), "a_b_c_d_e");
    }

    #[test]
    fn sanitize_handles_empty_and_control_chars() {
        assert_eq!(sanitize_file_name("   "), "unnamed_file");
        assert_eq!(sanitize_file_name(""), "unnamed_file");
        assert_eq!(sanitize_file_name("a\u{0}b"), "a_b");
    }

    #[test]
    fn sanitize_caps_length() {
        let long = "x".repeat(500);
        assert_eq!(sanitize_file_name(&long).chars().count(), 120);
    }

    #[test]
    fn extension_detection_is_case_insensitive() {
        assert_eq!(extension_of("Report.DOCX"), "docx");
        assert_eq!(extension_of("no_extension"), "");
        assert_eq!(extension_of("archive.tar.gz"), "gz");
    }

    #[test]
    fn missing_file_reports_clearly() {
        let err = extract_text(Path::new("B:/definitely/not/here.txt")).unwrap_err();
        assert!(err.contains("文件不存在"), "got: {err}");
    }

    #[test]
    fn unsupported_binary_format_is_reported_not_silently_empty() {
        let dir = std::env::temp_dir().join("eazyqq_extract_test");
        logging::ensure_dir(&dir);
        let path = dir.join("clip.mp4");
        std::fs::write(&path, b"\x00\x01\x02\x03binary").unwrap();

        let err = extract_text(&path).unwrap_err();
        assert!(err.contains("暂不支持"), "got: {err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn legacy_office_formats_get_an_actionable_message() {
        let dir = std::env::temp_dir().join("eazyqq_extract_test");
        logging::ensure_dir(&dir);
        let path = dir.join("old.doc");
        std::fs::write(&path, b"\xd0\xcf\x11\xe0legacy").unwrap();

        let err = extract_text(&path).unwrap_err();
        assert!(err.contains("另存为"), "got: {err}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn plain_text_round_trips() {
        let dir = std::env::temp_dir().join("eazyqq_extract_test");
        logging::ensure_dir(&dir);
        let path = dir.join("notes.md");
        std::fs::write(&path, "# 标题\n正文内容").unwrap();

        let text = extract_text(&path).unwrap();
        assert!(text.contains("标题"));
        assert!(text.contains("正文内容"));
        let _ = std::fs::remove_file(&path);
    }
}
