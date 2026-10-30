use eazyqq_lib::models::MessageItemDto;
use eazyqq_lib::services::policy;
use eazyqq_lib::services::protocol::{context, pipeline};

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
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
    let platform_id = context::identifier(&resp["data"]["message_id"]).unwrap_or_else(|| format!("manual_{now}"));
    let outgoing = MessageItemDto { id: format!("msg_{}_{}", target, platform_id), target_id: target.clone(), sender_id: scope.account_uin.clone(), sender_name: "我".into(), content: text.clone(), is_from_me: true, ai_reply_status: "none".into(), timestamp: now };
    context::save_outgoing_source(&svc.db, &scope, &outgoing, &platform_id, "owner_manual").map_err(|error| format!("消息已发送，但本地记录保存失败，请勿重复发送: {error}"))?;

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
    if source_id.is_none() && prompt.trim().is_empty() { return Err("提供 --text 或 --source-message-id 指定当前输入".into()); }

    let name = svc
        .db
        .get_all_rules()
        .unwrap_or_default()
        .into_iter()
        .find(|r| r.target_id == target)
        .map(|r| r.name)
        .unwrap_or_else(|| target.clone());

    let kind = context::target_type(&svc.db, &target, args.flag("type"))?;
    let scope = context::ConversationScope::for_client(&svc.onebot, &kind, &target)?;
    let (history, incoming) = if let Some(id) = source_id {
        let message = context::source_message(&svc.db, &scope, id)?;
        pipeline::prepare_context(&svc.db, &svc.onebot, &message, &kind, svc.ai.current().max_context_messages).await?
    } else { (context::build_context_history(&svc.db, &scope, None, svc.ai.current().max_context_messages)?, context::requested_input(&scope, &prompt)) };
    let (content, thinking) = svc
        .ai
        .generate_context_reply(&history, &incoming, &name)
        .await
        .map_err(|e| format!("AI 生成失败: {}", e))?;

    if args.json() {
        print_json(&serde_json::json!({
            "targetId": target,
            "targetName": name,
            "reply": content,
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

    let draft = svc
        .db
        .get_pending_drafts()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| format!("找不到草稿 {}", id))?;

    // Re-validate at send time: the rule may have been revoked since the draft was made.
    let pol = policy::load(&svc.db, &draft.target_id);
    if !pol.ai_execution_allowed() {
        return Err(format!(
            "目标 {} 当前不在消息接管白名单中（{}），已安全拦截",
            draft.target_id,
            pol.describe()
        ));
    }

    let content = args
        .flag("text")
        .map(|s| s.to_string())
        .unwrap_or_else(|| draft.generated_content.clone());
    let target_type = if draft.target_type == "group" {
        "group"
    } else {
        "private"
    };

    let resp = svc
        .onebot
        .send_msg(target_type, &draft.target_id, &content)
        .await?;
    if resp.get("status").and_then(|s| s.as_str()) != Some("ok") {
        return Err(format!("发送失败: {}", resp));
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let platform_id = context::identifier(&resp["data"]["message_id"]).unwrap_or_else(|| format!("draft_{now_ms}"));
    let scope = context::ConversationScope::for_client(&svc.onebot, target_type, &draft.target_id)?;
    let outgoing = MessageItemDto {
        id: format!("msg_{}_{}", draft.target_id, platform_id),
        target_id: draft.target_id.clone(),
        sender_id: scope.account_uin.clone(),
        sender_name: "我 (草稿放行)".to_string(),
        content: content.clone(),
        is_from_me: true,
        ai_reply_status: "auto_replied".to_string(),
        timestamp: now_ms,
    };
    context::save_outgoing_source(&svc.db, &scope, &outgoing, &platform_id, "approved_ai_draft").map_err(|error| format!("草稿已发送，但本地记录保存失败，请勿重复发送: {error}"))?;
    svc.db.delete_draft(id).map_err(|e| e.to_string())?;

    tracing::info!("draft {} approved and sent to {}", id, draft.target_id);

    if args.json() {
        print_json(&serde_json::json!({
            "draftId": id,
            "targetId": draft.target_id,
            "targetType": target_type,
            "content": content,
            "status": "sent",
        }));
    } else {
        println!("草稿已放行 -> {} ({})", draft.target_id, target_type);
        println!("内容: {}", content);
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

    let draft = svc
        .db
        .get_pending_drafts()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| format!("找不到草稿 {}", id))?;

    let scope = context::ConversationScope::for_client(&svc.onebot, &draft.target_type, &draft.target_id)?;
    let source_exists = svc.db.get_message_source(&draft.reply_to_msg_id).map_err(|error| error.to_string())?.is_some();
    let (history, incoming) = match context::source_message(&svc.db, &scope, &draft.reply_to_msg_id) {
        Ok(message) => pipeline::prepare_context(&svc.db, &svc.onebot, &message, &draft.target_type, svc.ai.current().max_context_messages).await?,
        Err(error) if source_exists => return Err(error),
        Err(_) => (context::build_context_history(&svc.db, &scope, None, svc.ai.current().max_context_messages)?, context::requested_input(&scope, &draft.incoming_message_snippet)),
    };
    let (content, thinking) = svc
        .ai
        .regenerate_context_reply(
            &history,
            &incoming,
            &draft.target_name,
            &draft.generated_content,
            instruction,
        )
        .await
        .map_err(|e| format!("重新生成失败: {}", e))?;

    svc.db
        .update_draft_content(id, &content, thinking.as_deref())
        .map_err(|e| e.to_string())?;

    tracing::info!("draft {} regenerated with instruction: {}", id, instruction);

    if args.json() {
        print_json(&serde_json::json!({
            "draftId": id,
            "instruction": instruction,
            "content": content,
            "thinking": thinking,
        }));
    } else {
        hr();
        println!("草稿 {} 已按指令重新生成", id);
        hr();
        println!("{}", content);
        hr();
    }
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
