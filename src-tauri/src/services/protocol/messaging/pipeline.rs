use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::error;

use super::onebot::OneBotClient;
use crate::models::{MessageItemDto, PendingDraftDto};
use crate::services::ai::{AiService, ReplyMessage};
use crate::services::db::Database;
use super::context::{self, ConversationScope};

async fn refresh_history(db: &Database, onebot: &OneBotClient, scope: &ConversationScope, limit: usize) {
    let count = limit.clamp(1, 100).saturating_add(1).min(100) as i32;
    match tokio::time::timeout(std::time::Duration::from_millis(2500), context::load_history(db, onebot, scope, count)).await {
        Ok(Ok(_)) => {},
        Ok(Err(error)) => tracing::warn!("Bound conversation history refresh failed; retaining verified cache: {}", error),
        Err(_) => tracing::warn!("Bound conversation history refresh timed out; retaining verified cache"),
    }
}

fn auto_reply_still_allowed(db: &Database, scope: &ConversationScope, incoming: &MessageItemDto, snapshot: &[String]) -> Result<bool, String> {
    let authorized = db.get_all_rules().map_err(|error| error.to_string())?.iter().any(|rule| {
        let kind = if rule.target_type == "friend" { "private" } else { rule.target_type.as_str() };
        rule.target_id == scope.target_id && kind == scope.target_type && rule.enabled && rule.mode == "auto_reply"
    });
    Ok(authorized && context::auto_reply_snapshot(db, scope, incoming)?.as_deref() == Some(snapshot))
}

pub async fn prepare_context(db: &Database, onebot: &OneBotClient, incoming: &MessageItemDto, target_type: &str, limit: usize) -> Result<(Vec<ReplyMessage>, ReplyMessage), String> {
    let scope = ConversationScope::for_client(onebot, target_type, &incoming.target_id)?;
    refresh_history(db, onebot, &scope, limit).await;
    let source = db.get_message_source(&incoming.id).map_err(|error| error.to_string())?;
    let source = if let Some(source) = source {
        let resolved = match tokio::time::timeout(std::time::Duration::from_secs(8), context::resolve_source(onebot, &scope, &source)).await {
            Ok(result) => result?,
            Err(_) => { let mut source = source; source["resolutionWarning"] = serde_json::json!("读取引用或合并转发超时，未读取的正文不可推断"); source }
        };
        db.save_message_source(&incoming.id, &resolved).map_err(|error| error.to_string())?;
        Some(resolved)
    } else { None };
    let history = context::build_context_history(db, &scope, Some(incoming), limit)?;
    let incoming = context::context_message(&scope, incoming, source.as_ref())?;
    Ok((history, incoming))
}

pub async fn execute_auto_reply(
    app_handle: Option<AppHandle>,
    incoming: MessageItemDto,
    target_type: String,
    db: Arc<Database>,
    onebot: Arc<OneBotClient>,
    ai: Arc<AiService>,
) -> Result<bool, String> {
    let target_id = incoming.target_id.clone();
    let (history, input) = prepare_context(&db, &onebot, &incoming, &target_type, ai.current().max_context_messages).await?;
    let scope = ConversationScope::for_client(&onebot, &target_type, &target_id)?;
    let Some(snapshot) = context::auto_reply_snapshot(&db, &scope, &incoming)? else {
        tracing::info!("Auto reply skipped because its incoming message is no longer current");
        return Ok(false);
    };

    let (reply_content, _thinking) = ai
        .generate_context_reply(&history, &input, &incoming.sender_name)
        .await
        .map_err(|e| {
            error!("AI auto-reply generation failed: {}", e);
            e
        })?;

    if reply_content.is_empty() {
        return Err("大模型返回了空回复".to_string());
    }

    refresh_history(&db, &onebot, &scope, ai.current().max_context_messages).await;
    if !auto_reply_still_allowed(&db, &scope, &incoming, &snapshot)? {
        tracing::info!("Auto reply discarded because authorization or the verified conversation changed");
        return Ok(false);
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
        sender_id: onebot.expected_account().unwrap_or_default().to_string(),
        sender_name: "我 (AI自动秒回)".to_string(),
        content: reply_content,
        is_from_me: true,
        ai_reply_status: "auto_replied".to_string(),
        timestamp: reply_ms,
    };
    context::save_outgoing_source(&db, &scope, &reply_dto, &msg_id_val, "ai_reply")
        .map_err(|error| format!("消息已发送，但来源回执保存失败: {error}"))?;
    if let Some(handle) = app_handle {
        let _ = handle.emit("new-chat-message", &reply_dto);
    }
    Ok(true)
}

pub async fn execute_draft(
    app_handle: Option<AppHandle>,
    incoming: MessageItemDto,
    target_type: String,
    db: Arc<Database>,
    onebot: Arc<OneBotClient>,
    ai: Arc<AiService>,
) -> Result<(), String> {
    let (history, input) = prepare_context(&db, &onebot, &incoming, &target_type, ai.current().max_context_messages).await?;

    let (generated_content, thinking_content) = ai
        .generate_context_reply(&history, &input, &incoming.sender_name)
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
        target_id: incoming.target_id,
        target_name: incoming.sender_name,
        target_type,
        reply_to_msg_id: incoming.id,
        incoming_message_snippet: incoming.content,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn automatic_send_rechecks_revocation_mode_and_conversation_scope() {
        let root=crate::services::logging::workspace_root().join("auto-reply-authorization-snapshot");
        let db=Database::init(root.join("account/eazyqq.db")).unwrap();
        let onebot=OneBotClient::for_account("http://127.0.0.1:9".into(),Some("10001".into()));
        let scope=ConversationScope::for_client(&onebot,"private","20001").unwrap();
        let raw=json!({"message_id":1,"message_type":"private","user_id":20001,"time":1,"message":"incoming"});
        let incoming=context::message_from_protocol(&scope,&raw,0).unwrap();context::store_message(&db,&scope,&incoming,&context::source_record(&scope,"peer_live",&raw),true).unwrap();
        let snapshot=context::auto_reply_snapshot(&db,&scope,&incoming).unwrap().unwrap();
        let mut rule=crate::services::db::ContactRuleRecord { target_id:"20001".into(),target_type:"friend".into(),name:"peer".into(),avatar_url:String::new(),mode:"auto_reply".into(),trigger_condition:"all".into(),keywords:"[]".into(),cooldown_seconds:0,enabled:true,is_summary_whitelist:false,summary_interval_hours:0,updated_at:0 };
        db.upsert_rule(&rule).unwrap();assert!(auto_reply_still_allowed(&db,&scope,&incoming,&snapshot).unwrap());
        rule.enabled=false;db.upsert_rule(&rule).unwrap();assert!(!auto_reply_still_allowed(&db,&scope,&incoming,&snapshot).unwrap());
        rule.enabled=true;rule.mode="copilot".into();db.upsert_rule(&rule).unwrap();assert!(!auto_reply_still_allowed(&db,&scope,&incoming,&snapshot).unwrap());
        rule.mode="auto_reply".into();db.upsert_rule(&rule).unwrap();
        let group_scope=ConversationScope::for_client(&onebot,"group","20001").unwrap();
        assert!(!auto_reply_still_allowed(&db,&group_scope,&incoming,&snapshot).unwrap());
    }
}
