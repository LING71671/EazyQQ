use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::error;

use super::onebot::OneBotClient;
use crate::models::{MessageItemDto, PendingDraftDto};
use crate::services::ai::{AiService, ChatMessage};
use crate::services::db::Database;

pub fn build_history(db: &Database, target_id: &str) -> Vec<ChatMessage> {
    db.get_messages_by_target(target_id, 10)
        .unwrap_or_default()
        .iter()
        .map(|m| ChatMessage {
            role: if m.is_from_me {
                "assistant".to_string()
            } else {
                "user".to_string()
            },
            content: m.content.clone(),
        })
        .collect()
}

pub async fn execute_auto_reply(
    app_handle: Option<AppHandle>,
    target_id: String,
    target_type: String,
    raw_message: String,
    sender_name: String,
    db: Arc<Database>,
    onebot: Arc<OneBotClient>,
    ai: Arc<AiService>,
) -> Result<(), String> {
    let history = build_history(&db, &target_id);

    let (reply_content, _thinking) = ai
        .generate_reply(&history, &raw_message, &sender_name)
        .await
        .map_err(|e| {
            error!("AI auto-reply generation failed: {}", e);
            e
        })?;

    if reply_content.is_empty() {
        return Err("大模型返回了空回复".to_string());
    }

    let send_res = onebot
        .send_msg(&target_type, &target_id, &reply_content)
        .await
        .map_err(|e| format!("自动回复发送失败: {}", e))?;

    let reply_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let msg_id_val = send_res
        .get("data")
        .and_then(|d| d.get("message_id"))
        .map(|v| v.to_string().replace('"', ""))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("reply_{}", reply_ms));

    let reply_dto = MessageItemDto {
        id: format!("msg_{}_{}", target_id, msg_id_val),
        target_id: target_id.clone(),
        sender_id: "me".to_string(),
        sender_name: "我 (AI自动秒回)".to_string(),
        content: reply_content,
        is_from_me: true,
        ai_reply_status: "auto_replied".to_string(),
        timestamp: reply_ms,
    };
    let _ = db.save_message(&reply_dto);
    if let Some(handle) = app_handle {
        let _ = handle.emit("new-chat-message", &reply_dto);
    }
    Ok(())
}

pub async fn execute_draft(
    app_handle: Option<AppHandle>,
    target_id: String,
    sender_name: String,
    target_type: String,
    reply_to_msg_id: String,
    raw_message: String,
    db: Arc<Database>,
    ai: Arc<AiService>,
) -> Result<(), String> {
    let history = build_history(&db, &target_id);

    let (generated_content, thinking_content) = ai
        .generate_reply(&history, &raw_message, &sender_name)
        .await
        .map_err(|e| {
            error!("AI draft generation failed: {}", e);
            e
        })?;

    let draft_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let draft = PendingDraftDto {
        id: format!("draft_{}", draft_ms),
        target_id,
        target_name: sender_name,
        target_type,
        reply_to_msg_id,
        incoming_message_snippet: raw_message,
        generated_content,
        thinking_content,
        model_used: ai.model(),
        created_at: draft_ms,
    };

    db.save_draft(&draft)
        .map_err(|e| format!("草稿落库失败: {}", e))?;
    if let Some(handle) = app_handle {
        let _ = handle.emit("new-draft", &draft);
    }
    Ok(())
}
