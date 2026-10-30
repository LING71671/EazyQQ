use crate::commands::AppState;
use crate::models::{ApiResponse, MessageItemDto, PendingDraftDto};
use tauri::{command, State};

#[command]
#[tracing::instrument(name = "eazyqq.ipc.get_messages", skip_all, err)]
pub async fn get_messages(
    state: State<'_, AppState>,
    target_id: String,
    limit: Option<i32>,
    _offset: Option<i32>,
    target_type: Option<String>,
) -> Result<ApiResponse<Vec<MessageItemDto>>, String> {
    let req_limit = limit.unwrap_or(50).clamp(10, 100);

    let kind = crate::services::protocol::context::target_type(&state.db, &target_id, target_type.as_deref())?;
    let scope = crate::services::protocol::context::ConversationScope::for_client(&state.onebot, &kind, &target_id)?;
    let list = crate::services::protocol::context::load_history(&state.db, &state.onebot, &scope, req_limit).await?;
    Ok(ApiResponse::ok(list))
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.send_message", skip_all, err)]
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
        sender_id: state.onebot.expected_account().unwrap_or_default().to_string(),
        sender_name: "我".to_string(),
        content,
        is_from_me: true,
        ai_reply_status: "none".to_string(),
        timestamp: now_ms,
    };

    let scope = crate::services::protocol::context::ConversationScope::for_client(&state.onebot, &t_type, &target_id)?;
    if let Err(error) = crate::services::protocol::context::save_outgoing_source(&state.db, &scope, &chat_msg, &msg_id_val, "owner_manual") {
        tracing::error!("Sent manual message source could not be saved: {}", error);
    }

    Ok(ApiResponse::ok(serde_json::json!({
        "messageId": msg_id,
        "onebotResponse": resp
    })))
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.get_pending_drafts", skip_all, err)]
pub async fn get_pending_drafts(
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<PendingDraftDto>>, String> {
    let drafts = state.db.get_pending_drafts().map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(drafts))
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.send_draft", skip_all, err)]
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
        sender_id: state.onebot.expected_account().unwrap_or_default().to_string(),
        sender_name: "我 (AI草稿放行)".to_string(),
        content: send_content,
        is_from_me: true,
        ai_reply_status: "draft_pending".to_string(),
        timestamp: now_ms,
    };

    let scope = crate::services::protocol::context::ConversationScope::for_client(&state.onebot, target_type, &draft.target_id)?;
    if let Err(error) = crate::services::protocol::context::save_outgoing_source(&state.db, &scope, &chat_msg, &msg_id_val, "approved_ai_draft") {
        tracing::error!("Approved draft source could not be saved: {}", error);
    }
    let _ = state.db.delete_draft(&draft_id);

    Ok(ApiResponse::ok(serde_json::json!({
        "sentMessageId": chat_msg.id
    })))
}

#[command]
#[tracing::instrument(name = "eazyqq.ipc.dismiss_draft", skip_all, err)]
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
#[tracing::instrument(name = "eazyqq.ipc.regenerate_draft", skip_all, err)]
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

    let inst = custom_instruction.unwrap_or_else(|| "针对对方最后一句重新拟写，保留实际信息，语气自然简短，不强加幽默".to_string());
    let scope = crate::services::protocol::context::ConversationScope::for_client(&state.onebot, &draft.target_type, &draft.target_id)?;
    let context = crate::services::protocol::context::source_message(&state.db, &scope, &draft.reply_to_msg_id);
    let source_exists = state.db.get_message_source(&draft.reply_to_msg_id).map_err(|error| error.to_string())?.is_some();
    let (history, incoming) = match context {
        Ok(message) => crate::services::protocol::pipeline::prepare_context(&state.db, &state.onebot, &message, &draft.target_type, state.ai.current().max_context_messages).await?,
        Err(error) if source_exists => return Err(error),
        Err(_) => (crate::services::protocol::context::build_context_history(&state.db, &scope, None, state.ai.current().max_context_messages)?, crate::services::protocol::context::requested_input(&scope, &draft.incoming_message_snippet)),
    };
    let (new_content, new_thinking) = state
        .ai
        .regenerate_context_reply(
            &history,
            &incoming,
            &draft.target_name,
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
#[tracing::instrument(name = "eazyqq.ipc.trigger_ai_reply", skip_all, err)]
pub async fn trigger_ai_reply(
    state: State<'_, AppState>,
    target_id: String,
    context_snippet: String,
    source_message_id: Option<String>,
    target_type: Option<String>,
) -> Result<ApiResponse<String>, String> {
    let kind = crate::services::protocol::context::target_type(&state.db, &target_id, target_type.as_deref())?;
    let scope = crate::services::protocol::context::ConversationScope::for_client(&state.onebot, &kind, &target_id)?;
    let (chat_history, incoming, name) = if let Some(id) = source_message_id {
        let message = crate::services::protocol::context::source_message(&state.db, &scope, &id)?;
        let (history, incoming) = crate::services::protocol::pipeline::prepare_context(&state.db, &state.onebot, &message, &kind, state.ai.current().max_context_messages).await?;
        (history, incoming, message.sender_name)
    } else {
        (crate::services::protocol::context::build_context_history(&state.db, &scope, None, state.ai.current().max_context_messages)?, crate::services::protocol::context::requested_input(&scope, &context_snippet), "当前会话".to_string())
    };

    let (reply, _thinking) = state
        .ai
        .generate_context_reply(&chat_history, &incoming, &name)
        .await
        .map_err(|e| format!("AI构思回复失败: {}", e))?;

    Ok(ApiResponse::ok(reply))
}
