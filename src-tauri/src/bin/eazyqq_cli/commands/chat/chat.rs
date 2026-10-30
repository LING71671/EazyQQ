use eazyqq_lib::models::MessageItemDto;
use eazyqq_lib::services::protocol::context;
use eazyqq_lib::services::protocol::reply_workflow;

use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json, truncate};

pub async fn cmd_send(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <群号或QQ号>".to_string())?
        .to_string();
    let text = args
        .flag("text")
        .ok_or_else(|| "缺少 --text <消息内容>".to_string())?
        .to_string();
    let target_type = args.flag("type").unwrap_or_else(|| {
        // Group ids from the live roster win; otherwise treat as a friend.
        "auto"
    });

    let client = if let Some(uin) = args.flag("account").or_else(|| args.flag("uin")) {
        svc.instance_pool.get_client(uin).await
    } else {
        svc.onebot.clone()
    };
    if client.expected_account() != svc.onebot.expected_account() {
        return Err("发送账号与数据库上下文不一致，请用 --account 绑定本次命令".into());
    }

    let resolved_type = if target_type == "auto" {
        let groups = client.get_group_list().await.unwrap_or_default();
        if groups.iter().any(|g| g.group_id.to_string() == target) {
            "group"
        } else {
            "friend"
        }
    } else {
        target_type
    };

    let resp = client.send_msg(resolved_type, &target, &text).await?;
    let ok = resp.get("status").and_then(|s| s.as_str()) == Some("ok");
    if !ok {
        return Err(format!("发送失败: {}", resp));
    }

    let message_id = resp
        .get("data")
        .and_then(|d| d.get("message_id"))
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_string());
    let scope = context::ConversationScope::for_client(&client, resolved_type, &target)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let platform_id =
        context::identifier(&resp["data"]["message_id"]).unwrap_or_else(|| format!("manual_{now}"));
    let outgoing = MessageItemDto {
        id: format!("msg_{}_{}", target, platform_id),
        target_id: target.clone(),
        sender_id: scope.account_uin.clone(),
        sender_name: "我".into(),
        content: text.clone(),
        is_from_me: true,
        ai_reply_status: "none".into(),
        timestamp: now,
    };
    context::save_outgoing_source(&svc.db, &scope, &outgoing, &platform_id, "owner_manual")
        .map_err(|error| format!("消息已发送，但本地记录保存失败，请勿重复发送: {error}"))?;

    tracing::info!("message sent to {} ({})", target, resolved_type);

    if args.json() {
        print_json(&serde_json::json!({
            "targetId": target,
            "targetType": resolved_type,
            "messageId": message_id,
            "status": "ok",
        }));
    } else {
        println!("已发送 -> {} ({})", target, resolved_type);
        println!("message_id: {}", message_id);
    }
    Ok(())
}

pub async fn cmd_ask(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <目标ID>".to_string())?
        .to_string();
    let source_id = args.flag("source-message-id");
    let prompt = args.flag("text").unwrap_or("").to_string();
    if source_id.is_none() && prompt.trim().is_empty() {
        return Err("提供 --text 或 --source-message-id 指定当前输入".into());
    }

    let name = target.clone();
    let (plan, thinking) = reply_workflow::suggest(
        &svc.db,
        &svc.onebot,
        &svc.ai,
        &target,
        args.flag("type"),
        &prompt,
        source_id,
    )
    .await?;
    let content = plan.preview();

    if args.json() {
        print_json(&serde_json::json!({
            "targetId": target,
            "targetName": name,
            "reply": content,
            "replyPlan":plan,
            "thinking": thinking,
            "model": svc.ai.model(),
        }));
    } else {
        hr();
        println!("AI 候选回复 -> {} ({})", name, target);
        hr();
        println!("{}", content);
        if let Some(t) = thinking {
            println!();
            println!("--- 思考链路 ---");
            println!("{}", t);
        }
        hr();
    }
    Ok(())
}

pub async fn cmd_drafts(svc: &Services, args: &Args) -> Result<(), String> {
    let drafts = svc.db.get_pending_drafts().map_err(|e| e.to_string())?;
    if args.json() {
        print_json(&serde_json::to_value(&drafts).unwrap_or(serde_json::json!([])));
        return Ok(());
    }
    hr();
    println!("待审核草稿 ({} 条)", drafts.len());
    hr();
    for d in drafts {
        println!(
            "{}  -> {} ({})\n  收到: {}\n  拟答: {}\n  模型: {}",
            d.id,
            d.target_name,
            d.target_id,
            truncate(&d.incoming_message_snippet, 60),
            truncate(&d.generated_content, 60),
            d.model_used
        );
        println!();
    }
    hr();
    Ok(())
}

pub async fn cmd_history(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <目标ID>".to_string())?;
    let limit: usize = args
        .flag("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    let msgs = svc
        .db
        .get_messages_by_target(target, limit)
        .map_err(|e| e.to_string())?;

    if args.json() {
        print_json(&serde_json::to_value(&msgs).unwrap_or(serde_json::json!([])));
        return Ok(());
    }
    hr();
    println!("消息流水 {} (最近 {} 条)", target, msgs.len());
    hr();
    for m in msgs {
        println!(
            "[{}] {}: {}",
            if m.is_from_me { "我" } else { &m.sender_name },
            m.timestamp,
            truncate(&m.content, 70)
        );
    }
    hr();
    Ok(())
}

pub async fn cmd_draft_send(svc: &Services, args: &Args) -> Result<(), String> {
    let id = args
        .flag("id")
        .ok_or_else(|| "缺少 --id <草稿ID>（可用 `eazyqq-cli drafts` 查看）".to_string())?;

    let bubbles = args
        .flag("bubbles-json")
        .map(|value| {
            serde_json::from_str::<Vec<String>>(value)
                .map_err(|_| "--bubbles-json 必须是文字数组，数组项对应一条消息".to_string())
        })
        .transpose()?;
    let report = reply_workflow::approve_draft(
        &svc.db,
        &svc.onebot,
        id,
        args.flag("text").map(str::to_owned),
        bubbles,
    )
    .await?;
    print_json(&serde_json::to_value(&report).map_err(|error| error.to_string())?);
    if report.status != "sent" {
        return Err(format!(
            "批次状态 {}；请核对回执后处理剩余消息",
            report.status
        ));
    }
    Ok(())
}

pub fn cmd_draft_dismiss(svc: &Services, args: &Args) -> Result<(), String> {
    let id = args
        .flag("id")
        .ok_or_else(|| "缺少 --id <草稿ID>".to_string())?;
    svc.db.delete_draft(id).map_err(|e| e.to_string())?;
    if args.json() {
        print_json(&serde_json::json!({"dismissed":id}));
    } else {
        println!("草稿 {} 已丢弃", id);
    }
    Ok(())
}

/// Regenerate a draft with a human refinement instruction.
pub async fn cmd_draft_regenerate(svc: &Services, args: &Args) -> Result<(), String> {
    let id = args
        .flag("id")
        .ok_or_else(|| "缺少 --id <草稿ID>".to_string())?;
    let instruction = args
        .flag("instruction")
        .ok_or_else(|| "缺少 --instruction <微调指令>".to_string())?;

    let draft = reply_workflow::regenerate(&svc.db, &svc.onebot, &svc.ai, id, instruction).await?;
    print_json(&serde_json::to_value(&draft).map_err(|error| error.to_string())?);
    Ok(())
}

pub fn cmd_mark_read(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <目标ID>".to_string())?;

    let before = svc.db.count_unread(target).unwrap_or(0);
    let at = args
        .flag("at")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

    svc.db
        .mark_read(target, at)
        .map_err(|e| format!("标记已读失败: {}", e))?;

    let after = svc.db.count_unread(target).unwrap_or(0);

    if args.json() {
        print_json(&serde_json::json!({
            "targetId": target,
            "markedAt": at,
            "unreadBefore": before,
            "unreadAfter": after,
        }));
    } else {
        println!("已标记 {} 为已读（未读 {} -> {}）", target, before, after);
    }
    Ok(())
}
