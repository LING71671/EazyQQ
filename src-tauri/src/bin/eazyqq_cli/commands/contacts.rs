use eazyqq_lib::services::contacts;
use eazyqq_lib::services::db::ContactRuleRecord;

use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, mode_label, print_json, rule_to_json, truncate};

pub async fn cmd_contacts(svc: &Services, args: &Args) -> Result<(), String> {
    let report = contacts::sync_roster(&svc.db, &svc.onebot).await;

    let type_filter = args.flag("type").unwrap_or("");
    let search = args.flag("search").unwrap_or("").to_lowercase();
    let only_whitelist = args.has("whitelist");
    let only_summary = args.has("summary-only");

    let filtered: Vec<&ContactRuleRecord> = report
        .rules
        .iter()
        .filter(|r| type_filter.is_empty() || r.target_type == type_filter)
        .filter(|r| search.is_empty() || r.name.to_lowercase().contains(&search) || r.target_id.contains(&search))
        .filter(|r| !only_whitelist || r.mode != "ignore")
        .filter(|r| !only_summary || r.is_summary_whitelist)
        .collect();

    if args.json() {
        let list: Vec<serde_json::Value> = filtered.iter().map(|r| rule_to_json(r)).collect();
        print_json(&serde_json::json!({
            "offline": report.offline,
            "friends": report.friends,
            "groups": report.groups,
            "created": report.created,
            "total": list.len(),
            "list": list,
        }));
        return Ok(());
    }

    hr();
    println!(
        "联系人规则  (好友 {} / 群 {} / 新增默认拒绝 {} / 刷新 {})",
        report.friends, report.groups, report.created, report.refreshed
    );
    if report.offline {
        println!("注意: OneBot 不可达，以下为本地缓存规则。");
    }
    hr();
    if filtered.is_empty() {
        println!("(无匹配条目)");
    }
    for r in filtered {
        let kind = if r.target_type == "group" { "群" } else { "友" };
        let trigger = match r.trigger_condition.as_str() {
            "at_me" => "@我",
            "keyword" => "关键词",
            _ => "全部",
        };
        let unread = svc.db.count_unread(&r.target_id).unwrap_or(0);
        println!(
            "[{}] {:<28} {}  {:<8} 触发:{} 冷却:{}s{}{}{}",
            kind,
            truncate(&r.name, 28),
            r.target_id,
            mode_label(&r.mode),
            trigger,
            r.cooldown_seconds,
            if r.is_summary_whitelist {
                format!("  简报白名单({}h)", r.summary_interval_hours)
            } else {
                String::new()
            },
            if r.keywords != "[]" && !r.keywords.is_empty() {
                format!("  关键词:{}", r.keywords)
            } else {
                String::new()
            },
            if unread > 0 {
                format!("  未读:{}", unread)
            } else {
                String::new()
            }
        );
    }
    hr();
    Ok(())
}

pub async fn cmd_raw_roster(svc: &Services, args: &Args, groups: bool) -> Result<(), String> {
    if groups {
        let list = svc.onebot.get_group_list().await?;
        if args.json() {
            print_json(&serde_json::to_value(&list).unwrap_or(serde_json::json!([])));
        } else {
            hr();
            println!("真实群列表 ({} 个)", list.len());
            hr();
            for g in list {
                println!(
                    "{}  {:<30} 成员 {}",
                    g.group_id,
                    truncate(&g.group_name, 30),
                    g.member_count.unwrap_or(0)
                );
            }
            hr();
        }
    } else {
        let list = svc.onebot.get_friend_list().await?;
        if args.json() {
            print_json(&serde_json::to_value(&list).unwrap_or(serde_json::json!([])));
        } else {
            hr();
            println!("真实好友列表 ({} 个)", list.len());
            hr();
            for f in list {
                println!(
                    "{}  {:<24} 备注: {}",
                    f.user_id,
                    truncate(&f.nickname, 24),
                    f.remark.unwrap_or_else(|| "-".to_string())
                );
            }
            hr();
        }
    }
    Ok(())
}

pub async fn cmd_rule(svc: &Services, args: &Args) -> Result<(), String> {
    let target_id = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <目标ID>".to_string())?
        .to_string();

    // Ensure the roster is synced so the rule exists.
    let report = contacts::sync_roster(&svc.db, &svc.onebot).await;
    let mut rule = report
        .rules
        .into_iter()
        .find(|r| r.target_id == target_id)
        .ok_or_else(|| format!("找不到目标 {}，请先运行 `eazyqq-cli contacts` 同步名单", target_id))?;

    let mut changed = Vec::new();

    if let Some(mode) = args.flag("mode") {
        if !["auto_reply", "copilot", "ignore"].contains(&mode) {
            return Err(format!("无效的 --mode {}（可选 auto_reply/copilot/ignore）", mode));
        }
        rule.mode = mode.to_string();
        rule.enabled = mode != "ignore";
        changed.push(format!("mode={}", mode));
    }

    if let Some(summary) = args.flag("summary") {
        let on = matches!(summary, "on" | "true" | "1" | "yes");
        rule.is_summary_whitelist = on;
        changed.push(format!("summaryWhitelist={}", on));
    }

    if let Some(hours) = args.flag("interval-hours") {
        let h: i32 = hours
            .parse()
            .map_err(|_| format!("--interval-hours 需要整数，收到 {}", hours))?;
        rule.summary_interval_hours = h;
        changed.push(format!("intervalHours={}", h));
    }

    if let Some(trigger) = args.flag("trigger") {
        if !["all", "at_me", "keyword"].contains(&trigger) {
            return Err(format!("无效的 --trigger {}", trigger));
        }
        rule.trigger_condition = trigger.to_string();
        changed.push(format!("trigger={}", trigger));
    }

    if let Some(keywords) = args.flag("keywords") {
        let list: Vec<String> = keywords
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        rule.keywords = serde_json::to_string(&list).unwrap_or_else(|_| "[]".to_string());
        changed.push(format!("keywords={}", list.join("|")));
    }

    if changed.is_empty() {
        if args.json() {
            print_json(&rule_to_json(&rule));
        } else {
            println!("当前规则: {}", serde_json::to_string_pretty(&rule_to_json(&rule)).unwrap_or_default());
            println!("未指定任何变更项，未做修改。");
        }
        return Ok(());
    }

    rule.updated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    svc.db
        .upsert_rule(&rule)
        .map_err(|e| format!("写入规则失败: {}", e))?;

    tracing::info!("rule updated for {}: {}", target_id, changed.join(", "));

    if args.json() {
        print_json(&rule_to_json(&rule));
    } else {
        println!("已更新 [{}] {}", rule.target_type, rule.name);
        println!("变更: {}", changed.join(", "));
        println!("模式: {}", mode_label(&rule.mode));
        println!(
            "简报白名单: {} ({}h)",
            if rule.is_summary_whitelist { "是" } else { "否" },
            rule.summary_interval_hours
        );
    }
    Ok(())
}
