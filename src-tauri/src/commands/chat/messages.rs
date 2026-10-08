use crate::commands::AppState;
use crate::models::{ApiResponse, MessageItemDto, PendingDraftDto};
use tauri::{command, State};

#[command]
pub async fn get_messages(
    state: State<'_, AppState>,
    target_id: String,
    limit: Option<i32>,
    _offset: Option<i32>,
    target_type: Option<String>,
) -> Result<ApiResponse<Vec<MessageItemDto>>, String> {
    let req_limit = limit.unwrap_or(50).clamp(10, 100);

    // 1. Sync roaming history from OneBot if available
    let is_group = target_type.as_deref() == Some("group") || {
        state
            .db
            .get_all_rules()
            .ok()
            .and_then(|rules| rules.into_iter().find(|r| r.target_id == target_id))
            .map(|r| r.target_type == "group")
            .unwrap_or(false)
    };

    let ob_msgs = if is_group {
        state
            .onebot
            .get_group_msg_history(&target_id, req_limit)
            .await
            .unwrap_or_default()
    } else {
        let friend_res = state
            .onebot
            .get_friend_msg_history(&target_id, req_limit)
            .await
            .unwrap_or_default();
        if friend_res.is_empty() {
            state
                .onebot
                .get_group_msg_history(&target_id, req_limit)
                .await
                .unwrap_or_default()
        } else {
            friend_res
        }
    };

    for item in ob_msgs {
        let raw_msg = item
            .get("raw_message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if raw_msg.trim().is_empty() {
            continue;
        }
        let time_sec = item.get("time").and_then(|v| v.as_i64()).unwrap_or(0);
        let time_ms = if time_sec > 10_000_000_000 {
            time_sec
        } else {
            time_sec * 1000
        };
        let msg_id_val = item
            .get("message_id")
            .map(|v| v.to_string().replace('"', ""))
            .unwrap_or_else(|| format!("ob_{}", time_ms));
        let unique_id = format!("msg_{}_{}", target_id, msg_id_val);

        let sender = item.get("sender");
        let sender_id = item
            .get("user_id")
            .or_else(|| sender.and_then(|s| s.get("user_id")))
            .map(|v| v.to_string().replace('"', ""))
            .unwrap_or_default();
        let self_id = item
            .get("self_id")
            .map(|v| v.to_string().replace('"', ""))
            .unwrap_or_default();
        let is_from_me = !self_id.is_empty() && self_id == sender_id;

        let sender_name = if is_from_me {
            "我".to_string()
        } else {
            sender
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
                    if !sender_id.is_empty() {
                        sender_id.clone()
                    } else if is_group {
                        "群成员".to_string()
                    } else {
                        "好友".to_string()
                    }
                })
        };

        let chat_msg = MessageItemDto {
            id: unique_id,
            target_id: target_id.clone(),
            sender_id,
            sender_name,
            content: raw_msg.to_string(),
            is_from_me,
            ai_reply_status: "none".to_string(),
            timestamp: time_ms,
        };
        let _ = state.db.save_message(&chat_msg);
    }

    // 2. Query unified SQLite storage
    let list = state
        .db
        .get_messages_by_target(&target_id, req_limit as usize)
        .map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(list))
}

#[command]
pub async fn send_message(
    state: State<'_, AppState>,
    target_id: String,
    content: String,
    target_type: Option<String>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    // Manual send is an explicit human action, so it is intentionally NOT gated by the
    // whitelist (the user typed the message themselves). It IS still validated and
    // logged so an accidental send to the wrong target is traceable.
    if content.trim().is_empty() {
        return Ok(ApiResponse::err(4001, "消息内容不能为空", None));
    }

    let policy = crate::services::policy::load(&state.db, &target_id);
    tracing::info!(
        "manual send -> {} ({}) [{}]",
        target_id,
        target_type.as_deref().unwrap_or("auto"),
        policy.describe()
    );

    let t_type = target_type.unwrap_or_else(|| "private".to_string());

    // Call OneBot send_msg
    let resp = state
        .onebot
        .send_msg(&t_type, &target_id, &content)
        .await
        .map_err(|e| format!("发送消息失败: {}", e))?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let msg_id_val = resp
        .get("data")
        .and_then(|d| d.get("message_id"))
        .map(|v| v.to_string().replace('"', ""))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("out_{}", now_ms));

    let msg_id = format!("msg_{}_{}", target_id, msg_id_val);

    let chat_msg = MessageItemDto {
        id: msg_id.clone(),
        target_id: target_id.clone(),
        sender_id: "me".to_string(),
        sender_name: "我".to_string(),
        content,
        is_from_me: true,
        ai_reply_status: "none".to_string(),
        timestamp: now_ms,
    };

    let _ = state.db.save_message(&chat_msg);

    Ok(ApiResponse::ok(serde_json::json!({
        "messageId": msg_id,
        "onebotResponse": resp
    })))
}

#[command]
pub async fn get_pending_drafts(
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<PendingDraftDto>>, String> {
    let drafts = state.db.get_pending_drafts().map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(drafts))
}

#[command]
pub async fn send_draft(
    state: State<'_, AppState>,
    draft_id: String,
    final_content: Option<String>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let drafts = state.db.get_pending_drafts().map_err(|e| e.to_string())?;
    let draft = drafts
        .into_iter()
        .find(|d| d.id == draft_id)
        .ok_or_else(|| format!("草稿 {} 不存在", draft_id))?;

    // Verify the contact is still whitelisted before sending (policy gate).
    let policy = crate::services::policy::load(&state.db, &draft.target_id);
    if !policy.ai_execution_allowed() {
        tracing::warn!(
            "refusing to send draft {} for {}: target no longer whitelisted [{}]",
            draft_id,
            draft.target_id,
            policy.describe()
        );
        return Ok(ApiResponse::err(
            4030,
            "该会话已不在消息接管白名单中，草稿已被安全拦截。",
            None,
        ));
    }

    let send_content = final_content.unwrap_or(draft.generated_content);
    let target_type = if draft.target_type == "group" {
        "group"
    } else {
        "private"
    };

    tracing::info!(
        "draft {} approved -> sending to {} ({})",
        draft_id,
        draft.target_id,
        target_type
    );

    // Send via OneBot
    let send_res = state
        .onebot
        .send_msg(target_type, &draft.target_id, &send_content)
        .await
        .map_err(|e| format!("放行发送草稿失败: {}", e))?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let msg_id_val = send_res
        .get("data")
        .and_then(|d| d.get("message_id"))
        .map(|v| v.to_string().replace('"', ""))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("draft_sent_{}", now_ms));

    let chat_msg = MessageItemDto {
        id: format!("msg_{}_{}", draft.target_id, msg_id_val),
        target_id: draft.target_id.clone(),
        sender_id: "me".to_string(),
        sender_name: "我 (AI草稿放行)".to_string(),
        content: send_content,
        is_from_me: true,
        ai_reply_status: "draft_pending".to_string(),
        timestamp: now_ms,
    };

    let _ = state.db.save_message(&chat_msg);
    let _ = state.db.delete_draft(&draft_id);

    Ok(ApiResponse::ok(serde_json::json!({
        "sentMessageId": chat_msg.id
    })))
}

#[command]
pub async fn dismiss_draft(
    state: State<'_, AppState>,
    draft_id: String,
) -> Result<ApiResponse<()>, String> {
    state
        .db
        .delete_draft(&draft_id)
        .map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn regenerate_draft(
    state: State<'_, AppState>,
    draft_id: String,
    custom_instruction: Option<String>,
) -> Result<ApiResponse<PendingDraftDto>, String> {
    let drafts = state.db.get_pending_drafts().map_err(|e| e.to_string())?;
    let mut draft = drafts
        .into_iter()
        .find(|d| d.id == draft_id)
        .ok_or_else(|| "找不到指定的草稿".to_string())?;

    let inst = custom_instruction.unwrap_or_else(|| "请换一个说法更加自然幽默".to_string());
    let (new_content, new_thinking) = state
        .ai
        .regenerate_with_instruction(
            &draft.incoming_message_snippet,
            &draft.generated_content,
            &inst,
        )
        .await
        .map_err(|e| format!("重新生成草稿失败: {}", e))?;

    draft.generated_content = new_content.clone();
    draft.thinking_content = new_thinking.clone();

    let _ = state
        .db
        .update_draft_content(&draft_id, &new_content, new_thinking.as_deref());

    Ok(ApiResponse::ok(draft))
}

#[command]
pub async fn trigger_ai_reply(
    state: State<'_, AppState>,
    target_id: String,
    context_snippet: String,
) -> Result<ApiResponse<String>, String> {
    let history = state
        .db
        .get_messages_by_target(&target_id, 10)
        .unwrap_or_default();
    let chat_history: Vec<crate::services::ai::ChatMessage> = history
        .iter()
        .map(|m| crate::services::ai::ChatMessage {
            role: if m.is_from_me {
                "assistant".to_string()
            } else {
                "user".to_string()
            },
            content: m.content.clone(),
        })
        .collect();

    let (reply, _thinking) = state
        .ai
        .generate_reply(&chat_history, &context_snippet, "好友")
        .await
        .map_err(|e| format!("AI构思回复失败: {}", e))?;

    Ok(ApiResponse::ok(reply))
}
