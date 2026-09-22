use std::sync::Arc;
use crate::models::*;
use crate::services::db::{Database, ContactRuleRecord};
use crate::services::napcat::NapCatService;
use crate::services::onebot::OneBotClient;
use tauri::{command, Manager, State};
use serde::{Serialize, Deserialize};

pub struct AppState {
    pub db: Arc<Database>,
    pub napcat: Arc<NapCatService>,
    pub onebot: Arc<OneBotClient>,
    pub ai: Arc<crate::services::ai::AiService>,
}

/// QQ exposes avatars by account number; NapCat's login payload does not include one, so
/// derive it here to satisfy the Phase 1 "avatar + nickname echo" acceptance item.
fn qq_avatar(uin: Option<&String>) -> Option<String> {
    uin.filter(|s| !s.is_empty())
        .map(|s| format!("https://q1.qlogo.cn/g?b=qq&nk={}&s=100", s))
}

use std::sync::atomic::{AtomicI64, Ordering};
static LAST_AUTO_QUICK_LOGIN_TIME: AtomicI64 = AtomicI64::new(0);

#[command]
pub async fn get_protocol_status(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<ApiResponse<ProtocolStatusDto>, String> {
    // 1. First probe OneBot HTTP for active session
    if let Ok(info) = state.onebot.get_login_info().await {
        if let Some(data) = info.get("data") {
            let uin = data.get("user_id").and_then(|v| v.as_i64()).map(|n| n.to_string());
            let nickname = data.get("nickname").and_then(|v| v.as_str()).map(|s| s.to_string());
            if let Some(observed) = &uin {
                reconcile_account(&app, observed);
            }
            if uin.is_some() {
                let avatar_url = qq_avatar(uin.as_ref());
                return Ok(ApiResponse::ok(ProtocolStatusDto {
                    is_connected: true,
                    login_status: "logged_in".to_string(),
                    qrcode_base64: None,
                    qrcode_error: None,
                    qq_number: uin,
                    nickname,
                    avatar_url,
                    quick_login_accounts: None,
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
            qrcode_error: Some(
                "NapCat 协议端启动加载中，正在准备本地运行环境...".to_string(),
            ),
            qq_number: None,
            nickname: None,
            avatar_url: None,
            quick_login_accounts: None,
        }));
    }

    // 3. Probe available remembered quick-login accounts
    let mut quick_login_accounts: Vec<QuickLoginAccountDto> = Vec::new();
    if let Ok(res) = state.napcat.get_quick_login_list().await {
        if let Some(arr) = res.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(u) = item.get("uin").and_then(|v| v.as_str()) {
                    let nick = item.get("nickName").and_then(|v| v.as_str()).unwrap_or(u);
                    let face = item.get("faceUrl").and_then(|v| v.as_str()).map(|s| s.to_string())
                        .or_else(|| qq_avatar(Some(&u.to_string())));
                    quick_login_accounts.push(QuickLoginAccountDto {
                        uin: u.to_string(),
                        nickname: nick.to_string(),
                        face_url: face,
                    });
                }
            }
        }
    }

    if let Ok(status) = state.napcat.check_login().await {
        if let Some(data) = status.get("data") {
            let is_login = data.get("isLogin").and_then(|v| v.as_bool()).unwrap_or(false);
            let uin = data.get("uin").and_then(|v| v.as_str()).map(|s| s.to_string());
            if is_login {
                let avatar_url = qq_avatar(uin.as_ref());
                return Ok(ApiResponse::ok(ProtocolStatusDto {
                    is_connected: true,
                    login_status: "logged_in".to_string(),
                    qrcode_base64: None,
                    qrcode_error: None,
                    qq_number: uin,
                    nickname: None,
                    avatar_url,
                    quick_login_accounts: if quick_login_accounts.is_empty() { None } else { Some(quick_login_accounts) },
                }));
            }
        }
    }

    // 4. Auto-trigger quick login if remembered accounts exist (defaults to last bound account e.g. 462564834)
    if !quick_login_accounts.is_empty() {
        let last_acc = crate::services::accounts::read_bootstrap().last_account;
        let candidate = last_acc
            .as_deref()
            .and_then(|target| quick_login_accounts.iter().find(|a| a.uin == target))
            .unwrap_or(&quick_login_accounts[0]);

        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let last_attempt = LAST_AUTO_QUICK_LOGIN_TIME.load(Ordering::Relaxed);

        if now_sec - last_attempt > 10 {
            LAST_AUTO_QUICK_LOGIN_TIME.store(now_sec, Ordering::Relaxed);
            tracing::info!(
                "get_protocol_status: auto-triggering quick login for {} ({})",
                candidate.nickname,
                candidate.uin
            );
            if let Ok(ql_res) = state.napcat.set_quick_login(&candidate.uin).await {
                if ql_res.get("code").and_then(|c| c.as_i64()) == Some(0) {
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                    if let Ok(info) = state.onebot.get_login_info().await {
                        if let Some(data) = info.get("data") {
                            let uin = data.get("user_id").and_then(|v| v.as_i64()).map(|n| n.to_string());
                            let nickname = data.get("nickname").and_then(|v| v.as_str()).map(|s| s.to_string());
                            if let Some(observed) = &uin {
                                reconcile_account(&app, observed);
                            }
                            if uin.is_some() {
                                let avatar_url = qq_avatar(uin.as_ref());
                                return Ok(ApiResponse::ok(ProtocolStatusDto {
                                    is_connected: true,
                                    login_status: "logged_in".to_string(),
                                    qrcode_base64: None,
                                    qrcode_error: None,
                                    qq_number: uin,
                                    nickname,
                                    avatar_url,
                                    quick_login_accounts: Some(quick_login_accounts),
                                }));
                            }
                        }
                    }
                }
            }
        }
    }

    let (qrcode_base64, qrcode_error) = match state.napcat.get_qrcode().await {
        Ok(qr) => (Some(qr), None),
        Err(_) => (
            None,
            Some("NapCat 协议端启动加载中，正在准备登录二维码与凭据...".to_string()),
        ),
    };

    Ok(ApiResponse::ok(ProtocolStatusDto {
        is_connected: true,
        login_status: "waiting_scan".to_string(),
        qrcode_base64,
        qrcode_error,
        qq_number: None,
        nickname: None,
        avatar_url: None,
        quick_login_accounts: if quick_login_accounts.is_empty() { None } else { Some(quick_login_accounts) },
    }))
}

#[command]
pub async fn quick_login(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    uin: String,
) -> Result<ApiResponse<()>, String> {
    tracing::info!("quick_login requested for {}", uin);
    let res = state.napcat.set_quick_login(&uin).await.map_err(|e| e.to_string())?;
    let code = res.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 0 {
        let msg = res.get("message").and_then(|m| m.as_str()).unwrap_or("快速登录失败");
        return Ok(ApiResponse::err(1002, msg, Some("请确认手机 QQ 或该账号仍有效".to_string())));
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
    reconcile_account(&app, &uin);
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn get_quick_login_accounts(
    state: State<'_, AppState>,
) -> Result<ApiResponse<Vec<QuickLoginAccountDto>>, String> {
    let mut list = Vec::new();
    if let Ok(res) = state.napcat.get_quick_login_list().await {
        if let Some(arr) = res.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(u) = item.get("uin").and_then(|v| v.as_str()) {
                    let nick = item.get("nickName").and_then(|v| v.as_str()).unwrap_or(u);
                    let face = item.get("faceUrl").and_then(|v| v.as_str()).map(|s| s.to_string())
                        .or_else(|| qq_avatar(Some(&u.to_string())));
                    list.push(QuickLoginAccountDto {
                        uin: u.to_string(),
                        nickname: nick.to_string(),
                        face_url: face,
                    });
                }
            }
        }
    }
    Ok(ApiResponse::ok(list))
}

#[command]
pub async fn refresh_qrcode(state: State<'_, AppState>) -> Result<ApiResponse<serde_json::Value>, String> {
    // 1. Make sure the protocol side is up, and keep the reason if it will not start.
    let mut launch_error: Option<String> = None;
    if !state.napcat.is_alive().await {
        if let Err(e) = state.napcat.launch_if_needed() {
            launch_error = Some(e);
        }
        // Wait up to 5 seconds for the WebUI to start listening.
        for _ in 0..10 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            if state.napcat.is_alive().await {
                break;
            }
        }
    }

    // 2. Ask NapCat for a genuinely new code.
    //
    // A failure is reported, not papered over. This used to fall back to `get_qrcode()`,
    // which returns whatever code NapCat last issued - so pressing "refresh" handed back the
    // same image while the UI announced "最新有效二维码已就绪" for a code that was already
    // stale, and the real reason was discarded with `Err(_)`. The caller keeps showing the
    // previous image; what it must not do is claim the code is fresh when it is not.
    match state.napcat.refresh_qrcode().await {
        Ok(base64_str) => Ok(ApiResponse::ok(serde_json::json!({
            "qrcodeBase64": base64_str,
            "expiresInSeconds": 120
        }))),
        Err(e) => {
            let mut reason = format!("无法获取全新二维码: {}", e);
            if let Some(note) = launch_error {
                reason.push_str(&format!("；协议端未能启动: {}", note));
            }
            Ok(ApiResponse::err(
                1001,
                reason,
                Some("请确认 NapCat 正在运行，且 QQ 安装路径正确".to_string()),
            ))
        }
    }
}

#[command]
pub async fn logout() -> Result<ApiResponse<()>, String> {
    // NapCat's WebUI exposes no logout route (its full route table is: QQLogin
    // CheckLoginStatus / GetQQLoginInfo / GetQQLoginQrcode / GetQuickLogin* /
    // SetQuickLogin*, plus base / auth / OB11Config / Log / File / WebUIConfig).
    // Returning Ok(()) here would be a lie - the session would stay alive.
    tracing::warn!("logout requested but NapCat provides no logout endpoint");
    Ok(ApiResponse::err(
        4050,
        "当前协议端 (NapCat) 未提供登出接口，无法在应用内注销。\
         如需切换账号，请在 NapCat WebUI (http://127.0.0.1:6099) 中退出登录，\
         或停止协议端进程后使用「快速登录」切换到其它账号。",
        None,
    ))
}

/// Full end-to-end link report for the frontend.
///
/// Returns every hop with its health, what breaks if it is down, and - most usefully -
/// the first break in pipeline order, since in a chain everything after the first failure
/// is a consequence rather than a separate fault.
#[command]
pub async fn get_chain_status() -> Result<ApiResponse<serde_json::Value>, String> {
    let links = crate::services::chain::snapshot();
    let first = crate::services::chain::first_break();

    Ok(ApiResponse::ok(serde_json::json!({
        "links": links,
        "firstBreak": first,
        "hasFailure": crate::services::chain::has_failure(),
        "uptimeSecs": crate::services::chain::uptime_secs(),
    })))
}

/// Keep this process's data directory aligned with the account actually signed in.
///
/// Every private path - database, log file, downloaded group files, diagnostics - is
/// computed once at startup from the recorded account. If the protocol side reports a
/// different one, carrying on would write this account's messages, drafts and logs into
/// the previous account's directory, which is exactly the cross-account leak we must not
/// have. So record the account and restart, which recomputes every path.
///
/// The restart happens once: after it, the recorded account matches the observed one.
fn reconcile_account(app: &tauri::AppHandle, observed: &str) {
    let observed = observed.trim();
    if observed.is_empty() {
        return;
    }

    match crate::services::accounts::active() {
        Some(current) if current == observed => {}
        Some(current) => {
            tracing::warn!(
                "account switched from {} to {}; restarting so each account keeps its own \
                 data directory",
                current,
                observed
            );
            if let Err(e) = crate::services::accounts::adopt(observed) {
                tracing::error!("could not record the new account: {} - not restarting", e);
                return;
            }
            app.restart();
        }
        None => {
            // First time the account is known: a fresh install, or data written before
            // accounts were separated. Restarting now means the very first session already
            // writes into the right directory instead of the unbound one.
            tracing::info!(
                "adopting account {} so its data is isolated from other accounts",
                observed
            );
            if let Err(e) = crate::services::accounts::adopt(observed) {
                tracing::warn!("could not record the account: {}", e);
                return;
            }
            app.restart();
        }
    }
}

/// Mark a conversation as read, clearing its unread badge.
///
/// `at` is a millisecond timestamp; the frontend passes the newest message it has
/// rendered, and the database keeps the maximum so out-of-order calls cannot un-read.
#[command]
pub async fn mark_read(
    state: State<'_, AppState>,
    target_id: String,
    at: Option<i64>,
) -> Result<ApiResponse<i64>, String> {
    let at_ms = at.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    });

    state
        .db
        .mark_read(&target_id, at_ms)
        .map_err(|e| e.to_string())?;

    tracing::debug!("marked {} read up to {}", target_id, at_ms);
    Ok(ApiResponse::ok(at_ms))
}

#[command]
pub async fn get_contacts(state: State<'_, AppState>) -> Result<ApiResponse<serde_json::Value>, String> {
    // Proof of life for the frontend: if the UI is up at all, it calls this on mount.
    // Recorded into the chain monitor because a blank window and a working window are
    // indistinguishable from the outside - this is the only reliable signal that the
    // WebView actually executed the app.
    tracing::info!("IPC: get_contacts called from the frontend");
    crate::services::chain::record_ok(
        crate::services::chain::Link::Frontend,
        "前端已加载并调用后端",
    );

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
                // 0 = follow the global cadence (see scheduler::resolve_interval_hours).
                summary_interval_hours: 0,
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
            unread_count: state.db.count_unread(&target_id).unwrap_or(0) as i32,
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
                // 0 = follow the global cadence (see scheduler::resolve_interval_hours).
                summary_interval_hours: 0,
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
            unread_count: state.db.count_unread(&target_id).unwrap_or(0) as i32,
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
            unread_count: state.db.count_unread(&target_id).unwrap_or(0) as i32,
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
        state.db.get_all_rules().ok()
            .and_then(|rules| rules.into_iter().find(|r| r.target_id == target_id))
            .map(|r| r.target_type == "group")
            .unwrap_or(false)
    };

    let ob_msgs = if is_group {
        state.onebot.get_group_msg_history(&target_id, req_limit).await.unwrap_or_default()
    } else {
        let friend_res = state.onebot.get_friend_msg_history(&target_id, req_limit).await.unwrap_or_default();
        if friend_res.is_empty() {
            state.onebot.get_group_msg_history(&target_id, req_limit).await.unwrap_or_default()
        } else {
            friend_res
        }
    };

    for item in ob_msgs {
        let raw_msg = item.get("raw_message").and_then(|v| v.as_str()).unwrap_or("");
        if raw_msg.trim().is_empty() {
            continue;
        }
        let time_sec = item.get("time").and_then(|v| v.as_i64()).unwrap_or(0);
        let time_ms = if time_sec > 10_000_000_000 { time_sec } else { time_sec * 1000 };
        let msg_id_val = item.get("message_id")
            .map(|v| v.to_string().replace('"', ""))
            .unwrap_or_else(|| format!("ob_{}", time_ms));
        let unique_id = format!("msg_{}_{}", target_id, msg_id_val);

        let sender = item.get("sender");
        let sender_id = item.get("user_id")
            .or_else(|| sender.and_then(|s| s.get("user_id")))
            .map(|v| v.to_string().replace('"', ""))
            .unwrap_or_default();
        let self_id = item.get("self_id")
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
    let list = state.db.get_messages_by_target(&target_id, req_limit as usize)
        .map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(list))
}

#[command]
pub async fn send_message(state: State<'_, AppState>, target_id: String, content: String, target_type: Option<String>) -> Result<ApiResponse<serde_json::Value>, String> {
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
    let resp = state.onebot.send_msg(&t_type, &target_id, &content).await
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
pub async fn get_pending_drafts(state: State<'_, AppState>) -> Result<ApiResponse<Vec<PendingDraftDto>>, String> {
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
    let draft = drafts.into_iter().find(|d| d.id == draft_id)
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
    let target_type = if draft.target_type == "group" { "group" } else { "private" };

    tracing::info!(
        "draft {} approved -> sending to {} ({})",
        draft_id,
        draft.target_id,
        target_type
    );

    // Send via OneBot
    let send_res = state.onebot.send_msg(target_type, &draft.target_id, &send_content).await
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
pub async fn get_group_files(
    state: State<'_, AppState>,
    group_id: String,
    _folder_id: Option<String>,
) -> Result<ApiResponse<Vec<GroupFileItemDto>>, String> {
    // Real implementation: mirror the remote list, then report local download state.
    if let Err(e) = crate::services::group_files::sync_group_files(&state.db, &state.onebot, &group_id).await {
        tracing::warn!("group file sync failed for {}: {}", group_id, e);
    }

    let records = state.db.get_group_files(&group_id).map_err(|e| e.to_string())?;
    let list: Vec<GroupFileItemDto> = records
        .into_iter()
        .map(|r| GroupFileItemDto {
            file_id: r.file_id,
            file_name: r.file_name,
            file_size: r.file_size,
            file_url: None,
            uploader_id: String::new(),
            uploader_name: r.uploader_name,
            upload_time: r.upload_time,
            download_status: r.download_status,
            local_path: r.local_path,
        })
        .collect();

    Ok(ApiResponse::ok(list))
}

#[command]
pub async fn download_file(
    state: State<'_, AppState>,
    group_id: String,
    file_id: String,
    file_name: String,
) -> Result<ApiResponse<serde_json::Value>, String> {
    tracing::info!("download requested: {} (group {}, file {})", file_name, group_id, file_id);

    let path = crate::services::group_files::download_group_file(
        &state.db,
        &state.onebot,
        &group_id,
        &file_id,
    )
    .await?;

    Ok(ApiResponse::ok(serde_json::json!({
        "taskId": format!("dl_{}", file_id),
        "localSavePath": path.display().to_string(),
    })))
}

#[command]
pub async fn summarize_file(
    state: State<'_, AppState>,
    local_file_path: String,
) -> Result<ApiResponse<FileSummaryResultDto>, String> {
    let result = crate::services::group_files::summarize_file(&state.ai, &local_file_path).await?;
    Ok(ApiResponse::ok(result))
}

#[command]
pub async fn open_folder(target_path: String) -> Result<ApiResponse<()>, String> {
    let base_data = crate::services::logging::data_dir();
    let path = if target_path.is_empty() || target_path == "." {
        base_data.join("group_files")
    } else {
        let p = std::path::Path::new(&target_path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            let rel = target_path
                .replace('/', "\\")
                .trim_start_matches("EazyQQ_Data\\")
                .trim_start_matches("EazyQQ_Data/")
                .to_string();
            base_data.join(rel)
        }
    };

    let _ = std::fs::create_dir_all(&path);

    #[cfg(target_os = "windows")]
    {
        let canonical_str = path.to_string_lossy().to_string();
        tracing::info!("open_folder: launching explorer for {}", canonical_str);
        let _ = std::process::Command::new("explorer")
            .arg(&canonical_str)
            .spawn();
    }
    Ok(ApiResponse::ok(()))
}

#[command]
pub async fn generate_summary(
    state: State<'_, AppState>,
    target_id: String,
    sliding_window_hours: Option<i32>,
    hours: Option<i32>,
) -> Result<ApiResponse<GroupSummaryDto>, String> {
    // Real summarization: reads the sliding-window message stream, filters noise and
    // asks the model for structured output. (Previously this returned a hardcoded
    // template string without reading a single message.)
    let settings = crate::services::scheduler::read_settings(&state.db);
    let window = sliding_window_hours
        .or(hours)
        .unwrap_or(settings.sliding_window_hours)
        .clamp(1, 720);

    tracing::info!("manual summary requested for {} ({}h window)", target_id, window);

    let req = crate::services::summarizer::SummaryRequest {
        target_id,
        sliding_window_hours: window,
        custom_prompt: Some(settings.custom_prompt),
        max_messages: 400,
        // A manual request should always attempt the model, even on a quiet window.
        min_messages: 1,
    };

    let outcome = crate::services::summarizer::generate(&state.db, &state.ai, &req).await?;
    Ok(ApiResponse::ok(outcome.summary))
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

    // Single source of truth, shared with the audit test in `services::config`.
    let default_config = crate::services::config::default_app_config();

    Ok(ApiResponse::ok(default_config))
}

#[command]
pub async fn update_config(
    state: State<'_, AppState>,
    config: serde_json::Value,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let serialized = serde_json::to_string(&config).map_err(|e| e.to_string())?;
    state.db.set_setting("app_config", &serialized).map_err(|e| e.to_string())?;

    // Hot-apply the AI provider so switching to a local model takes effect immediately
    // instead of requiring an app restart.
    let fallback_key = state
        .db
        .get_setting("tokenrhythm_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let ai_cfg =
        crate::services::ai::AiRuntimeConfig::from_app_config(config.get("ai"), &fallback_key);
    state.ai.reconfigure(ai_cfg.clone());

    tracing::info!("config updated; active AI provider -> {}", ai_cfg.describe());

    Ok(ApiResponse::ok(config))
}

#[command]
pub async fn test_ai_connection(state: State<'_, AppState>, _provider: String, _model_id: Option<String>) -> Result<ApiResponse<serde_json::Value>, String> {
    let cfg = state.ai.current();
    tracing::info!("AI connectivity test against {}", cfg.describe());

    let start = std::time::Instant::now();
    match state.ai.generate_reply(&[], "ping", "系统连通性自检").await {
        Ok((reply, _)) => {
            let latency = start.elapsed().as_millis();
            Ok(ApiResponse::ok(serde_json::json!({
                "isSuccess": true,
                "latencyMs": latency,
                "provider": cfg.provider,
                "model": cfg.model,
                "endpoint": cfg.base_url,
                "reply": reply,
            })))
        }
        Err(e) => Ok(ApiResponse::err(
            1003,
            format!("大模型连通测试失败 [{}]: {}", cfg.describe(), e),
            None,
        )),
    }
}

#[command]
pub async fn fetch_provider_models(
    provider: String,
    base_url: Option<String>,
    api_key: Option<String>,
) -> Result<ApiResponse<Vec<String>>, String> {
    let (preset_url, _) = crate::services::ai::provider_preset(&provider);
    let target_url = base_url
        .filter(|u| !u.trim().is_empty())
        .or(preset_url)
        .ok_or_else(|| "缺少 base_url".to_string())?;

    let key = if let Some(k) = api_key.filter(|s| !s.trim().is_empty()) {
        Some(k)
    } else if provider == "opencode" || provider == "opencode-go" {
        crate::services::ai::detect_opencode_auth_key()
    } else {
        None
    };

    match crate::services::ai::fetch_models_from_endpoint(&target_url, key.as_deref()).await {
        Ok(models) => Ok(ApiResponse::ok(models)),
        Err(e) => Ok(ApiResponse::err(1003, format!("获取模型失败: {}", e), None)),
    }
}

#[command]
pub async fn check_dependencies(state: State<'_, AppState>) -> Result<ApiResponse<DependencyHealthReport>, String> {
    // Real probes - this used to return hardcoded "everything is fine" values, which
    // made the health panel useless for diagnosing an actual problem.

    // 1. QQNT client: try the paths Tencent uses plus whatever NapCat was configured with.
    let qq_candidates = [
        r"C:\Program Files\Tencent\QQNT\QQ.exe",
        r"C:\Program Files (x86)\Tencent\QQNT\QQ.exe",
        r"D:\Program Files\Tencent\QQNT\QQ.exe",
    ];
    let (qq_ready, qq_path) = qq_candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| (true, p.to_string()))
        .unwrap_or_else(|| (false, qq_candidates[0].to_string()));

    // 2. Local AI runtime: a configured key or a local opencode binary both count.
    let has_key = state
        .db
        .get_setting("tokenrhythm_api_key")
        .ok()
        .flatten()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("TOKENRHYTHM_API_KEY").is_ok();
    let opencode_candidates = [
        r"A:\DevEnv\SDKs\Node\npm-global\opencode.cmd",
        r"C:\Users\www17\AppData\Roaming\npm\opencode.cmd",
    ];
    let opencode_found = opencode_candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists());
    let (ai_ready, ai_path) = match opencode_found {
        Some(p) => (true, p.to_string()),
        None if has_key => (true, format!("云端模型 (API Key 已配置, {})", state.ai.model())),
        None => (false, "未检测到本地 OpenCode，且未配置 API Key".to_string()),
    };

    // 3. Storage: really write a probe file, and report actual free space.
    let data_dir = crate::services::logging::data_dir();
    let writable = crate::services::diagnostics::is_dir_writable(&data_dir);
    let free_mb = crate::services::diagnostics::free_space_mb(&data_dir).unwrap_or(0);

    let napcat_ready = state.napcat.is_alive().await;
    let onebot_ready = state.onebot.get_login_info().await.is_ok();

    let is_all_ready = qq_ready && ai_ready && writable && napcat_ready && onebot_ready;

    tracing::info!(
        "health: qqnt={} ai={} storage={} napcat={} onebot={} => allReady={}",
        qq_ready,
        ai_ready,
        writable,
        napcat_ready,
        onebot_ready,
        is_all_ready
    );

    Ok(ApiResponse::ok(DependencyHealthReport {
        is_all_ready,
        qq_nt: ReadyPath {
            ready: qq_ready,
            path: qq_path,
        },
        open_code: ReadyPath {
            ready: ai_ready,
            path: ai_path,
        },
        storage: StorageReady {
            is_writable: writable,
            free_space_mb: free_mb,
        },
    }))
}

#[command]
pub async fn export_diagnostics_bundle(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let result = crate::services::diagnostics::export_bundle(
        &state.db,
        &state.ai,
        &state.napcat,
        &state.onebot,
    )
    .await?;

    Ok(ApiResponse::ok(serde_json::json!({
        "zipFilePath": result.zip_path.display().to_string(),
        "entries": result.entries,
        "bytes": result.bytes,
    })))
}

/// Window behavior preferences, persisted in the SQLite `app_settings.app_config` blob.
/// Project default: BOTH minimize and close collapse into the system tray, so the
/// background OneBot listener and scheduled summarizer keep running.
pub fn read_window_behavior(db: &Database) -> (bool, bool) {
    if let Ok(Some(raw)) = db.get_setting("app_config") {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) {
            if let Some(win) = parsed.get("window") {
                let minimize_to_tray = win
                    .get("minimizeToTray")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let close_to_tray = win
                    .get("closeToTray")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                return (minimize_to_tray, close_to_tray);
            }
        }
    }
    (true, true)
}

#[command]
pub async fn restart_napcat() -> Result<ApiResponse<crate::services::napcat_boot::BootOutcome>, String> {
    let root = crate::services::logging::workspace_root();
    let napcat_dir = root.join("napcat");
    let outcome = crate::services::napcat_boot::restart(&napcat_dir);
    Ok(ApiResponse::ok(outcome))
}


/// Restore the main window from tray / minimized state and pull it to the foreground.
pub fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[command]
pub async fn app_minimize_window(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let (minimize_to_tray, _) = read_window_behavior(&state.db);
    if minimize_to_tray {
        window.hide().map_err(|e| e.to_string())?;
    } else {
        window.minimize().map_err(|e| e.to_string())?;
    }
    Ok(minimize_to_tray)
}

#[command]
pub async fn app_toggle_maximize_window(window: tauri::Window) -> Result<bool, String> {
    if window.is_maximized().map_err(|e| e.to_string())? {
        window.unmaximize().map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        let _ = window.unminimize();
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

#[command]
pub async fn app_close_window(
    window: tauri::Window,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let (_, close_to_tray) = read_window_behavior(&state.db);
    if close_to_tray {
        window.hide().map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        window.app_handle().exit(0);
        Ok(false)
    }
}

#[command]
pub async fn app_start_drag_window(window: tauri::Window) -> Result<(), String> {
    window.start_dragging().map_err(|e| e.to_string())
}

#[command]
pub async fn app_show_window(app: tauri::AppHandle) -> Result<(), String> {
    show_main_window(&app);
    Ok(())
}

#[command]
pub async fn app_get_window_behavior(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let (minimize_to_tray, close_to_tray) = read_window_behavior(&state.db);
    Ok(ApiResponse::ok(serde_json::json!({
        "minimizeToTray": minimize_to_tray,
        "closeToTray": close_to_tray
    })))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_name: String,
    pub release_notes: String,
    pub html_url: String,
    pub download_url: Option<String>,
    pub published_at: String,
}

#[command]
pub async fn check_app_update() -> Result<ApiResponse<AppUpdateInfo>, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let client = reqwest::Client::builder()
        .user_agent("EazyQQ-Updater")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let url = "https://api.github.com/repos/LING71671/EazyQQ/releases/latest";
    let resp = client.get(url).send().await
        .map_err(|e| format!("检查更新网络请求失败: {}", e))?;

    if !resp.status().is_success() {
        return Ok(ApiResponse::ok(AppUpdateInfo {
            current_version: current_version.clone(),
            latest_version: current_version,
            has_update: false,
            release_name: "暂无线上发布版本".to_string(),
            release_notes: String::new(),
            html_url: "https://github.com/LING71671/EazyQQ/releases".to_string(),
            download_url: None,
            published_at: String::new(),
        }));
    }

    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("解析发布数据失败: {}", e))?;

    let tag_name = json.get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();

    let release_name = json.get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let release_notes = json.get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let html_url = json.get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("https://github.com/LING71671/EazyQQ/releases")
        .to_string();

    let published_at = json.get("published_at")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let download_url = json.get("assets")
        .and_then(|a| a.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
                if name.ends_with(".exe") || name.ends_with(".msi") || name.ends_with(".zip") {
                    item.get("browser_download_url").and_then(|u| u.as_str()).map(|s| s.to_string())
                } else {
                    None
                }
            })
        });

    let has_update = !tag_name.is_empty() && tag_name != current_version;

    Ok(ApiResponse::ok(AppUpdateInfo {
        current_version,
        latest_version: if tag_name.is_empty() { env!("CARGO_PKG_VERSION").to_string() } else { tag_name },
        has_update,
        release_name,
        release_notes,
        html_url,
        download_url,
        published_at,
    }))
}

