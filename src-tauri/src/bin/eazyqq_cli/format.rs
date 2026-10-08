use eazyqq_lib::services::db::ContactRuleRecord;

pub fn hr() {
    println!("{}", "-".repeat(64));
}

pub fn print_json(value: &serde_json::Value) {
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("JSON encode error: {}", e),
    }
}

pub fn mode_label(mode: &str) -> &'static str {
    match mode {
        "auto_reply" => "自动秒回",
        "copilot" => "草稿审核",
        "ignore" => "直通忽略",
        other => Box::leak(format!("未知({})", other).into_boxed_str()),
    }
}

pub fn rule_to_json(r: &ContactRuleRecord) -> serde_json::Value {
    serde_json::json!({
        "targetId": r.target_id,
        "targetType": r.target_type,
        "name": r.name,
        "avatarUrl": r.avatar_url,
        "mode": r.mode,
        "triggerCondition": r.trigger_condition,
        "keywords": serde_json::from_str::<serde_json::Value>(&r.keywords).unwrap_or(serde_json::json!([])),
        "cooldownSeconds": r.cooldown_seconds,
        "enabled": r.enabled,
        "isSummaryWhitelist": r.is_summary_whitelist,
        "summaryIntervalHours": r.summary_interval_hours,
        "updatedAt": r.updated_at,
    })
}

pub fn truncate(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= width {
        s.to_string()
    } else {
        format!(
            "{}…",
            chars[..width.saturating_sub(1)].iter().collect::<String>()
        )
    }
}

/// Wrap a string for terminal output, counting characters rather than bytes so CJK text
/// lines up.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut n = 0usize;
    for ch in text.chars() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut cur));
            n = 0;
            continue;
        }
        let w = if (ch as u32) > 0x2000 { 2 } else { 1 };
        if n + w > width {
            lines.push(std::mem::take(&mut cur));
            n = 0;
        }
        cur.push(ch);
        n += w;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}
