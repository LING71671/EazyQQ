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

    let kind = crate::services::protocol::context::target_type(
        &state.db,
        &target_id,
        target_type.as_deref(),
    )?;
    let scope = crate::services::protocol::context::ConversationScope::for_client(
        &state.onebot,
        &kind,
        &target_id,
    )?;
    let list = crate::services::protocol::context::load_history(
        &state.db,
        &state.onebot,
        &scope,
        req_limit,
    )
    .await?;
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
        sender_id: state
            .onebot
            .expected_account()
            .unwrap_or_default()
            .to_string(),
        sender_name: "我".to_string(),
        content,
        is_from_me: true,
        ai_reply_status: "none".to_string(),
        timestamp: now_ms,
    };

    let scope = crate::services::protocol::context::ConversationScope::for_client(
        &state.onebot,
        &t_type,
        &target_id,
    )?;
    if let Err(error) = crate::services::protocol::context::save_outgoing_source(
        &state.db,
        &scope,
        &chat_msg,
        &msg_id_val,
        "owner_manual",
    ) {
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
    final_bubbles: Option<Vec<String>>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let report = crate::services::protocol::reply_workflow::approve_draft(
        &state.db,
        &state.onebot,
        &draft_id,
        final_content,
        final_bubbles,
    )
    .await?;
    Ok(ApiResponse::ok(
        serde_json::to_value(report).map_err(|error| error.to_string())?,
    ))
}

#[command]
pub async fn manage_reply_batches(
    state: State<'_, AppState>,
    request: serde_json::Value,
) -> Result<ApiResponse<serde_json::Value>, String> {
    crate::services::protocol::reply_workflow::manage_batches(&state.db, &state.onebot, request)
        .map(ApiResponse::ok)
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
    let instruction = custom_instruction.unwrap_or_else(|| {
        "针对对方最后一句重新拟写，保留实际信息，语气自然简短，不强加幽默".into()
    });
    crate::services::protocol::reply_workflow::regenerate(
        &state.db,
        &state.onebot,
        &state.ai,
        &draft_id,
        &instruction,
    )
    .await
    .map(ApiResponse::ok)
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
    let (plan, _) = crate::services::protocol::reply_workflow::suggest(
        &state.db,
        &state.onebot,
        &state.ai,
        &target_id,
        target_type.as_deref(),
        &context_snippet,
        source_message_id.as_deref(),
    )
    .await?;
    Ok(ApiResponse::ok(plan.preview()))
}
