use std::sync::Arc;
use tauri::{AppHandle, Emitter};

use super::context::{self, ConversationScope};
use super::onebot::OneBotClient;
use super::reply_workflow;
use crate::models::{MessageItemDto, PendingDraftDto};
use crate::services::ai::persona_settings;
use crate::services::ai::{AiService, ReplyMessage};
use crate::services::db::Database;
use crate::services::workflows::replies::{self, DeliveryRequest};

pub(super) async fn refresh_history(
    db: &Database,
    onebot: &OneBotClient,
    scope: &ConversationScope,
    limit: usize,
) {
    let count = limit.clamp(1, 100).saturating_add(1).min(100) as i32;
    match tokio::time::timeout(
        std::time::Duration::from_millis(2500),
        context::load_history(db, onebot, scope, count),
    )
    .await
    {
        Ok(Ok(_)) => {}
        Ok(Err(error)) => tracing::warn!(
            "Bound conversation history refresh failed; retaining verified cache: {}",
            error
        ),
        Err(_) => {
            tracing::warn!("Bound conversation history refresh timed out; retaining verified cache")
        }
    }
}

fn auto_reply_still_allowed(
    db: &Database,
    scope: &ConversationScope,
    incoming: &MessageItemDto,
    snapshot: &[String],
) -> Result<bool, String> {
    let authorized = db
        .get_all_rules()
        .map_err(|error| error.to_string())?
        .iter()
        .any(|rule| {
            let kind = if rule.target_type == "friend" {
                "private"
            } else {
                rule.target_type.as_str()
            };
            rule.target_id == scope.target_id
                && kind == scope.target_type
                && rule.enabled
                && rule.mode == "auto_reply"
        });
    Ok(authorized
        && context::auto_reply_snapshot(db, scope, incoming)?.as_deref() == Some(snapshot))
}

pub async fn prepare_context(
    db: &Database,
    onebot: &OneBotClient,
    incoming: &MessageItemDto,
    target_type: &str,
    limit: usize,
) -> Result<(Vec<ReplyMessage>, ReplyMessage), String> {
    let scope = ConversationScope::for_client(onebot, target_type, &incoming.target_id)?;
    refresh_history(db, onebot, &scope, limit).await;
    let source = db
        .get_message_source(&incoming.id)
        .map_err(|error| error.to_string())?;
    let source = if let Some(source) = source {
        let resolved = match tokio::time::timeout(
            std::time::Duration::from_secs(8),
            context::resolve_source(onebot, &scope, &source),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                let mut source = source;
                source["resolutionWarning"] =
                    serde_json::json!("读取引用或合并转发超时，未读取的正文不可推断");
                source
            }
        };
        db.save_message_source(&incoming.id, &resolved)
            .map_err(|error| error.to_string())?;
        Some(resolved)
    } else {
        None
    };
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
    let (history, input) = prepare_context(
        &db,
        &onebot,
        &incoming,
        &target_type,
        ai.current().max_context_messages,
    )
    .await?;
    let scope = ConversationScope::for_client(&onebot, &target_type, &target_id)?;
    let Some(snapshot) = context::auto_reply_snapshot(&db, &scope, &incoming)? else {
        tracing::info!("Auto reply skipped because its incoming message is no longer current");
        return Ok(false);
    };

    let persona = persona_settings::resolve(&db, &scope, Some(&incoming.sender_id))?;
    let Some(policy) = reply_workflow::policy_snapshot(&db, &scope, true)? else {
        return Ok(false);
    };
    let batch_id = reply_workflow::new_id("auto")?;
    let service = ai.for_request(&batch_id)?;
    let ticket = ai.reserve_inference_sequence();
    let generated = service
        .generate_reply_plan(&history, &input, &incoming.sender_name, &persona.text, None)
        .await;
    ai.observe_scoped_inference(&service.current(), ticket, &generated);
    let (plan, thinking) = generated?;
    if service.current() != ai.current() {
        return Ok(false);
    }

    refresh_history(&db, &onebot, &scope, ai.current().max_context_messages).await;
    if !auto_reply_still_allowed(&db, &scope, &incoming, &snapshot)? {
        tracing::info!(
            "Auto reply discarded because authorization or the verified conversation changed"
        );
        return Ok(false);
    }

    let report = replies::deliver(
        &db,
        &onebot,
        DeliveryRequest {
            batch_id: batch_id.clone(),
            target_type: target_type.clone(),
            target_id: target_id.clone(),
            source_message_id: incoming.id.clone(),
            input_snapshot: snapshot.clone(),
            authorization: "auto".into(),
            plan: plan.clone(),
        },
        |sent_ids| {
            let db = &db;
            let onebot = &onebot;
            let scope = scope.clone();
            let incoming = &incoming;
            let snapshot = &snapshot;
            let policy = &policy;
            let service = &service;
            let ai = &ai;
            async move {
                refresh_history(db, onebot, &scope, 100).await;
                Ok(
                    reply_workflow::policy_snapshot(db, &scope, true)?.as_ref() == Some(policy)
                        && persona_settings::resolve(db, &scope, Some(&incoming.sender_id))?
                            .revision
                            == persona.revision
                        && service.current() == ai.current()
                        && context::reply_snapshot_ignoring(db, &scope, incoming, &sent_ids)?
                            .as_ref()
                            == Some(snapshot),
                )
            }
        },
    )
    .await?;
    if report.status != "sent" {
        let draft = PendingDraftDto {
            id: batch_id,
            target_id: incoming.target_id.clone(),
            target_name: incoming.sender_name.clone(),
            target_type: target_type.clone(),
            reply_to_msg_id: incoming.id.clone(),
            incoming_message_snippet: incoming.content.clone(),
            generated_content: plan.preview(),
            reply_plan: Some(plan),
            thinking_content: thinking,
            model_used: service.model(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        };
        reply_workflow::save_plan_draft(&db, &onebot, &draft, snapshot, persona.revision)?;
        if let Some(handle) = &app_handle {
            let _ = handle.emit("new-draft", &draft);
        }
    }
    if let Some(handle) = app_handle {
        for message in &report.messages {
            let _ = handle.emit("new-chat-message", message);
        }
    }
    Ok(report.status == "sent")
}

pub async fn execute_draft(
    app_handle: Option<AppHandle>,
    incoming: MessageItemDto,
    target_type: String,
    db: Arc<Database>,
    onebot: Arc<OneBotClient>,
    ai: Arc<AiService>,
) -> Result<(), String> {
    let (history, input) = prepare_context(
        &db,
        &onebot,
        &incoming,
        &target_type,
        ai.current().max_context_messages,
    )
    .await?;
    let scope = ConversationScope::for_client(&onebot, &target_type, &incoming.target_id)?;
    let Some(snapshot) = context::auto_reply_snapshot(&db, &scope, &incoming)? else {
        return Ok(());
    };
    let persona = persona_settings::resolve(&db, &scope, Some(&incoming.sender_id))?;
    let Some(policy) = reply_workflow::policy_snapshot(&db, &scope, false)? else {
        return Ok(());
    };
    let draft_id = reply_workflow::new_id("draft")?;
    let service = ai.for_request(&draft_id)?;
    let ticket = ai.reserve_inference_sequence();
    let generated = service
        .generate_reply_plan(&history, &input, &incoming.sender_name, &persona.text, None)
        .await;
    ai.observe_scoped_inference(&service.current(), ticket, &generated);
    let (plan, thinking_content) = generated?;
    if service.current() != ai.current()
        || reply_workflow::policy_snapshot(&db, &scope, false)?.as_ref() != Some(&policy)
        || persona_settings::resolve(&db, &scope, None)?.revision != persona.revision
    {
        return Ok(());
    }

    let draft_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let draft = PendingDraftDto {
        id: draft_id,
        target_id: incoming.target_id,
        target_name: incoming.sender_name,
        target_type,
        reply_to_msg_id: incoming.id,
        incoming_message_snippet: incoming.content,
        generated_content: plan.preview(),
        reply_plan: Some(plan),
        thinking_content,
        model_used: ai.model(),
        created_at: draft_ms,
    };

    reply_workflow::save_plan_draft(&db, &onebot, &draft, snapshot, persona.revision)
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
        let root =
            crate::services::logging::workspace_root().join("auto-reply-authorization-snapshot");
        let db = Database::init(root.join("account/eazyqq.db")).unwrap();
        let onebot = OneBotClient::for_account("http://127.0.0.1:9".into(), Some("10001".into()));
        let scope = ConversationScope::for_client(&onebot, "private", "20001").unwrap();
        let raw = json!({"message_id":1,"message_type":"private","user_id":20001,"time":1,"message":"incoming"});
        let incoming = context::message_from_protocol(&scope, &raw, 0).unwrap();
        context::store_message(
            &db,
            &scope,
            &incoming,
            &context::source_record(&scope, "peer_live", &raw),
            true,
        )
        .unwrap();
        let snapshot = context::auto_reply_snapshot(&db, &scope, &incoming)
            .unwrap()
            .unwrap();
        let mut rule = crate::services::db::ContactRuleRecord {
            target_id: "20001".into(),
            target_type: "friend".into(),
            name: "peer".into(),
            avatar_url: String::new(),
            mode: "auto_reply".into(),
            trigger_condition: "all".into(),
            keywords: "[]".into(),
            cooldown_seconds: 0,
            enabled: true,
            is_summary_whitelist: false,
            summary_interval_hours: 0,
            updated_at: 0,
        };
        db.upsert_rule(&rule).unwrap();
        assert!(auto_reply_still_allowed(&db, &scope, &incoming, &snapshot).unwrap());
        rule.enabled = false;
        db.upsert_rule(&rule).unwrap();
        assert!(!auto_reply_still_allowed(&db, &scope, &incoming, &snapshot).unwrap());
        rule.enabled = true;
        rule.mode = "copilot".into();
        db.upsert_rule(&rule).unwrap();
        assert!(!auto_reply_still_allowed(&db, &scope, &incoming, &snapshot).unwrap());
        rule.mode = "auto_reply".into();
        db.upsert_rule(&rule).unwrap();
        let group_scope = ConversationScope::for_client(&onebot, "group", "20001").unwrap();
        assert!(!auto_reply_still_allowed(&db, &group_scope, &incoming, &snapshot).unwrap());
    }
}
