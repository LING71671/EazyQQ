use crate::commands::AppState;
use crate::models::{ApiResponse, ContactItemDto, RoutingRuleDto};
use crate::services::db::ContactRuleRecord;
use tauri::{command, State};

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
pub async fn get_contacts(
    state: State<'_, AppState>,
) -> Result<ApiResponse<serde_json::Value>, String> {
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
pub async fn update_rule(
    state: State<'_, AppState>,
    rule: serde_json::Value,
) -> Result<ApiResponse<serde_json::Value>, String> {
    let target_id = rule
        .get("targetId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if target_id.is_empty() {
        return Ok(ApiResponse::err(1002, "目标 ID 不能为空", None));
    }

    // Find existing rule if present to preserve contact details
    let existing = state
        .db
        .get_all_rules()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|r| r.target_id == target_id);

    let mode = rule
        .get("mode")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|e| e.mode.clone())
                .unwrap_or_else(|| "ignore".to_string())
        });
    if !["ignore", "auto_reply", "copilot", "summary_only"].contains(&mode.as_str()) {
        return Ok(ApiResponse::err(1002, "Invalid rule mode", None));
    }
    let trigger_condition = rule
        .get("triggerCondition")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|e| e.trigger_condition.clone())
                .unwrap_or_else(|| "at_me".to_string())
        });
    if !["all", "at_me", "keyword"].contains(&trigger_condition.as_str()) {
        return Ok(ApiResponse::err(1002, "Invalid trigger condition", None));
    }
    let keywords = rule
        .get("keywords")
        .map(|k| k.to_string())
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|e| e.keywords.clone())
                .unwrap_or_else(|| "[]".to_string())
        });
    let cooldown = rule
        .get("cooldownSeconds")
        .and_then(|v| v.as_i64())
        .map(|v| v.clamp(0, 86400) as i32)
        .unwrap_or_else(|| existing.as_ref().map(|e| e.cooldown_seconds).unwrap_or(5));
    let enabled = rule
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|e| e.enabled)
                .unwrap_or(mode != "ignore")
        });
    let is_summary_whitelist = rule
        .get("isSummaryWhitelist")
        .and_then(|v| v.as_bool())
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|e| e.is_summary_whitelist)
                .unwrap_or(false)
        });
    let summary_interval = rule
        .get("summaryIntervalHours")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .unwrap_or_else(|| {
            existing
                .as_ref()
                .map(|e| e.summary_interval_hours)
                .unwrap_or(6)
        });

    let record = ContactRuleRecord {
        target_id: target_id.clone(),
        target_type: existing
            .as_ref()
            .map(|e| e.target_type.clone())
            .unwrap_or_else(|| "group".to_string()),
        name: existing
            .as_ref()
            .map(|e| e.name.clone())
            .unwrap_or_default(),
        avatar_url: existing
            .as_ref()
            .map(|e| e.avatar_url.clone())
            .unwrap_or_default(),
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
pub async fn batch_update_mode(
    state: State<'_, AppState>,
    target_ids: Vec<String>,
    mode: String,
) -> Result<ApiResponse<serde_json::Value>, String> {
    if !["ignore", "auto_reply", "copilot", "summary_only"].contains(&mode.as_str()) {
        return Ok(ApiResponse::err(1002, "Invalid rule mode", None));
    }
    let affected = state
        .db
        .batch_update_mode(&target_ids, &mode)
        .map_err(|e| e.to_string())?;
    Ok(ApiResponse::ok(serde_json::json!({
        "affectedCount": affected
    })))
}
