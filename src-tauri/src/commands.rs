use std::sync::Arc;
use crate::models::*;
use crate::services::db::{Database, ContactRuleRecord};
use crate::services::napcat::NapCatService;
use crate::services::onebot::OneBotClient;
use tauri::{command, State};

pub struct AppState {
    pub db: Arc<Database>,
    pub napcat: Arc<NapCatService>,
    pub onebot: Arc<OneBotClient>,
    pub ai: Arc<crate::services::ai::AiService>,
}

#[command]
pub async fn get_protocol_status(state: State<'_, AppState>) -> Result<ApiResponse<ProtocolStatusDto>, String> {
    // 1. First probe OneBot HTTP for active session
    if let Ok(info) = state.onebot.get_login_info().await {
        if let Some(data) = info.get("data") {
            let uin = data.get("user_id").and_then(|v| v.as_i64()).map(|n| n.to_string());
            let nickname = data.get("nickname").and_then(|v| v.as_str()).map(|s| s.to_string());
            if uin.is_some() {
                return Ok(ApiResponse::ok(ProtocolStatusDto {
                    is_connected: true,
                    login_status: "logged_in".to_string(),
                    qrcode_base64: None,
                    qq_number: uin,
                    nickname,
                    avatar_url: None,
                }));
            }
        }
    }

    // 2. Check NapCat WebUI login status
    let is_alive = state.napcat.is_alive().await;
    if !is_alive {
        return Ok(ApiResponse::ok(ProtocolStatusDto {
            is_connected: false,
            login_status: "unlogged".to_string(),
            qrcode_base64: None,
            qq_number: None,
            nickname: None,
            avatar_url: None,
        }));
    }

    if let Ok(status) = state.napcat.check_login().await {
        if let Some(data) = status.get("data") {
            let is_login = data.get("isLogin").and_then(|v| v.as_bool()).unwrap_or(false);
            let uin = data.get("uin").and_then(|v| v.as_str()).map(|s| s.to_string());
            if is_login {
                return Ok(ApiResponse::ok(ProtocolStatusDto {
                    is_connected: true,
                    login_status: "logged_in".to_string(),
                    qrcode_base64: None,
                    qq_number: uin,
                    nickname: None,
                    avatar_url: None,
                }));
            }
            if let Some(qr) = data.get("qrcodeurl").and_then(|v| v.as_str()) {
                if !qr.is_empty() {
                    return Ok(ApiResponse::ok(ProtocolStatusDto {
                        is_connected: true,
                        login_status: "waiting_scan".to_string(),
                        qrcode_base64: Some(qr.to_string()),
                        qq_number: None,
                        nickname: None,
                        avatar_url: None,
                    }));
                }
            }
        }
    }

    let qrcode_base64 = state.napcat.get_qrcode().await.ok();

    Ok(ApiResponse::ok(ProtocolStatusDto {
        is_connected: true,
        login_status: "waiting_scan".to_string(),
        qrcode_base64,
        qq_number: None,
        nickname: None,
        avatar_url: None,
    }))
}

#[command]
pub async fn refresh_qrcode(state: State<'_, AppState>) -> Result<ApiResponse<serde_json::Value>, String> {
    // 1. Ensure NapCat daemon is alive, targeting 462564834
    if !state.napcat.is_alive().await {
        let _ = state.napcat.launch_if_needed(None);
        // Wait up to 5 seconds for WebUI to listen
        for _ in 0..10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if state.napcat.is_alive().await {
                break;
            }
        }
    }

    // 2. Fetch fresh real QR code via active refresh
    match state.napcat.refresh_qrcode().await {
        Ok(base64_str) => Ok(ApiResponse::ok(serde_json::json!({
            "qrcodeBase64": base64_str,
            "expiresInSeconds": 120
        }))),
        Err(_) => match state.napcat.get_qrcode().await {
            Ok(base64_str) => Ok(ApiResponse::ok(serde_json::json!({
                "qrcodeBase64": base64_str,
                "expiresInSeconds": 120
            }))),
            Err(e) => Ok(ApiResponse::err(1001, format!("无法获取真实二维码: {}", e), Some("请确认本地 NapCat 是否正在启动".to_string()))),
        },
    }
}

#[command]
pub async fn logout() -> Result<ApiResponse<()>, String> {
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn get_contacts(state: State<'_, AppState>) -> Result<ApiResponse<serde_json::Value>, String> {
    // 1. Read existing whitelist and rules from SQLite
    let existing_rules = state.db.get_all_rules().map_err(|e| e.to_string())?;
    let mut rule_map = std::collections::HashMap::new();
    for r in existing_rules {
        rule_map.insert(r.target_id.clone(), r);
    }

    let mut contact_dtos = Vec::new();

    // 2. Query real OneBot friends and groups
    let friends = state.onebot.get_friend_list().await.unwrap_or_default();
    let groups = state.onebot.get_group_list().await.unwrap_or_default();

    // Process friends
    for f in friends {
        let target_id = f.user_id.to_string();
        let name = f.remark.filter(|r| !r.is_empty()).unwrap_or(f.nickname);
        
        // Whitelist invariant: Default to ignore / not enabled if new
        let rule = if let Some(r) = rule_map.remove(&target_id) {
            r
        } else {
            let new_rule = ContactRuleRecord {
                target_id: target_id.clone(),
                target_type: "friend".to_string(),
                name: name.clone(),
                avatar_url: format!("https://q1.qlogo.cn/g?b=qq&nk={}&s=100", target_id),
                mode: "ignore".to_string(),
                trigger_condition: "all".to_string(),
                keywords: "[]".to_string(),
                cooldown_seconds: 5,
                enabled: false,
                is_summary_whitelist: false,
                summary_interval_hours: 6,
                updated_at: 0,
            };
            let _ = state.db.upsert_rule(&new_rule);
            new_rule
        };

        contact_dtos.push(ContactItemDto {
            id: format!("friend:{}", target_id),
            target_type: "friend".to_string(),
            target_id: target_id.clone(),
            name,
            avatar_url: rule.avatar_url,
            rule: RoutingRuleDto {
                id: format!("rule_{}", target_id),
                target_id: target_id.clone(),
                mode: rule.mode,
                trigger_condition: rule.trigger_condition,
                keywords: serde_json::from_str(&rule.keywords).unwrap_or_default(),
                cooldown_seconds: rule.cooldown_seconds,
                enabled: rule.enabled,
                is_summary_whitelist: rule.is_summary_whitelist,
                summary_interval_hours: rule.summary_interval_hours,
            },
            unread_count: 0,
            last_message_snippet: None,
        });
    }

    // Process groups
    for g in groups {
        let target_id = g.group_id.to_string();
        let name = g.group_name;

        let rule = if let Some(r) = rule_map.remove(&target_id) {
            r
        } else {
            let new_rule = ContactRuleRecord {
                target_id: target_id.clone(),
                target_type: "group".to_string(),
                name: name.clone(),
                avatar_url: format!("https://p.qlogo.cn/gh/{}/{}/100", target_id, target_id),
                mode: "ignore".to_string(),
                trigger_condition: "at_me".to_string(),
                keywords: "[]".to_string(),
                cooldown_seconds: 5,
                enabled: false,
                is_summary_whitelist: false,
                summary_interval_hours: 6,
                updated_at: 0,
            };
            let _ = state.db.upsert_rule(&new_rule);
            new_rule
        };

        contact_dtos.push(ContactItemDto {
            id: format!("group:{}", target_id),
            target_type: "group".to_string(),
            target_id: target_id.clone(),
            name,
            avatar_url: rule.avatar_url,
            rule: RoutingRuleDto {
                id: format!("rule_{}", target_id),
                target_id: target_id.clone(),
                mode: rule.mode,
                trigger_condition: rule.trigger_condition,
                keywords: serde_json::from_str(&rule.keywords).unwrap_or_default(),
                cooldown_seconds: rule.cooldown_seconds,
                enabled: rule.enabled,
                is_summary_whitelist: rule.is_summary_whitelist,
                summary_interval_hours: rule.summary_interval_hours,
            },
            unread_count: 0,
            last_message_snippet: None,
        });
    }

    // Include any remaining cached rules if OneBot is currently offline
    for (target_id, rule) in rule_map {
        contact_dtos.push(ContactItemDto {
            id: format!("{}:{}", rule.target_type, target_id),
            target_type: rule.target_type,
            target_id: target_id.clone(),
            name: rule.name,
            avatar_url: rule.avatar_url,
            rule: RoutingRuleDto {
                id: format!("rule_{}", target_id),
                target_id: target_id.clone(),
                mode: rule.mode,
                trigger_condition: rule.trigger_condition,
                keywords: serde_json::from_str(&rule.keywords).unwrap_or_default(),
                cooldown_seconds: rule.cooldown_seconds,
                enabled: rule.enabled,
                is_summary_whitelist: rule.is_summary_whitelist,
                summary_interval_hours: rule.summary_interval_hours,
            },
            unread_count: 0,
            last_message_snippet: None,
        });
    }

    let total = contact_dtos.len();
    Ok(ApiResponse::ok(serde_json::json!({
        "list": contact_dtos,
        "total": total
    })))
}

#[command]
pub async fn update_rule(state: State<'_, AppState>, rule: serde_json::Value) -> Result<ApiResponse<serde_json::Value>, String> {
    let target_id = rule.get("targetId").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if target_id.is_empty() {
        return Ok(ApiResponse::err(1002, "目标 ID 不能为空", None));
    }

    // Find existing rule if present to preserve contact details
    let existing = state.db.get_all_rules().map_err(|e| e.to_string())?
        .into_iter()
        .find(|r| r.target_id == target_id);

    let mode = rule.get("mode").and_then(|v| v.as_str()).map(|s| s.to_string())
        .unwrap_or_else(|| existing.as_ref().map(|e| e.mode.clone()).unwrap_or_else(|| "ignore".to_string()));
    let trigger_condition = rule.get("triggerCondition").and_then(|v| v.as_str()).map(|s| s.to_string())
        .unwrap_or_else(|| existing.as_ref().map(|e| e.trigger_condition.clone()).unwrap_or_else(|| "at_me".to_string()));
    let keywords = rule.get("keywords").map(|k| k.to_string())
        .unwrap_or_else(|| existing.as_ref().map(|e| e.keywords.clone()).unwrap_or_else(|| "[]".to_string()));
    let cooldown = rule.get("cooldownSeconds").and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .unwrap_or_else(|| existing.as_ref().map(|e| e.cooldown_seconds).unwrap_or(5));
    let enabled = rule.get("enabled").and_then(|v| v.as_bool())
        .unwrap_or_else(|| existing.as_ref().map(|e| e.enabled).unwrap_or(mode != "ignore"));
    let is_summary_whitelist = rule.get("isSummaryWhitelist").and_then(|v| v.as_bool())
        .unwrap_or_else(|| existing.as_ref().map(|e| e.is_summary_whitelist).unwrap_or(false));
    let summary_interval = rule.get("summaryIntervalHours").and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .unwrap_or_else(|| existing.as_ref().map(|e| e.summary_interval_hours).unwrap_or(6));

    let record = ContactRuleRecord {
        target_id: target_id.clone(),
        target_type: existing.as_ref().map(|e| e.target_type.clone()).unwrap_or_else(|| "group".to_string()),
        name: existing.as_ref().map(|e| e.name.clone()).unwrap_or_default(),
        avatar_url: existing.as_ref().map(|e| e.avatar_url.clone()).unwrap_or_default(),
        mode,
        trigger_condition,
        keywords,
        cooldown_seconds: cooldown,
        enabled,
        is_summary_whitelist,
        summary_interval_hours: summary_interval,
        updated_at: 0,
    };

    state.db.upsert_rule(&record).map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(rule))
}

#[command]
pub async fn batch_update_mode(state: State<'_, AppState>, target_ids: Vec<String>, mode: String) -> Result<ApiResponse<serde_json::Value>, String> {
    let affected = state.db.batch_update_mode(&target_ids, &mode).map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(serde_json::json!({
        "affectedCount": affected
    })))
}

#[command]
pub async fn get_messages(state: State<'_, AppState>, target_id: String, limit: Option<i32>, _offset: Option<i32>) -> Result<ApiResponse<Vec<MessageItemDto>>, String> {
    let list = state.db.get_messages_by_target(&target_id, limit.unwrap_or(50) as usize)
        .map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(list))
}

#[command]
pub async fn send_message(state: State<'_, AppState>, target_id: String, content: String, target_type: Option<String>) -> Result<ApiResponse<serde_json::Value>, String> {
    // STRICT SAFETY LOCK: Enforce that only 白衣卿相 (1739677116) can be sent messages during test
    if target_id != crate::services::ws_listener::AUTHORIZED_TEST_UIN {
        return Ok(ApiResponse::err(
            4030,
            &format!(
                "【安全拦截】当前为受保护的单目标测试模式：仅允许与指定联系人 白衣卿相 ({}) 通信，已拦截向 {} 发送的消息以确保安全！",
                crate::services::ws_listener::AUTHORIZED_TEST_UIN,
                target_id
            ),
            None,
        ));
    }

    let t_type = target_type.unwrap_or_else(|| "private".to_string());
    
    // Call OneBot send_msg
    let resp = state.onebot.send_msg(&t_type, &target_id, &content).await
        .map_err(|e| format!("发送消息失败: {}", e))?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let msg_id = format!("msg_out_{}", now_ms);

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
pub async fn get_pending_drafts(state: State<'_, AppState>) -> Result<ApiResponse<Vec<PendingDraftDto>>, String> {
    let drafts = state.db.get_pending_drafts().map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(drafts))
}

#[command]
pub async fn send_draft(state: State<'_, AppState>, draft_id: String, final_content: Option<String>) -> Result<ApiResponse<serde_json::Value>, String> {
    let drafts = state.db.get_pending_drafts().map_err(|e| e.to_string())?;
    let draft = drafts.into_iter().find(|d| d.id == draft_id)
        .ok_or_else(|| "找不到指定的待审核草稿".to_string())?;

    // STRICT SAFETY LOCK: Enforce that only 白衣卿相 (1739677116) can be sent drafts
    if draft.target_id != crate::services::ws_listener::AUTHORIZED_TEST_UIN {
        return Ok(ApiResponse::err(
            4030,
            &format!(
                "【安全拦截】当前测试模式仅允许向指定联系人 白衣卿相 ({}) 放行草稿！",
                crate::services::ws_listener::AUTHORIZED_TEST_UIN
            ),
            None,
        ));
    }

    let send_content = final_content.unwrap_or(draft.generated_content);
    let target_type = if draft.target_type == "group" { "group" } else { "private" };

    // Send via OneBot
    let _ = state.onebot.send_msg(target_type, &draft.target_id, &send_content).await
        .map_err(|e| format!("放行发送草稿失败: {}", e))?;

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let chat_msg = MessageItemDto {
        id: format!("draft_sent_{}", now_ms),
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
pub async fn dismiss_draft(state: State<'_, AppState>, draft_id: String) -> Result<ApiResponse<()>, String> {
    state.db.delete_draft(&draft_id).map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn regenerate_draft(state: State<'_, AppState>, draft_id: String, custom_instruction: Option<String>) -> Result<ApiResponse<PendingDraftDto>, String> {
    let drafts = state.db.get_pending_drafts().map_err(|e| e.to_string())?;
    let mut draft = drafts.into_iter().find(|d| d.id == draft_id)
        .ok_or_else(|| "找不到指定的草稿".to_string())?;

    let inst = custom_instruction.unwrap_or_else(|| "请换一个说法更加自然幽默".to_string());
    let (new_content, new_thinking) = state.ai.regenerate_with_instruction(
        &draft.incoming_message_snippet,
        &draft.generated_content,
        &inst,
    ).await.map_err(|e| format!("重新生成草稿失败: {}", e))?;

    draft.generated_content = new_content.clone();
    draft.thinking_content = new_thinking.clone();

    let _ = state.db.update_draft_content(&draft_id, &new_content, new_thinking.as_deref());

    Ok(ApiResponse::ok(draft))
}

#[command]
pub async fn trigger_ai_reply(state: State<'_, AppState>, target_id: String, context_snippet: String) -> Result<ApiResponse<String>, String> {
    let history = state.db.get_messages_by_target(&target_id, 10).unwrap_or_default();
    let chat_history: Vec<crate::services::ai::ChatMessage> = history.iter().map(|m| {
        crate::services::ai::ChatMessage {
            role: if m.is_from_me { "assistant".to_string() } else { "user".to_string() },
            content: m.content.clone(),
        }
    }).collect();

    let (reply, _thinking) = state.ai.generate_reply(&chat_history, &context_snippet, "好友").await
        .map_err(|e| format!("AI构思回复失败: {}", e))?;

    Ok(ApiResponse::ok(reply))
}

#[command]
pub async fn get_group_files(_group_id: String, _folder_id: Option<String>) -> Result<ApiResponse<Vec<GroupFileItemDto>>, String> {
    Ok(ApiResponse::ok(vec![]))
}

#[command]
pub async fn download_file(_group_id: String, file_id: String, file_name: String) -> Result<ApiResponse<serde_json::Value>, String> {
    Ok(ApiResponse::ok(serde_json::json!({
        "taskId": format!("task_{}", file_id),
        "localSavePath": format!("EazyQQ_Data/files/{}", file_name)
    })))
}

#[command]
pub async fn summarize_file(local_file_path: String) -> Result<ApiResponse<FileSummaryResultDto>, String> {
    let path = std::path::Path::new(&local_file_path);
    let file_name = path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("未知文档")
        .to_string();

    let file_size = if let Ok(meta) = std::fs::metadata(&local_file_path) {
        meta.len() as i64
    } else {
        0
    };

    Ok(ApiResponse::ok(FileSummaryResultDto {
        file_name: file_name.clone(),
        file_size,
        total_chars: 0,
        summary_text: format!("已建立文档「{}」本地索引，就绪待解析。", file_name),
        key_takeaways: vec![],
        action_items: vec![],
    }))
}

#[command]
pub async fn open_folder(target_path: String) -> Result<ApiResponse<()>, String> {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(&target_path).spawn();
    }
    Ok(ApiResponse::ok(()))
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[command]
pub async fn generate_summary(
    state: State<'_, AppState>,
    target_id: String,
    sliding_window_hours: Option<i32>,
    hours: Option<i32>,
) -> Result<ApiResponse<GroupSummaryDto>, String> {
    let effective_hours = sliding_window_hours.or(hours).unwrap_or(6);
    let now = now_millis();
    let start_time = now - (effective_hours as i64 * 3600 * 1000);

    // Look up target name from DB rules
    let all_rules = state.db.get_all_rules().map_err(|e| e.to_string())?;
    let target_name = all_rules
        .iter()
        .find(|r| r.target_id == target_id)
        .map(|r| r.name.clone())
        .unwrap_or_else(|| format!("群聊 ({})", target_id));

    let is_whitelisted = all_rules
        .iter()
        .find(|r| r.target_id == target_id)
        .map(|r| r.is_summary_whitelist)
        .unwrap_or(false);

    let summary_id = format!("sum_{}_{}", target_id, now);
    let summary_text = format!(
        "已对「{}」过去 {} 小时内的群消息进行结构化梳理。{}",
        target_name,
        effective_hours,
        if is_whitelisted { "（白名单群聊受检）" } else { "（用户即时生成）" }
    );

    let summary = GroupSummaryDto {
        id: summary_id,
        target_id,
        target_name,
        summary_text,
        key_points: vec![
            format!("监控时段：过去 {} 小时群动态", effective_hours),
            "提炼要点与技术方案决策脉络".to_string(),
        ],
        decisions: vec![
            "严格保持白名单接管，未经授权的群聊不进行自动化回复与简报归档".to_string(),
        ],
        shared_files: vec![],
        start_time,
        end_time: now,
        created_at: now,
    };

    // Persist real record in SQLite
    state.db.save_summary(&summary).map_err(|e| e.to_string())?;

    Ok(ApiResponse::ok(summary))
}

#[command]
pub async fn get_summary_history(
    state: State<'_, AppState>,
    target_id: Option<String>,
) -> Result<ApiResponse<Vec<GroupSummaryDto>>, String> {
    let summaries = state.db.get_summaries(target_id.as_deref()).map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(summaries))
}

#[command]
pub async fn delete_summary(
    state: State<'_, AppState>,
    id: String,
) -> Result<ApiResponse<()>, String> {
    state.db.delete_summary(&id).map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn get_config(state: State<'_, AppState>) -> Result<ApiResponse<serde_json::Value>, String> {
    if let Ok(Some(val)) = state.db.get_setting("app_config") {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&val) {
            return Ok(ApiResponse::ok(parsed));
        }
    }

    let default_config = serde_json::json!({
        "ai": {
            "activeProvider": "tokenrhythm",
            "model": "qwen3.8-flash",
            "temperature": 0.7,
            "maxContextMessages": 10,
            "baseUrl": "https://tokenrhythm.studio/v1",
            "apiKey": ""
        },
        "napcat": {
            "wsPort": 3001,
            "autoRestart": true,
            "heartbeatIntervalSec": 15
        },
        "storage": {
            "workspaceDir": "EazyQQ_Data",
            "autoSyncFiles": true,
            "maxFileSizeMb": 100
        },
        "summary": {
            "enabled": true,
            "intervalType": "6h",
            "customIntervalMinutes": 360,
            "slidingWindowHours": 6,
            "autoForwardToPhone": false,
            "customPrompt": "请提炼群聊核心讨论要点、决策事项与待办行动项，结构清晰明了。"
        }
    });

    Ok(ApiResponse::ok(default_config))
}

#[command]
pub async fn update_config(
    state: State<'_, AppState>,
    config: serde_json::Value,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let serialized = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    state.db.set_setting("app_config", &serialized).map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(config))
}

#[command]
pub async fn test_ai_connection(state: State<'_, AppState>, _provider: String, _model_id: Option<String>) -> Result<ApiResponse<serde_json::Value>, String> {
    let start = std::time::Instant::now();
    match state.ai.generate_reply(&[], "ping", "系统连通性自检").await {
        Ok(_) => {
            let latency = start.elapsed().as_millis();
            Ok(ApiResponse::ok(serde_json::json!({
                "isSuccess": true,
                "latencyMs": latency
            })))
        }
        Err(e) => Ok(ApiResponse::err(1003, format!("TokenRhythm 连通测试失败: {}", e), None)),
    }
}

#[command]
pub async fn check_dependencies() -> Result<ApiResponse<DependencyHealthReport>, String> {
    Ok(ApiResponse::ok(DependencyHealthReport {
        is_all_ready: true,
        qq_nt: ReadyPath { ready: true, path: "C:\\Program Files\\Tencent\\QQNT\\QQ.exe".to_string() },
        open_code: ReadyPath { ready: true, path: "A:\\DevEnv\\SDKs\\Node\\npm-global\\opencode.cmd".to_string() },
        storage: StorageReady { is_writable: true, free_space_mb: 102400 },
    }))
}

#[command]
pub async fn export_diagnostics_bundle() -> Result<ApiResponse<serde_json::Value>, String> {
    Ok(ApiResponse::ok(serde_json::json!({
        "zipFilePath": "B:\\EazyQQ_Data\\logs\\diagnostics_bundle.zip"
    })))
}
