use std::sync::Arc;
use std::time::Duration;
use futures_util::StreamExt;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

use crate::models::{MessageItemDto, PendingDraftDto};
use crate::services::ai::{AiService, ChatMessage};
use crate::services::db::Database;
use crate::services::onebot::OneBotClient;

// NOTE: the previous hardcoded `AUTHORIZED_TEST_UIN` gate was removed on purpose.
// Whitelist decisions are now rule-driven and strictly default-deny - see
// `services::policy`. Hardcoding a single authorized account contradicted ROADMAP
// Phase 2 and silently blocked every real target.

/// What the pipeline decided to do with one incoming event. Returned so the CLI can
/// report it without duplicating any logic.
#[derive(Debug, Clone)]
pub struct MessageOutcome {
    /// True when the event was not a message event at all.
    pub not_a_message: bool,
    /// True when the target is not on any whitelist and the event was fully bypassed.
    pub bypassed: bool,
    /// True when the message row was persisted.
    pub recorded: bool,
    pub target_id: String,
    pub mode: String,
    /// `none` | `auto_replied` | `draft_created` | `skipped` | `error`
    pub action: &'static str,
    pub detail: Option<String>,
}

impl MessageOutcome {
    fn ignored() -> Self {
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
            return format!("目标 {} 未加入白名单，完全旁路（未落库、未调用 AI）", self.target_id);
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
                    crate::services::chain::record_ok(
                        crate::services::chain::Link::OneBotWs,
                        "已连接，正在接收消息",
                    );
                    let (_, mut read) = ws_stream.split();

                    while let Some(msg_res) = read.next().await {
                        match msg_res {
                            Ok(msg) => {
                                if msg.is_text() {
                                    if let Ok(text) = msg.into_text() {
                                        if let Ok(json) = serde_json::from_str::<Value>(&text) {
                                            let outcome = handle_onebot_event(
                                                Some(&app_handle),
                                                &json,
                                                &db,
                                                &onebot,
                                                &ai,
                                                false,
                                            )
                                            .await;
                                            // Proof the ingress link is not just open but
                                            // actually delivering events.
                                            crate::services::chain::record_ok(
                                                crate::services::chain::Link::OneBotWs,
                                                format!("已连接，最近事件: {}", outcome.action),
                                            );
                                            tracing::debug!("ws event: {}", outcome.summary());
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("WebSocket stream error: {}, will reconnect...", e);
                                crate::services::chain::record_error(
                                    crate::services::chain::Link::OneBotWs,
                                    format!("连接中断: {}", e),
                                );
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("OneBot WebSocket connect failed ({}), retrying in 3s", e);
                    crate::services::chain::record_error(
                        crate::services::chain::Link::OneBotWs,
                        format!("无法连接 {}: {}", ws_url, e),
                    );
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    });
}

/// Dispatch a raw OneBot event.
///
/// `app_handle` is optional so the exact same code path can be exercised headlessly
/// from `eazyqq-cli simulate`. `inline_ai` awaits the AI work instead of spawning it,
/// which is what the CLI needs to observe the result before exiting.
pub async fn handle_onebot_event(
    app_handle: Option<&AppHandle>,
    event: &Value,
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
    ai: &Arc<AiService>,
    inline_ai: bool,
) -> MessageOutcome {
    let post_type = event.get("post_type").and_then(|v| v.as_str()).unwrap_or("");

    match post_type {
        "message" => {
            handle_message_event(app_handle, event, db, onebot, ai, inline_ai).await
        }
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

/// Group file uploads (ROADMAP Phase 4): record the file and silently download it when
/// the group is whitelisted. Unknown notice types are simply logged.
async fn handle_notice_event(
    _app_handle: Option<&AppHandle>,
    event: &Value,
    db: &Arc<Database>,
    onebot: &Arc<OneBotClient>,
) {
    let notice_type = event.get("notice_type").and_then(|v| v.as_str()).unwrap_or("");
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

            // Download off the event loop so a large file cannot stall the listener.
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
    let message_type = event.get("message_type").and_then(|v| v.as_str()).unwrap_or("");
    let sender = event.get("sender");
    let user_id = event
        .get("user_id")
        .map(|v| v.to_string().replace('\"', ""))
        .unwrap_or_default();
    let sender_name = sender
        .and_then(|s| s.get("card").or_else(|| s.get("nickname")))
        .and_then(|v| v.as_str())
        .unwrap_or("好友")
        .to_string();

    let target_id = if message_type == "group" {
        event
            .get("group_id")
            .map(|v| v.to_string().replace('\"', ""))
            .unwrap_or_else(|| user_id.clone())
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

    // --- Whitelist gate comes FIRST. Anything the user has not explicitly opted in to
    // --- is bypassed completely: no message row, no UI event, no AI call.
    // --- (ROADMAP Phase 2: "未加入白名单的群聊发消息时，软件完全旁路静默".)
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

    // Outgoing messages (our own account) arrive as normal message events; mark them so
    // summaries and AI context can tell the two directions apart.
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

    // Record + broadcast (only reached for whitelisted targets)
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

    // --- Trigger condition: enforce `all` / `at_me` / `keyword`. Without this a group
    // --- configured as `at_me` would be answered on every single message.
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

    // --- Cooldown: pace replies so a burst of messages cannot produce a burst of answers.
    if let Err(wait) = crate::services::cooldown::try_acquire(&target_id, policy.cooldown_seconds) {
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

    let target_type = if message_type == "group" { "group" } else { "private" }.to_string();

    match policy.mode.as_str() {
        "auto_reply" => {
            info!("AI auto-reply triggered for {} [{}]", target_id, policy.describe());
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
            info!("AI draft generation triggered for {} [{}]", target_id, policy.describe());
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

fn build_history(db: &Database, target_id: &str) -> Vec<ChatMessage> {
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

async fn execute_auto_reply(
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

async fn execute_draft(
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
        model_used: "qwen3.8-flash".to_string(),
        created_at: draft_ms,
    };

    db.save_draft(&draft).map_err(|e| format!("草稿落库失败: {}", e))?;
    if let Some(handle) = app_handle {
        let _ = handle.emit("new-draft", &draft);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixture {
        db: Arc<Database>,
        onebot: Arc<OneBotClient>,
        ai: Arc<AiService>,
    }

    /// Deliberately points every endpoint at a dead port: any accidental network call in a
    /// bypass test would show up as an error outcome instead of a clean skip.
    fn fixture(tag: &str) -> Fixture {
        let dir = std::env::temp_dir().join("eazyqq_ws_tests");
        crate::services::logging::ensure_dir(&dir);
        let path = dir.join(format!("{}_{}.db", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);

        Fixture {
            db: Arc::new(Database::init(&path).expect("temp db")),
            onebot: Arc::new(OneBotClient::new("http://127.0.0.1:1".to_string())),
            // Points at a dead port on purpose: any test that reaches the AI step must
            // fail there, which is how we prove the request got past the earlier gates.
            ai: Arc::new(AiService::new(crate::services::ai::AiRuntimeConfig {
                provider: "test".to_string(),
                base_url: "http://127.0.0.1:1/v1".to_string(),
                api_key: String::new(),
                model: "test-model".to_string(),
                ..Default::default()
            })),
        }
    }

    fn group_message(group_id: &str, text: &str) -> Value {
        json!({
            "post_type": "message",
            "message_type": "group",
            "group_id": group_id,
            "user_id": "10001",
            "self_id": "462564834",
            "sender": { "nickname": "测试者", "card": "测试者" },
            "raw_message": text,
            "message": text
        })
    }

    #[tokio::test]
    async fn unwhitelisted_group_is_bypassed_without_touching_storage() {
        let f = fixture("bypass");
        let outcome =
            handle_onebot_event(None, &group_message("555000111", "这条不该被处理"), &f.db, &f.onebot, &f.ai, true)
                .await;

        assert!(outcome.bypassed, "expected a full bypass");
        assert!(!outcome.recorded, "nothing may be persisted");
        assert_eq!(outcome.action, "skipped");
        assert_eq!(
            f.db.get_messages_by_target("555000111", 10).unwrap().len(),
            0,
            "messages_log must stay empty for a non-whitelisted target"
        );
    }

    fn rule(
        target_id: &str,
        mode: &str,
        trigger: &str,
        keywords: &str,
        cooldown: i32,
    ) -> crate::services::db::ContactRuleRecord {
        crate::services::db::ContactRuleRecord {
            target_id: target_id.to_string(),
            target_type: "group".to_string(),
            name: "测试群".to_string(),
            avatar_url: String::new(),
            mode: mode.to_string(),
            trigger_condition: trigger.to_string(),
            keywords: keywords.to_string(),
            cooldown_seconds: cooldown,
            enabled: mode != "ignore",
            is_summary_whitelist: true,
            summary_interval_hours: 2,
            updated_at: 0,
        }
    }

    #[tokio::test]
    async fn whitelisted_target_is_recorded_and_reaches_the_ai_step() {
        let f = fixture("whitelisted");
        f.db.upsert_rule(&rule("1104661022", "copilot", "all", "[]", 0))
            .unwrap();

        let outcome =
            handle_onebot_event(None, &group_message("1104661022", "白名单群的消息"), &f.db, &f.onebot, &f.ai, true)
                .await;

        assert!(!outcome.bypassed);
        assert!(outcome.recorded, "whitelisted messages must be persisted");
        assert_eq!(
            f.db.get_messages_by_target("1104661022", 10).unwrap().len(),
            1
        );
        // The AI call itself fails (no API key / dead endpoint), which is exactly the
        // proof that the request made it past the whitelist gate.
        assert_eq!(outcome.action, "error");
    }

    #[tokio::test]
    async fn at_me_trigger_records_but_never_reaches_the_ai_step() {
        let f = fixture("at_me_skip");
        f.db.upsert_rule(&rule("1104661022", "auto_reply", "at_me", "[]", 0))
            .unwrap();

        let outcome =
            handle_onebot_event(None, &group_message("1104661022", "大家早啊"), &f.db, &f.onebot, &f.ai, true)
                .await;

        assert!(outcome.recorded, "the message is still tracked for summaries");
        assert_eq!(
            outcome.action, "trigger_skipped",
            "a plain group message must not be answered when the rule says at_me"
        );
        assert!(outcome.detail.unwrap().contains("未 @"));
    }

    #[tokio::test]
    async fn at_me_trigger_passes_when_the_account_is_mentioned() {
        let f = fixture("at_me_hit");
        f.db.upsert_rule(&rule("1104661022", "copilot", "at_me", "[]", 0))
            .unwrap();

        let event = json!({
            "post_type": "message",
            "message_type": "group",
            "group_id": "1104661022",
            "user_id": "10001",
            "self_id": "462564834",
            "sender": { "nickname": "测试者" },
            "raw_message": "[CQ:at,qq=462564834] 帮我看下",
            "message": [
                { "type": "at", "data": { "qq": "462564834" } },
                { "type": "text", "data": { "text": " 帮我看下" } }
            ]
        });

        let outcome = handle_onebot_event(None, &event, &f.db, &f.onebot, &f.ai, true).await;
        assert_eq!(
            outcome.action, "error",
            "an @-mention must pass the trigger gate and reach the AI step"
        );
    }

    #[tokio::test]
    async fn keyword_trigger_only_passes_on_a_keyword_hit() {
        let f = fixture("keyword");
        f.db.upsert_rule(&rule("1104661022", "copilot", "keyword", r#"["进度"]"#, 0))
            .unwrap();

        let miss =
            handle_onebot_event(None, &group_message("1104661022", "今天天气不错"), &f.db, &f.onebot, &f.ai, true)
                .await;
        assert_eq!(miss.action, "trigger_skipped");

        let hit =
            handle_onebot_event(None, &group_message("1104661022", "同步一下进度"), &f.db, &f.onebot, &f.ai, true)
                .await;
        assert_eq!(hit.action, "error", "a keyword hit must reach the AI step");
    }

    #[tokio::test]
    async fn cooldown_blocks_a_second_reply_in_the_same_window() {
        let f = fixture("cooldown");
        // A long cooldown guarantees the second message lands inside the window.
        f.db.upsert_rule(&rule("1104661022", "copilot", "all", "[]", 600))
            .unwrap();

        let first =
            handle_onebot_event(None, &group_message("1104661022", "第一条"), &f.db, &f.onebot, &f.ai, true)
                .await;
        assert_eq!(first.action, "error", "the first attempt reaches the AI step");

        let second =
            handle_onebot_event(None, &group_message("1104661022", "第二条"), &f.db, &f.onebot, &f.ai, true)
                .await;
        assert_eq!(second.action, "cooldown", "the second must be rate limited");
        assert!(second.recorded, "rate-limited messages are still tracked");

        // Leave the shared registry clean for other tests.
        crate::services::cooldown::reset("1104661022");
    }

    #[tokio::test]
    async fn own_messages_are_recorded_but_never_trigger_ai() {
        let f = fixture("from_me");
        f.db.upsert_rule(&crate::services::db::ContactRuleRecord {
            target_id: "1104661022".to_string(),
            target_type: "group".to_string(),
            name: "测试群".to_string(),
            avatar_url: String::new(),
            mode: "auto_reply".to_string(),
            trigger_condition: "all".to_string(),
            keywords: "[]".to_string(),
            cooldown_seconds: 5,
            enabled: true,
            is_summary_whitelist: false,
            summary_interval_hours: 6,
            updated_at: 0,
        })
        .unwrap();

        // self_id == user_id => this message came from our own account.
        let event = json!({
            "post_type": "message",
            "message_type": "group",
            "group_id": "1104661022",
            "user_id": "462564834",
            "self_id": "462564834",
            "sender": { "nickname": "我" },
            "raw_message": "我自己发的",
            "message": "我自己发的"
        });

        let outcome = handle_onebot_event(None, &event, &f.db, &f.onebot, &f.ai, true).await;
        assert!(outcome.recorded);
        assert_eq!(outcome.action, "skipped", "our own message must not be answered");
        let stored = f.db.get_messages_by_target("1104661022", 10).unwrap();
        assert_eq!(stored.len(), 1);
        assert!(stored[0].is_from_me);
    }

    #[tokio::test]
    async fn non_message_events_are_ignored() {
        let f = fixture("non_message");
        let event = json!({ "post_type": "meta_event", "meta_event_type": "heartbeat" });
        let outcome = handle_onebot_event(None, &event, &f.db, &f.onebot, &f.ai, true).await;
        assert!(outcome.not_a_message);
        assert_eq!(outcome.action, "skipped");
    }

    #[tokio::test]
    async fn empty_message_bodies_are_dropped() {
        let f = fixture("empty_body");
        let outcome =
            handle_onebot_event(None, &group_message("1104661022", "   "), &f.db, &f.onebot, &f.ai, true).await;
        assert!(!outcome.recorded);
        assert_eq!(outcome.action, "skipped");
    }

    #[test]
    fn outcome_summary_is_human_readable() {
        let bypassed = MessageOutcome {
            not_a_message: false,
            bypassed: true,
            recorded: false,
            target_id: "123".to_string(),
            mode: "ignore".to_string(),
            action: "skipped",
            detail: None,
        };
        assert!(bypassed.summary().contains("未加入白名单"));
        assert!(MessageOutcome::ignored().summary().contains("非消息事件"));
    }
}
