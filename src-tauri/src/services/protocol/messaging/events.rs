use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{error, info};

use super::onebot::OneBotClient;
use super::pipeline::{execute_auto_reply, execute_draft};
use super::context::{self, ConversationScope};
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
            "scheduled" => format!("目标 {} 已进入自动回复处理", self.target_id),
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
        "message" | "message_sent" => handle_message_event(app_handle, event, db, onebot, ai, inline_ai).await,
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
    let expected = onebot.expected_account();
    if expected.is_none() || super::session::account_id(&event["self_id"]).as_deref() != expected {
        let mut outcome = MessageOutcome::ignored();
        outcome.detail = Some("消息账号与固定账号不一致，已忽略".into());
        return outcome;
    }
    let user_id = context::sender_uin(event).unwrap_or_default();
    let sender_name = context::sender_name(event, (!user_id.is_empty()).then_some(user_id.as_str()));
    let target_id = if message_type == "group" {
        context::identifier(&event["group_id"]).unwrap_or_default()
    } else if message_type == "private" && Some(user_id.as_str()) == expected {
        ["target_id", "peer_id", "target_user_id", "recipient_id"].iter()
            .find_map(|key| super::session::account_id(&event[key]).filter(|id| Some(id.as_str()) != expected)).unwrap_or_default()
    } else if message_type == "private" { user_id.clone() } else { String::new() };
    let scope = match ConversationScope::for_client(onebot, message_type, &target_id) {
        Ok(scope) => scope,
        Err(error) => { let mut outcome = MessageOutcome::ignored(); outcome.detail = Some(error); return outcome; }
    };
    let raw_message = context::display_content(event);

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

    let self_id = scope.account_uin.clone();
    let chat_msg = match context::message_from_protocol(&scope, event, now_ms) {
        Ok(message) => message,
        Err(error) => { let mut outcome = MessageOutcome::ignored(); outcome.detail = Some(error); return outcome; }
    };
    let is_from_me = chat_msg.is_from_me;

    match context::store_message(db, &scope, &chat_msg, &context::source_record(&scope, if is_from_me { "owner_live" } else { "peer_live" }, event), true) {
        Ok(false) => return MessageOutcome { not_a_message: false, bypassed: false, recorded: true, target_id, mode: policy.mode, action: "skipped", detail: Some("重复消息事件，未再次触发 AI".into()) },
        Ok(true) => {},
        Err(error) => { error!("cannot persist message source: {}", error); return MessageOutcome { not_a_message: false, bypassed: false, recorded: false, target_id, mode: policy.mode, action: "error", detail: Some("消息及来源保存失败，未调用 AI".into()) }; }
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
                chat_msg.clone(),
                target_type,
                db.clone(),
                onebot.clone(),
                ai.clone(),
            );
            if inline_ai {
                match task.await {
                    Ok(true) => outcome.action = "auto_replied",
                    Ok(false) => { outcome.action = "skipped"; outcome.detail = Some("会话或授权已有更新，已停止过时的自动回复".into()); },
                    Err(e) => {
                        outcome.action = "error";
                        outcome.detail = Some(e);
                    }
                }
            } else {
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = task.await { tracing::error!("Automatic reply processing failed: {}", error); }
                });
                outcome.action = "scheduled";
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
                chat_msg,
                target_type,
                db.clone(),
                onebot.clone(),
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

#[cfg(test)]
mod context_tests {
    use super::*;
    use serde_json::json;

    fn fixture(tag: &str, mode: &str) -> (Arc<Database>, Arc<OneBotClient>, Arc<AiService>) {
        let root=crate::services::logging::workspace_root().join(format!("event-attribution-{tag}"));
        let db=Arc::new(Database::init(root.join("account/eazyqq.db")).unwrap());
        db.upsert_rule(&crate::services::db::ContactRuleRecord { target_id:"20001".into(),target_type:"friend".into(),name:"peer".into(),avatar_url:String::new(),mode:mode.into(),trigger_condition:"all".into(),keywords:"[]".into(),cooldown_seconds:0,enabled:true,is_summary_whitelist:false,summary_interval_hours:0,updated_at:0 }).unwrap();
        (db,Arc::new(OneBotClient::for_account("http://127.0.0.1:9".into(),Some("10001".into()))),Arc::new(AiService::for_account(Default::default(),root.join("account"))))
    }

    #[tokio::test]
    async fn manual_owner_message_sent_keeps_its_peer_target_without_ai() {
        for key in ["target_id","peer_id","recipient_id"] {
            let (db,onebot,ai)=fixture(key,"auto_reply");
            let mut raw=json!({"post_type":"message_sent","message_type":"private","self_id":10001,"user_id":10001,"message_id":71,"sender":{"user_id":10001,"nickname":"Owner"},"message":[{"type":"face","data":{"id":"76"}}]});
            raw[key]=json!(20001);
            let outcome=handle_onebot_event(None,&raw,&db,&onebot,&ai,true).await;
            assert!(outcome.recorded);assert_eq!(outcome.action,"skipped");assert!(outcome.detail.unwrap().contains("不触发 AI"));
            let saved=db.get_messages_by_target("20001",10).unwrap();assert_eq!(saved.len(),1);assert!(saved[0].is_from_me);assert_eq!(saved[0].content,"[CQ:face,id=76]");
        }
    }

    #[tokio::test]
    async fn foreign_live_event_is_ignored_and_replay_does_not_create_another_context() {
        let (db,onebot,ai)=fixture("replay","summary_only");
        let mut raw=json!({"post_type":"message","message_type":"private","self_id":10002,"user_id":20001,"message_id":81,"sender":{"user_id":20001,"nickname":"我"},"raw_message":"相同内容"});
        assert!(!handle_onebot_event(None,&raw,&db,&onebot,&ai,true).await.recorded);
        raw["self_id"]=json!(10001);
        assert!(handle_onebot_event(None,&raw,&db,&onebot,&ai,true).await.recorded);
        let second=handle_onebot_event(None,&raw,&db,&onebot,&ai,true).await;
        assert_eq!(second.action,"skipped");assert!(second.detail.unwrap().contains("重复消息"));
        let saved=db.get_messages_by_target("20001",10).unwrap();assert_eq!(saved.len(),1);assert!(!saved[0].is_from_me);
    }
}
