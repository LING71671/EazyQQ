use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::models::FileSummaryResultDto;
use crate::services::ai::{AiService, ChatMessage};
use super::document;

pub const MAX_PROMPT_CHARS: usize = 20_000;

pub const TEXT_EXTS: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "log", "xml", "yaml", "yml", "ini", "toml",
    "rs", "ts", "tsx", "js", "jsx", "py", "java", "c", "cpp", "h", "hpp", "go", "sql", "html",
    "htm", "css", "sh", "bat", "ps1",
];

pub fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

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
            return document::extract_docx(&bytes);
        }
        "xlsx" => {
            tracing::info!("extract_text: {} -> xlsx extractor", name);
            return document::extract_xlsx(&bytes);
        }
        "pptx" => {
            tracing::info!("extract_text: {} -> pptx extractor", name);
            return document::extract_pptx(&bytes);
        }
        "pdf" => {
            tracing::info!("extract_text: {} -> best-effort pdf extractor", name);
            return document::extract_pdf(&bytes);
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

    let text = String::from_utf8_lossy(&bytes).to_string();

    if text.trim().is_empty() {
        return Err("文件内容为空或无法解码为文本".to_string());
    }

    Ok(text)
}

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

pub fn string_vec(value: Option<&Value>) -> Vec<String> {
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
