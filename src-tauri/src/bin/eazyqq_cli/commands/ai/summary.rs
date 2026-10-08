use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json, rule_to_json, truncate};

pub async fn cmd_summaries(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args.flag("target");
    let list = svc.db.get_summaries(target).map_err(|e| e.to_string())?;
    if args.json() {
        print_json(&serde_json::to_value(&list).unwrap_or(serde_json::json!([])));
        return Ok(());
    }
    hr();
    println!("已生成简报 ({} 条)", list.len());
    hr();
    for s in list {
        println!(
            "{}  [{}] {}",
            s.created_at,
            s.target_name,
            truncate(&s.summary_text, 80)
        );
    }
    hr();
    Ok(())
}

pub async fn cmd_whitelist_groups(svc: &Services, args: &Args) -> Result<(), String> {
    let list = svc
        .db
        .get_summary_whitelist_groups()
        .map_err(|e| e.to_string())?;
    if args.json() {
        let json: Vec<serde_json::Value> = list.iter().map(rule_to_json).collect();
        print_json(&serde_json::json!({ "total": json.len(), "list": json }));
        return Ok(());
    }
    hr();
    println!("简报白名单群聊 ({} 个)", list.len());
    hr();
    if list.is_empty() {
        println!("(空) 未加入白名单的群聊默认完全旁路。");
    }
    for r in list {
        println!(
            "{}  {:<30} 周期 {}h",
            r.target_id,
            truncate(&r.name, 30),
            r.summary_interval_hours
        );
    }
    hr();
    Ok(())
}

pub async fn cmd_summarize(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <群号>".to_string())?
        .to_string();

    let settings = eazyqq_lib::services::scheduler::read_settings(&svc.db);
    let hours: i32 = args
        .flag("hours")
        .and_then(|v| v.parse().ok())
        .unwrap_or(settings.sliding_window_hours);

    let req = eazyqq_lib::services::summarizer::SummaryRequest {
        target_id: target.clone(),
        sliding_window_hours: hours,
        custom_prompt: args
            .flag("prompt")
            .map(|s| s.to_string())
            .or(Some(settings.custom_prompt.clone())),
        max_messages: 400,
        min_messages: 1,
    };

    let outcome = if args.has("stream") {
        let json = args.json();
        eazyqq_lib::services::summarizer::generate_stream(&svc.db, &svc.ai, &req, move |chunk| {
            if json {
                println!("{}", serde_json::json!({"type":"chunk","content":chunk}));
            } else {
                use std::io::Write;
                print!("{}", chunk);
                let _ = std::io::stdout().flush();
            }
        })
        .await?
    } else {
        eazyqq_lib::services::summarizer::generate(&svc.db, &svc.ai, &req).await?
    };
    if args.has("stream") && args.json() {
        println!(
            "{}",
            serde_json::json!({"type":"complete","summary":outcome.summary})
        );
        return Ok(());
    }

    if args.json() {
        print_json(&serde_json::to_value(&outcome.summary).unwrap_or(serde_json::json!({})));
        return Ok(());
    }

    hr();
    println!("群聊简报 -> {} ({})", outcome.summary.target_name, target);
    println!(
        "窗口 {}h | 扫描 {} 条 | 有效 {} 条",
        hours, outcome.scanned, outcome.usable
    );
    if let Some(reason) = &outcome.skipped_reason {
        println!("跳过原因: {}", reason);
    }
    hr();
    println!("{}", outcome.summary.summary_text);
    if !outcome.summary.key_points.is_empty() {
        println!();
        println!("[讨论要点]");
        for p in &outcome.summary.key_points {
            println!("  - {}", p);
        }
    }
    if !outcome.summary.decisions.is_empty() {
        println!();
        println!("[决议与待办]");
        for d in &outcome.summary.decisions {
            println!("  - {}", d);
        }
    }
    hr();
    Ok(())
}

/// Force one scheduler pass, exactly as the background loop would run it.
pub async fn cmd_scheduler_tick(svc: &Services, args: &Args) -> Result<(), String> {
    let settings = eazyqq_lib::services::scheduler::read_settings(&svc.db);
    println!(
        "定时总结: {} | 滑动窗口 {}h | Prompt: {}",
        if settings.enabled {
            "已启用"
        } else {
            "已暂停"
        },
        settings.sliding_window_hours,
        truncate(&settings.custom_prompt, 40)
    );

    let outcomes = eazyqq_lib::services::scheduler::tick(&svc.db, &svc.ai, &svc.onebot, None).await;

    if args.json() {
        let list: Vec<serde_json::Value> = outcomes
            .iter()
            .map(|o| {
                serde_json::json!({
                    "targetId": o.summary.target_id,
                    "targetName": o.summary.target_name,
                    "scanned": o.scanned,
                    "usable": o.usable,
                    "skipped": o.skipped_reason,
                    "summaryId": o.summary.id,
                })
            })
            .collect();
        print_json(&serde_json::json!({ "generated": list.len(), "list": list }));
        return Ok(());
    }

    hr();
    if outcomes.is_empty() {
        println!("本次没有到期的群聊（全部未到执行周期）。");
    } else {
        println!("本次生成 {} 份简报:", outcomes.len());
        for o in &outcomes {
            println!(
                "  - {} (扫描 {} 条 / 有效 {} 条){}",
                o.summary.target_name,
                o.scanned,
                o.usable,
                o.skipped_reason
                    .as_ref()
                    .map(|r| format!(" [{}]", r))
                    .unwrap_or_default()
            );
        }
    }
    hr();
    Ok(())
}
