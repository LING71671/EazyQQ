use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{error, info};

use super::onebot::OneBotClient;
use super::pipeline::{execute_auto_reply, execute_draft};
use crate::models::MessageItemDto;
use crate::services::ai::AiService;
use crate::services::db::Database;

#[derive(Debug, Clone)]
pub struct MessageOutcome {
    pub not_a_message: bool,
    pub bypassed: bool,
    pub recorded: bool,
    pub target_id: String,
    pub mode: String,
    pub action: &'static str,
    pub detail: Option<String>,
}

impl MessageOutcome {
    pub fn ignored() -> Self {
        Self {
            not_a_message: true,
            bypassed: true,
            recorded: false,
            target_id: String::new(),
            mode: "ignore".to_string(),
            action: "skipped",
            detail: None,
        }
    }

    pub fn summary(&self) -> String {
        if self.not_a_message {
            return "非消息事件，已忽略".to_string();
        }
        if self.bypassed {
            return format!(
                "目标 {} 未加入白名单，完全旁路（未落库、未调用 AI）",
                self.target_id
            );
        }
        match self.action {
            "auto_replied" => format!("目标 {} 已自动秒回", self.target_id),
            "draft_created" => format!("目标 {} 已生成待审核草稿", self.target_id),
            "trigger_skipped" => format!(
                "目标 {} 已记录，但未满足触发条件（{}）",
                self.target_id,
                self.detail.clone().unwrap_or_default()
            ),
            "cooldown" => format!(
                "目标 {} 已记录，处于回复冷却中（{}）",
                self.target_id,
                self.detail.clone().unwrap_or_default()
            ),
            "skipped" => format!(
                "目标 {} 已记录，未触发 AI（mode={}）",
                self.target_id, self.mode
            ),
            "error" => format!(
                "目标 {} 处理出错: {}",
                self.target_id,
                self.detail.clone().unwrap_or_default()
            ),
            other => format!("目标 {} -> {}", self.target_id, other),
        }
    }
}

pub async fn handle_onebot_event(
    app_handle: Option<&AppHandle>,
    event: &Value,
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
    ai: &Arc<AiService>,
    inline_ai: bool,
) -> MessageOutcome {
    let post_type = event
        .get("post_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match post_type {
        "message" => handle_message_event(app_handle, event, db, onebot, ai, inline_ai).await,
        "notice" => {
            handle_notice_event(app_handle, event, db, onebot).await;
            MessageOutcome::ignored()
        }
        "meta_event" => {
            let meta = event
                .get("meta_event_type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            tracing::debug!("meta event: {}", meta);
            MessageOutcome::ignored()
        }
        other => {
            tracing::trace!("unhandled post_type: {}", other);
            MessageOutcome::ignored()
        }
    }
}

async fn handle_notice_event(
    _app_handle: Option<&AppHandle>,
    event: &Value,
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
) {
    let notice_type = event
        .get("notice_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match notice_type {
        "group_upload" => {
            let group_id = event
                .get("group_id")
                .map(|v| v.to_string().replace('"', ""))
                .unwrap_or_default();
            let file = event.get("file").cloned().unwrap_or(Value::Null);
            let file_id = file
                .get("id")
                .or_else(|| file.get("file_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let name = file
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let size = file.get("size").and_then(|v| v.as_i64()).unwrap_or(0);
            let busid = file.get("busid").and_then(|v| v.as_i64()).unwrap_or(0);
            let uploader = event
                .get("user_id")
                .map(|v| v.to_string().replace('"', ""))
                .unwrap_or_default();

            if file_id.is_empty() {
                tracing::warn!("group_upload notice without a usable file id: {}", event);
                return;
            }

            tracing::info!(
                "group file uploaded: group={} name={} size={}B busid={}",
                group_id,
                name,
                size,
                busid
            );

            let db = db.clone();
            let onebot = onebot.clone();
            let task = async move {
                crate::services::group_files::handle_upload_notice(
                    &db, &onebot, &group_id, &file_id, &name, size, busid, &uploader,
                )
                .await;
            };
            tauri::async_runtime::spawn(task);
        }
        "group_recall" | "friend_recall" => {
            tracing::debug!("recall notice received");
        }
        other => tracing::trace!("unhandled notice_type: {}", other),
    }
}

async fn handle_message_event(
    app_handle: Option<&AppHandle>,
    event: &Value,
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
    ai: &Arc<AiService>,
    inline_ai: bool,
) -> MessageOutcome {
    let message_type = event
        .get("message_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let sender = event.get("sender");
    let user_id = event
        .get("user_id")
        .map(|v| v.to_string().replace('\"', ""))
        .unwrap_or_default();
    let sender_name = sender
        .and_then(|s| {
            s.get("card")
                .and_then(|v| v.as_str())
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    s.get("nickname")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                })
        })
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            if !user_id.is_empty() {
                user_id.clone()
            } else if message_type == "group" {
                "群成员".to_string()
            } else {
                "好友".to_string()
            }
        });

    let target_id = if message_type == "group" {
        event
            .get("group_id")
            .map(|v| v.to_string().replace('\"', ""))
            .unwrap_or_else(|| user_id.clone())
    } else {
        user_id.clone()
    };

    let raw_message = if let Some(raw) = event.get("raw_message").and_then(|v| v.as_str()) {
        raw.to_string()
    } else if let Some(msg_arr) = event.get("message").and_then(|v| v.as_array()) {
        let mut text = String::new();
        for seg in msg_arr {
            if let Some(t) = seg
                .get("data")
                .and_then(|d| d.get("text"))
                .and_then(|v| v.as_str())
            {
                text.push_str(t);
            }
        }
        text
    } else {
        event
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    if raw_message.trim().is_empty() {
        return MessageOutcome {
            not_a_message: false,
            bypassed: true,
            recorded: false,
            target_id,
            mode: "ignore".to_string(),
            action: "skipped",
            detail: Some("空消息内容".to_string()),
        };
    }

    let policy = crate::services::policy::load(db, &target_id);
    if !policy.tracked() {
        tracing::debug!(
            "bypass message from {} ({}) - not whitelisted [{}]",
            sender_name,
            target_id,
            policy.describe()
        );
        return MessageOutcome {
            not_a_message: false,
            bypassed: true,
            recorded: false,
            target_id,
            mode: policy.mode,
            action: "skipped",
            detail: None,
        };
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let msg_id_val = event
        .get("message_id")
        .map(|v| v.to_string().replace('"', ""))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| now_ms.to_string());
    let msg_id = format!("msg_{}_{}", target_id, msg_id_val);

    let self_id = event
        .get("self_id")
        .map(|v| v.to_string().replace('\"', ""))
        .unwrap_or_default();
    let is_from_me = !self_id.is_empty() && self_id == user_id;

    let chat_msg = MessageItemDto {
        id: msg_id.clone(),
        target_id: target_id.clone(),
        sender_id: user_id.clone(),
        sender_name: sender_name.clone(),
        content: raw_message.clone(),
        is_from_me,
        ai_reply_status: "none".to_string(),
        timestamp: now_ms,
    };

    if let Err(e) = db.save_message(&chat_msg) {
        error!("cannot persist message for {}: {}", target_id, e);
    }
    if let Some(handle) = app_handle {
        let _ = handle.emit("new-chat-message", &chat_msg);
    }

    let mut outcome = MessageOutcome {
        not_a_message: false,
        bypassed: false,
        recorded: true,
        target_id: target_id.clone(),
        mode: policy.mode.clone(),
        action: "skipped",
        detail: None,
    };

    if is_from_me {
        tracing::debug!("own message in {} recorded; no AI execution", target_id);
        outcome.detail = Some("我方发出的消息，不触发 AI".to_string());
        return outcome;
    }

    if !policy.ai_execution_allowed() {
        tracing::debug!(
            "target {} is tracked but AI execution is disabled [{}]",
            target_id,
            policy.describe()
        );
        outcome.detail = Some("已入白名单但未开启 AI 执行".to_string());
        return outcome;
    }

    let trigger = crate::services::trigger::evaluate(
        &policy.trigger_condition,
        &policy.keywords,
        event,
        &self_id,
        &raw_message,
    );
    if let Some(reason) = trigger.reason() {
        tracing::debug!(
            "target {} not triggered (condition={}): {}",
            target_id,
            policy.trigger_condition,
            reason
        );
        outcome.action = "trigger_skipped";
        outcome.detail = Some(reason.to_string());
        return outcome;
    }

    if let Err(wait) = crate::services::cooldown::try_acquire(
        &format!("{}:{}:{}", self_id, message_type, target_id),
        policy.cooldown_seconds,
    ) {
        tracing::info!(
            "target {} is cooling down, {}s remaining (cooldown={}s)",
            target_id,
            wait,
            policy.cooldown_seconds
        );
        outcome.action = "cooldown";
        outcome.detail = Some(format!("冷却中，剩余 {} 秒", wait));
        return outcome;
    }

    let target_type = if message_type == "group" {
        "group"
    } else {
        "private"
    }
    .to_string();

    match policy.mode.as_str() {
        "auto_reply" => {
            info!(
                "AI auto-reply triggered for {} [{}]",
                target_id,
                policy.describe()
            );
            let task = execute_auto_reply(
                app_handle.cloned(),
                target_id.clone(),
                target_type,
                raw_message,
                sender_name,
                db.clone(),
                onebot.clone(),
                ai.clone(),
            );
            if inline_ai {
                match task.await {
                    Ok(()) => outcome.action = "auto_replied",
                    Err(e) => {
                        outcome.action = "error";
                        outcome.detail = Some(e);
                    }
                }
            } else {
                tauri::async_runtime::spawn(task);
                outcome.action = "auto_replied";
            }
        }
        "copilot" => {
            info!(
                "AI draft generation triggered for {} [{}]",
                target_id,
                policy.describe()
            );
            let task = execute_draft(
                app_handle.cloned(),
                target_id.clone(),
                sender_name,
                target_type,
                msg_id,
                raw_message,
                db.clone(),
                ai.clone(),
            );
            if inline_ai {
                match task.await {
                    Ok(()) => outcome.action = "draft_created",
                    Err(e) => {
                        outcome.action = "error";
                        outcome.detail = Some(e);
                    }
                }
            } else {
                tauri::async_runtime::spawn(task);
                outcome.action = "draft_created";
            }
        }
        _ => {}
    }

    outcome
}
