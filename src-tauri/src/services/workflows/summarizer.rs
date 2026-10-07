//! Group chat summarizer (ROADMAP Phase 5).

use std::sync::Arc;

use crate::models::GroupSummaryDto;
use crate::services::ai::{AiService, ChatMessage};
use crate::services::db::Database;

const MAX_PROMPT_CHARS: usize = 12_000;
const MAX_MESSAGE_CHARS: usize = 800;

pub const DEFAULT_PROMPT: &str = "请提炼群聊核心讨论要点、达成的共识决策与待办行动项，结构清晰、简洁明了，不要复述闲聊。";

#[derive(Debug, Clone)]
pub struct SummaryRequest {
    pub target_id: String,
    pub sliding_window_hours: i32,
    pub custom_prompt: Option<String>,
    pub max_messages: usize,
    pub min_messages: usize,
}

impl SummaryRequest {
    pub fn new(target_id: impl Into<String>, sliding_window_hours: i32) -> Self {
        Self {
            target_id: target_id.into(),
            sliding_window_hours,
            custom_prompt: None,
            max_messages: 400,
            min_messages: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SummaryOutcome {
    pub summary: GroupSummaryDto,
    pub scanned: usize,
    pub usable: usize,
    pub skipped_reason: Option<String>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn is_noise(content: &str) -> bool {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return true;
    }
    let meaningful = trimmed
        .chars()
        .filter(|c| c.is_alphanumeric() || (*c >= '\u{4E00}' && *c <= '\u{9FFF}'))
        .count();
    meaningful < 2
}

pub fn dedupe_flood(messages: &[(String, String, String)]) -> Vec<(String, String, String)> {
    let mut out: Vec<(String, String, String)> = Vec::new();
    let mut last_key: Option<(String, String)> = None;
    let mut repeats = 0usize;

    for (sender, content, ts) in messages {
        let key = (sender.clone(), content.trim().to_string());
        if Some(&key) == last_key.as_ref() {
            repeats += 1;
            if repeats >= 2 {
                continue;
            }
        } else {
            repeats = 0;
            last_key = Some(key);
        }
        out.push((sender.clone(), content.clone(), ts.clone()));
    }
    out
}

pub fn strip_code_fences(raw: &str) -> String {
    let t = raw.trim();
    let t = t.strip_prefix("```json").unwrap_or(t);
    let t = t.strip_prefix("```").unwrap_or(t);
    let t = t.strip_suffix("```").unwrap_or(t);
    t.trim().to_string()
}

pub fn as_string_vec(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| match item {
                    serde_json::Value::String(s) => Some(s.trim().to_string()),
                    serde_json::Value::Object(o) => o
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

pub async fn generate(
    db: &Arc<Database>,
    ai: &Arc<AiService>,
    req: &SummaryRequest,
) -> Result<SummaryOutcome, String> {
    generate_stream(db, ai, req, |_| {}).await
}

pub async fn generate_stream<F>(
    db: &Arc<Database>,
    ai: &Arc<AiService>,
    req: &SummaryRequest,
    on_chunk: F,
) -> Result<SummaryOutcome, String>
where
    F: FnMut(&str) + Send + 'static,
{
    let window_hours = req.sliding_window_hours.max(1);
    let end_time = now_ms();
    let start_time = end_time - (window_hours as i64 * 3600 * 1000);

    let raw = db
        .get_messages_in_window(&req.target_id, start_time, req.max_messages)
        .map_err(|e| format!("读取消息流水失败: {}", e))?;
    let scanned = raw.len();

    let tuples: Vec<(String, String, String)> = raw
        .iter()
        .filter(|m| !is_noise(&m.content))
        .map(|m| {
            let who = if m.is_from_me {
                "我".to_string()
            } else {
                m.sender_name.clone()
            };
            let mut content = m.content.trim().to_string();
            if content.chars().count() > MAX_MESSAGE_CHARS {
                content = content.chars().take(MAX_MESSAGE_CHARS).collect::<String>() + "…";
            }
            (who, content, m.timestamp.to_string())
        })
        .collect();

    let usable = dedupe_flood(&tuples);
    let usable_count = usable.len();

    let target_name = db
        .get_all_rules()
        .unwrap_or_default()
        .into_iter()
        .find(|r| r.target_id == req.target_id)
        .map(|r| r.name)
        .unwrap_or_else(|| format!("群聊 ({})", req.target_id));

    tracing::info!(
        "summarizer: {} - scanned {} messages, {} usable after noise filtering (window {}h)",
        target_name,
        scanned,
        usable_count,
        window_hours
    );

    if usable_count < req.min_messages {
        let summary = GroupSummaryDto {
            id: format!("sum_{}_{}", req.target_id, end_time),
            target_id: req.target_id.clone(),
            target_name,
            summary_text: format!(
                "过去 {} 小时内有效消息不足（仅 {} 条），未生成简报。",
                window_hours, usable_count
            ),
            key_points: vec![],
            decisions: vec![],
            shared_files: vec![],
            start_time,
            end_time,
            created_at: end_time,
        };
        db.save_summary(&summary).map_err(|e| e.to_string())?;
        return Ok(SummaryOutcome {
            summary,
            scanned,
            usable: usable_count,
            skipped_reason: Some(format!("有效消息不足（{} 条）", usable_count)),
        });
    }

    let mut transcript = String::new();
    for (sender, content, _ts) in &usable {
        let line = format!("{}: {}\n", sender, content);
        if transcript.chars().count() + line.chars().count() > MAX_PROMPT_CHARS {
            break;
        }
        transcript.push_str(&line);
    }

    let instruction = req
        .custom_prompt
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_PROMPT.to_string());

    let system = format!(
        "你是一个群聊信息提炼助手。{} \n\
         必须只输出一个 JSON 对象，不要输出任何解释或 Markdown 代码块，格式为：\n\
         {{\"summary\":\"一段 80-200 字的整体概述\",\"key_points\":[\"讨论要点\"],\"decisions\":[\"已达成的决议\"],\"todos\":[\"待办事项及责任人\"]}}\n\
         没有内容的字段返回空数组。不要编造聊天中没有的信息。",
        instruction
    );

    let prompt = format!(
        "群聊名称：{}\n统计窗口：过去 {} 小时\n有效消息 {} 条\n\n以下是聊天记录（时间顺序）：\n{}",
        target_name, window_hours, usable_count, transcript
    );

    let history: Vec<ChatMessage> = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt,
    }];

    let (raw_reply, thinking) = ai
        .generate_with_system_stream(&system, &history, 0.3, on_chunk)
        .await
        .map_err(|e| format!("大模型简报生成失败: {}", e))?;

    let cleaned = strip_code_fences(&raw_reply);

    let (summary_text, key_points, decisions, todos) =
        match serde_json::from_str::<serde_json::Value>(&cleaned) {
            Ok(v) => {
                let summary_text = v
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| cleaned.clone());
                (
                    summary_text,
                    as_string_vec(v.get("key_points")),
                    as_string_vec(v.get("decisions")),
                    as_string_vec(v.get("todos")),
                )
            }
            Err(e) => {
                tracing::warn!(
                    "summarizer: model did not return parseable JSON ({}), storing raw text",
                    e
                );
                (cleaned.clone(), Vec::new(), Vec::new(), Vec::new())
            }
        };

    let mut decisions_out = decisions;
    if !todos.is_empty() {
        decisions_out.push(format!("待办: {}", todos.join("；")));
    }

    let summary = GroupSummaryDto {
        id: format!("sum_{}_{}", req.target_id, end_time),
        target_id: req.target_id.clone(),
        target_name,
        summary_text,
        key_points,
        decisions: decisions_out,
        shared_files: vec![],
        start_time,
        end_time,
        created_at: end_time,
    };

    db.save_summary(&summary).map_err(|e| e.to_string())?;
    tracing::info!(
        "summarizer: saved summary {} ({} key points, {} decisions)",
        summary.id,
        summary.key_points.len(),
        summary.decisions.len()
    );

    let _ = thinking;

    Ok(SummaryOutcome {
        summary,
        scanned,
        usable: usable_count,
        skipped_reason: None,
    })
}
