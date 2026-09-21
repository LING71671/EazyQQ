use std::sync::Arc;
use std::time::Duration;
use futures_util::StreamExt;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

use crate::models::{MessageItemDto, PendingDraftDto};
use crate::services::ai::AiService;
use crate::services::db::Database;
use crate::services::onebot::OneBotClient;

/// The strictly authorized test contact for auto-reply / draft testing
pub const AUTHORIZED_TEST_UIN: &str = "1739677116";

pub fn start_onebot_ws_listener(
    app_handle: AppHandle,
    ws_url: String,
    db: Arc<Database>,
    onebot: Arc<OneBotClient>,
    ai: Arc<AiService>,
) {
    tauri::async_runtime::spawn(async move {
        info!("Starting OneBot WebSocket listener targeting {}", ws_url);

        loop {
            match connect_async(&ws_url).await {
                Ok((ws_stream, _)) => {
                    info!("Successfully connected to OneBot WebSocket ({})", ws_url);
                    let (_, mut read) = ws_stream.split();

                    while let Some(msg_res) = read.next().await {
                        match msg_res {
                            Ok(msg) => {
                                if msg.is_text() {
                                    if let Ok(text) = msg.into_text() {
                                        if let Ok(json) = serde_json::from_str::<Value>(&text) {
                                            handle_onebot_event(
                                                &app_handle,
                                                &json,
                                                &db,
                                                &onebot,
                                                &ai,
                                            ).await;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("WebSocket stream error: {}, will reconnect...", e);
                                break;
                            }
                        }
                    }
                }
                Err(_e) => {
                    // Backoff and retry
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    });
}

async fn handle_onebot_event(
    app_handle: &AppHandle,
    event: &Value,
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
    ai: &Arc<AiService>,
) {
    let post_type = event.get("post_type").and_then(|v| v.as_str()).unwrap_or("");
    if post_type != "message" {
        return;
    }

    let message_type = event.get("message_type").and_then(|v| v.as_str()).unwrap_or("");
    let sender = event.get("sender");
    let user_id = event.get("user_id").map(|v| v.to_string().replace('\"', "")).unwrap_or_default();
    let sender_name = sender
        .and_then(|s| s.get("card").or_else(|| s.get("nickname")))
        .and_then(|v| v.as_str())
        .unwrap_or("好友")
        .to_string();

    let target_id = if message_type == "group" {
        event.get("group_id").map(|v| v.to_string().replace('\"', "")).unwrap_or(user_id.clone())
    } else {
        user_id.clone()
    };

    // Extract text content
    let raw_message = if let Some(raw) = event.get("raw_message").and_then(|v| v.as_str()) {
        raw.to_string()
    } else if let Some(msg_arr) = event.get("message").and_then(|v| v.as_array()) {
        let mut text = String::new();
        for seg in msg_arr {
            if let Some(t) = seg.get("data").and_then(|d| d.get("text")).and_then(|v| v.as_str()) {
                text.push_str(t);
            }
        }
        text
    } else {
        event.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string()
    };

    if raw_message.trim().is_empty() {
        return;
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let msg_id = format!("msg_{}_{}", target_id, now_ms);

    let chat_msg = MessageItemDto {
        id: msg_id.clone(),
        target_id: target_id.clone(),
        sender_id: user_id.clone(),
        sender_name: sender_name.clone(),
        content: raw_message.clone(),
        is_from_me: false,
        ai_reply_status: "none".to_string(),
        timestamp: now_ms,
    };

    // 1. Record incoming message into SQLite
    let _ = db.save_message(&chat_msg);

    // 2. Broadcast to UI
    let _ = app_handle.emit("new-chat-message", &chat_msg);

    // 3. Match whitelist rule
    let rules = db.get_all_rules().unwrap_or_default();
    let matched_rule = rules.into_iter().find(|r| r.target_id == target_id);

    let mode = matched_rule.map(|r| r.mode).unwrap_or_else(|| "ignore".to_string());

    // STRICT USER CONSTRAINT: Only test contact 白衣卿相 (1739677116) can receive auto replies / drafts
    let is_authorized_target = target_id == AUTHORIZED_TEST_UIN;

    if !is_authorized_target && mode != "ignore" {
        info!("Target {} has mode {}, but only test contact {} is authorized for AI execution.", target_id, mode, AUTHORIZED_TEST_UIN);
        return;
    }

    match mode.as_str() {
        "auto_reply" => {
            info!("Triggering AI auto-reply for authorized contact {}", target_id);
            let ai = ai.clone();
            let onebot = onebot.clone();
            let db = db.clone();
            let app_handle = app_handle.clone();
            let target_type = if message_type == "group" { "group" } else { "private" }.to_string();

            tauri::async_runtime::spawn(async move {
                // Fetch recent history
                let history = db.get_messages_by_target(&target_id, 10).unwrap_or_default();
                let chat_history: Vec<crate::services::ai::ChatMessage> = history.iter().map(|m| {
                    crate::services::ai::ChatMessage {
                        role: if m.is_from_me { "assistant".to_string() } else { "user".to_string() },
                        content: m.content.clone(),
                    }
                }).collect();

                match ai.generate_reply(&chat_history, &raw_message, &sender_name).await {
                    Ok((reply_content, _thinking)) => {
                        if !reply_content.is_empty() {
                            // Send via OneBot
                            let send_res = onebot.send_msg(&target_type, &target_id, &reply_content).await;
                            if send_res.is_ok() {
                                let reply_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_millis() as i64;
                                let reply_dto = MessageItemDto {
                                    id: format!("reply_{}", reply_ms),
                                    target_id: target_id.clone(),
                                    sender_id: "me".to_string(),
                                    sender_name: "我 (AI自动秒回)".to_string(),
                                    content: reply_content,
                                    is_from_me: true,
                                    ai_reply_status: "auto_replied".to_string(),
                                    timestamp: reply_ms,
                                };
                                let _ = db.save_message(&reply_dto);
                                let _ = app_handle.emit("new-chat-message", &reply_dto);
                            }
                        }
                    }
                    Err(e) => {
                        error!("AI auto-reply generation failed: {}", e);
                    }
                }
            });
        }
        "copilot" => {
            info!("Generating AI draft for review for authorized contact {}", target_id);
            let ai = ai.clone();
            let db = db.clone();
            let app_handle = app_handle.clone();
            let target_type = if message_type == "group" { "group" } else { "friend" }.to_string();

            tauri::async_runtime::spawn(async move {
                let history = db.get_messages_by_target(&target_id, 10).unwrap_or_default();
                let chat_history: Vec<crate::services::ai::ChatMessage> = history.iter().map(|m| {
                    crate::services::ai::ChatMessage {
                        role: if m.is_from_me { "assistant".to_string() } else { "user".to_string() },
                        content: m.content.clone(),
                    }
                }).collect();

                match ai.generate_reply(&chat_history, &raw_message, &sender_name).await {
                    Ok((generated_content, thinking_content)) => {
                        let draft_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64;
                        let draft = PendingDraftDto {
                            id: format!("draft_{}", draft_ms),
                            target_id: target_id.clone(),
                            target_name: sender_name,
                            target_type,
                            reply_to_msg_id: msg_id,
                            incoming_message_snippet: raw_message,
                            generated_content,
                            thinking_content,
                            model_used: "qwen3.8-flash".to_string(),
                            created_at: draft_ms,
                        };
                        let _ = db.save_draft(&draft);
                        let _ = app_handle.emit("new-draft", &draft);
                    }
                    Err(e) => {
                        error!("AI draft generation failed: {}", e);
                    }
                }
            });
        }
        _ => {
            // "ignore" -> Do nothing, already logged to DB
        }
    }
}
