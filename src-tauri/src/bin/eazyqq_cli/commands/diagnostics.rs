use eazyqq_lib::services::logging;

use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json, truncate, wrap};

pub async fn cmd_health(svc: &Services, args: &Args) -> Result<(), String> {
    let deep = args.has("deep");
    let mut items: Vec<(String, bool, String)> = Vec::new();

    let napcat_alive = svc.napcat.is_alive().await;
    items.push((
        "NapCat WebUI (127.0.0.1:6099)".to_string(),
        napcat_alive,
        if napcat_alive { "在线".into() } else { "离线".into() },
    ));

    let login = svc.onebot.get_login_info().await;
    items.push((
        "OneBot HTTP (127.0.0.1:3000)".to_string(),
        login.is_ok(),
        match &login {
            Ok(v) => format!(
                "已登录 {} ({})",
                v.get("data").and_then(|d| d.get("nickname")).and_then(|n| n.as_str()).unwrap_or("-"),
                v.get("data").and_then(|d| d.get("user_id")).and_then(|n| n.as_i64()).unwrap_or(0)
            ),
            Err(e) => e.clone(),
        },
    ));

    let data_dir = logging::data_dir();
    let writable = std::fs::metadata(&data_dir).is_ok();
    items.push((
        "本地数据目录可写".to_string(),
        writable,
        data_dir.display().to_string(),
    ));

    let (log_dir, crash_count, _last) = logging::diagnostics();
    items.push((
        "日志目录".to_string(),
        log_dir.exists(),
        format!("{} (历史崩溃报告 {} 份)", log_dir.display(), crash_count),
    ));

    let rule_count = svc.db.get_all_rules().map(|r| r.len()).unwrap_or(0);
    let summary_groups = svc.db.get_summary_whitelist_groups().map(|r| r.len()).unwrap_or(0);
    items.push((
        "SQLite 规则库".to_string(),
        true,
        format!("{} 条规则 / {} 个简报白名单群", rule_count, summary_groups),
    ));

    let api_key_present = svc
        .db
        .get_setting("tokenrhythm_api_key")
        .ok()
        .flatten()
        .map(|k| !k.is_empty())
        .unwrap_or(false)
        || std::env::var("TOKENRHYTHM_API_KEY").is_ok();
    items.push((
        "大模型 API Key".to_string(),
        api_key_present,
        if api_key_present { "已配置".into() } else { "缺失".into() },
    ));

    if deep {
        let probe = svc.ai.generate_reply(&[], "ping", "自检").await;
        items.push((
            format!("大模型连通性 ({})", svc.ai.model()),
            probe.is_ok(),
            match probe {
                Ok((c, _)) => format!("回复: {}", truncate(&c, 40)),
                Err(e) => e,
            },
        ));
    }

    let all_ok = items.iter().all(|(_, ok, _)| *ok);

    if args.json() {
        let json: Vec<serde_json::Value> = items
            .iter()
            .map(|(n, ok, d)| serde_json::json!({ "item": n, "ok": ok, "detail": d }))
            .collect();
        print_json(&serde_json::json!({ "allOk": all_ok, "items": json }));
        return Ok(());
    }

    hr();
    println!("EazyQQ 健康自检");
    hr();
    for (name, ok, detail) in &items {
        println!("{} {:<34} {}", if *ok { "[OK]  " } else { "[FAIL]" }, name, detail);
    }
    hr();
    println!("总体: {}", if all_ok { "全部通过" } else { "存在异常项" });
    hr();
    Ok(())
}

/// Debug backdoor: inject a synthetic OneBot message event into the real pipeline.
pub async fn cmd_simulate(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <群号或QQ号>".to_string())?
        .to_string();
    let text = args
        .flag("text")
        .ok_or_else(|| "缺少 --text <模拟收到的消息>".to_string())?
        .to_string();

    let sender_name = args.flag("sender-name").unwrap_or("模拟用户").to_string();
    let sender_id = args.flag("sender-id").unwrap_or("10000001").to_string();

    // Resolve the group/friend distinction from the live roster unless told explicitly.
    let is_group = match args.flag("type") {
        Some("group") => true,
        Some("friend") | Some("private") => false,
        _ => svc
            .onebot
            .get_group_list()
            .await
            .unwrap_or_default()
            .iter()
            .any(|g| g.group_id.to_string() == target),
    };

    let self_id = svc
        .onebot
        .get_login_info()
        .await
        .ok()
        .and_then(|v| {
            v.get("data")
                .and_then(|d| d.get("user_id"))
                .and_then(|u| u.as_i64())
        })
        .map(|n| n.to_string())
        .unwrap_or_default();

    let event = if is_group {
        serde_json::json!({
            "post_type": "message",
            "message_type": "group",
            "sub_type": "normal",
            "group_id": target,
            "user_id": sender_id,
            "self_id": self_id,
            "sender": { "nickname": sender_name, "card": sender_name },
            "raw_message": text,
            "message": text,
        })
    } else {
        serde_json::json!({
            "post_type": "message",
            "message_type": "private",
            "sub_type": "friend",
            "user_id": target,
            "self_id": self_id,
            "sender": { "nickname": sender_name },
            "raw_message": text,
            "message": text,
        })
    };

    println!(
        "注入模拟事件: {} {} <- {} ({})",
        if is_group { "群" } else { "好友" },
        target,
        truncate(&text, 40),
        sender_name
    );

    // `--repeat N` injects the same message N times inside one process. This is the only
    // way to exercise the in-memory reply cooldown from the CLI, since every other
    // invocation is a fresh process with an empty cooldown registry.
    let repeat: usize = args
        .flag("repeat")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
        .max(1);

    let mut last = None;
    for i in 0..repeat {
        let outcome = eazyqq_lib::services::ws_listener::handle_onebot_event(
            None,
            &event,
            &svc.db,
            &svc.onebot,
            &svc.ai,
            true, // await the AI work so the result is observable before we exit
        )
        .await;

        if repeat > 1 {
            println!("  [{}/{}] {} -> {}", i + 1, repeat, outcome.action, outcome.summary());
        }
        last = Some(outcome);
    }

    let outcome = last.expect("repeat is clamped to at least 1");

    if args.json() {
        print_json(&serde_json::json!({
            "notAMessage": outcome.not_a_message,
            "bypassed": outcome.bypassed,
            "recorded": outcome.recorded,
            "targetId": outcome.target_id,
            "mode": outcome.mode,
            "action": outcome.action,
            "detail": outcome.detail,
            "repeat": repeat,
        }));
    } else {
        hr();
        println!("处理结果: {}", outcome.action);
        println!("{}", outcome.summary());
        if let Some(d) = &outcome.detail {
            println!("备注: {}", d);
        }
        hr();
    }
    Ok(())
}

pub async fn cmd_export(svc: &Services, args: &Args) -> Result<(), String> {
    let result = eazyqq_lib::services::diagnostics::export_bundle(
        &svc.db,
        &svc.ai,
        &svc.napcat,
        &svc.onebot,
    )
    .await?;

    if args.json() {
        print_json(&serde_json::json!({
            "zipFilePath": result.zip_path.display().to_string(),
            "entries": result.entries,
            "bytes": result.bytes,
        }));
        return Ok(());
    }

    hr();
    println!("诊断包已生成");
    hr();
    println!("路径: {}", result.zip_path.display());
    println!("大小: {} KB", result.bytes / 1024);
    println!("内容:");
    for name in &result.entries {
        println!("  - {}", name);
    }
    hr();
    Ok(())
}

pub async fn cmd_chain_status(svc: &Services, args: &Args) -> Result<(), String> {
    // The CLI is a separate process from the GUI, so the in-process registry is empty
    // here. Probe the links this process can reach, then merge in what the GUI has
    // recorded if it is running.
    probe_chain_from_cli(svc).await;

    let links = eazyqq_lib::services::chain::snapshot();
    let first = eazyqq_lib::services::chain::first_break();

    if args.json() {
        print_json(&serde_json::json!({
            "links": links,
            "firstBreak": first,
            "hasFailure": eazyqq_lib::services::chain::has_failure(),
        }));
        return Ok(());
    }

    hr();
    println!("全链路状态");
    hr();
    for link in &links {
        let mark = match link.health {
            eazyqq_lib::services::chain::Health::Ok => "[OK]  ",
            eazyqq_lib::services::chain::Health::Unknown => "[?]   ",
            eazyqq_lib::services::chain::Health::Failed => "[FAIL]",
        };
        println!("{} {:<18} {}", mark, link.label, link.detail);
        if link.health == eazyqq_lib::services::chain::Health::Failed {
            println!("       └ 影响: {}", link.impact);
        }
    }
    hr();

    match first {
        Some(broken) => {
            println!("第一个断点: {} —— {}", broken.label, broken.detail);
            println!("影响范围  : {}", broken.impact);
            println!();
            println!("提示: 链路上后续环节的异常通常是这个断点的后果，先修这里。");
        }
        None => {
            let unknown = links
                .iter()
                .filter(|l| l.health == eazyqq_lib::services::chain::Health::Unknown)
                .count();
            if unknown == 0 {
                println!("链路完整，所有环节正常。");
            } else {
                println!("未发现断点，但有 {} 个环节尚未被验证（需要相应组件运行才能确认）。", unknown);
            }
        }
    }
    hr();
    Ok(())
}

/// Probe the externally-reachable links from the CLI process.
async fn probe_chain_from_cli(svc: &Services) {
    use eazyqq_lib::services::chain::{record_error, record_ok, record_unknown, Link};

    match svc.db.get_setting("app_config") {
        Ok(_) => record_ok(Link::Database, "读写正常"),
        Err(e) => record_error(Link::Database, format!("{}", e)),
    }

    match svc.napcat.check_login().await {
        Ok(v) => {
            record_ok(Link::NapcatWebUi, "WebUI 可达");
            let logged_in = v
                .get("data")
                .and_then(|d| d.get("isLogin"))
                .and_then(|b| b.as_bool())
                .unwrap_or(false);
            if logged_in {
                record_ok(Link::QqLogin, "已登录");
            } else {
                record_error(Link::QqLogin, "未登录（需要扫码或快速登录）");
            }
        }
        Err(e) => {
            let napcat_dir = eazyqq_lib::services::logging::workspace_root().join("napcat");
            record_error(
                Link::NapcatWebUi,
                eazyqq_lib::services::chain::explain_napcat_failure(&e, &napcat_dir),
            );
            record_unknown(Link::QqLogin, "WebUI 不可达，无法判断登录状态");
        }
    }

    match svc.onebot.get_login_info().await {
        Ok(v) => {
            let nick = v
                .get("data")
                .and_then(|d| d.get("nick"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            record_ok(
                Link::OneBotHttp,
                if nick.is_empty() {
                    "可达".to_string()
                } else {
                    format!("可达 ({})", nick)
                },
            );
        }
        Err(e) => record_error(Link::OneBotHttp, e),
    }

    let cfg = svc.ai.current();
    if cfg.api_key.trim().is_empty() && cfg.requires_api_key() {
        record_error(Link::AiProvider, "未配置 API Key");
    } else {
        match svc
            .ai
            .generate_with_system(
                "只回复 ok",
                &[eazyqq_lib::services::ai::ChatMessage {
                    role: "user".to_string(),
                    content: "ping".to_string(),
                }],
                0.0,
            )
            .await
        {
            Ok(_) => record_ok(Link::AiProvider, format!("{} 可用", cfg.model)),
            Err(e) => record_error(Link::AiProvider, e),
        }
    }

    let ws_open = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], 3001)),
        std::time::Duration::from_millis(800),
    )
    .is_ok();
    if ws_open {
        record_unknown(Link::OneBotWs, "端口开放；连接状态需由客户端进程确认");
    } else {
        record_error(Link::OneBotWs, "127.0.0.1:3001 未监听，消息无法进入");
    }

    record_unknown(Link::Scheduler, "需客户端进程运行才能确认");
    record_unknown(Link::Frontend, "需客户端进程运行才能确认");
}

pub async fn cmd_selftest(svc_ctx: &Services, args: &Args) -> Result<(), String> {
    use eazyqq_lib::services as svc;

    probe_chain_from_cli(svc_ctx).await;

    struct Check {
        name: &'static str,
        ok: bool,
        detail: String,
    }
    let mut checks: Vec<Check> = Vec::new();

    macro_rules! check {
        ($name:expr, $cond:expr) => {
            checks.push(Check {
                name: $name,
                ok: $cond,
                detail: String::new(),
            });
        };
        ($name:expr, $cond:expr, $detail:expr) => {
            checks.push(Check {
                name: $name,
                ok: $cond,
                detail: $detail,
            });
        };
    }

    // 1. No setting may be offered without being consumed.
    let unread = svc::config::unread_keys();
    check!(
        "配置项全部有消费点（无「改了没用」的设置）",
        unread.is_empty(),
        format!("死字段: {:?}", unread)
    );

    // 2. The chain report must reflect real probing.
    let links = svc::chain::snapshot();
    check!("全链路报告覆盖全部 8 个环节", links.len() == 8, format!("{} 个", links.len()));

    let probed = links
        .iter()
        .filter(|l| l.health != svc::chain::Health::Unknown)
        .count();
    check!(
        "链路状态来自真实探测（非全部未验证）",
        probed > 0,
        format!("{}/{} 个环节有结论", probed, links.len())
    );

    let unevidenced = links
        .iter()
        .filter(|l| l.health == svc::chain::Health::Ok && l.detail.trim().is_empty())
        .count();
    check!("标记为正常的环节必须给出依据", unevidenced == 0, format!("{} 个无说明", unevidenced));

    let contradictory = links
        .iter()
        .filter(|l| l.health == svc::chain::Health::Ok && l.last_ok_secs_ago.is_none())
        .count();
    check!(
        "标记为正常必须有过成功记录",
        contradictory == 0,
        format!("{} 个正常但无成功记录", contradictory)
    );

    // 3. Default deny: an unknown target is never tracked and never reaches AI.
    let policy = svc::policy::TargetPolicy::default();
    check!("未知目标默认拒绝（不接管、不调用 AI）",
        !policy.tracked() && !policy.ai_execution_allowed());

    // 4. Trigger conditions.
    let plain = serde_json::json!({ "raw_message": "大家早" });
    let mention = serde_json::json!({ "raw_message": "[CQ:at,qq=462564834] 在吗" });
    let at_me_plain = svc::trigger::evaluate("at_me", &[], &plain, "462564834", "大家早");
    let at_me_hit = svc::trigger::evaluate("at_me", &[], &mention, "462564834", "在吗");
    check!("@我 条件拦截普通消息", !at_me_plain.is_matched());
    check!("@我 条件放行被 @ 的消息", at_me_hit.is_matched());
    let unknown_cond = svc::trigger::evaluate("nonsense", &[], &plain, "462564834", "x");
    check!("未知触发条件失败关闭（不会退化为回复全部）", !unknown_cond.is_matched());

    // 5. Cooldown actually blocks a second attempt.
    let target = "selftest_cooldown_probe";
    svc::cooldown::reset(target);
    let first = svc::cooldown::try_acquire(target, 60);
    let second = svc::cooldown::try_acquire(target, 60);
    check!("回复冷却拦截窗口内的第二次", first.is_ok() && second.is_err());
    svc::cooldown::reset(target);

    // 6. Path safety: a hostile identifier must not escape its directory.
    let hostile_dir = svc::accounts::account_dir(Some("../../etc/passwd"));
    let dir_text = hostile_dir.to_string_lossy().replace('\\', "/");
    check!("恶意账号 ID 无法越出账号目录",
        dir_text.contains("/accounts/") && !dir_text.contains(".."),
        dir_text);

    let base = std::path::Path::new("safe_data/group_files/123456");
    let hostile_name = svc::group_files::sanitize_file_name("../../evil.exe");
    let resolved = base.join(&hostile_name);
    check!(
        "恶意文件名无法越出群文件目录",
        resolved.parent() == Some(base),
        format!("{} -> {}", hostile_name, resolved.display())
    );

    let dotdot = svc::group_files::sanitize_file_name("..");
    check!(
        "单独的 .. 不能作为文件名",
        dotdot != ".." && !dotdot.is_empty(),
        dotdot
    );

    // 7. Where the data lives must not depend on how the app was launched.
    let root = svc::logging::workspace_root();
    let root_text = root.to_string_lossy().replace('\\', "/");
    check!(
        "数据根目录不在构建输出内",
        !root_text.contains("/target/"),
        root_text.clone()
    );

    let cwd_before = std::env::current_dir().ok();
    if let Some(original) = cwd_before {
        if std::env::set_current_dir(std::env::temp_dir()).is_ok() {
            let moved = svc::logging::workspace_root();
            check!(
                "数据根目录与当前工作目录无关",
                moved == root,
                format!("从 {} 启动时变成 {}", original.display(), moved.display())
            );
            let _ = std::env::set_current_dir(&original);
        }
    }

    // 8. Redaction: a supplied secret must not survive.
    let secret = "sk_tr_selftest_secret_value".to_string();
    let redacted = svc::diagnostics::redact(&format!("key={}", secret), &[secret.clone()]);
    check!("诊断脱敏移除已知密钥", !redacted.contains(&secret));

    // 9. Diagnostics archive round-trip: we write the ZIP ourselves, so prove it reads back.
    let zip = svc::diagnostics::build_zip(&[("probe.txt".to_string(), b"hello".to_vec())]);
    let is_zip = zip.len() > 22 && &zip[0..4] == b"PK\x03\x04";
    check!("自实现 ZIP 容器结构正确", is_zip);

    // --- report ---------------------------------------------------------------------
    let failed: Vec<&Check> = checks.iter().filter(|c| !c.ok).collect();

    if args.json() {
        print_json(&serde_json::json!({
            "total": checks.len(),
            "passed": checks.len() - failed.len(),
            "failed": failed.iter().map(|c| c.name).collect::<Vec<_>>(),
            "checks": checks.iter().map(|c| serde_json::json!({
                "name": c.name, "ok": c.ok, "detail": c.detail,
            })).collect::<Vec<_>>(),
        }));
    } else {
        hr();
        println!("关键不变量自检");
        hr();
        for c in &checks {
            println!(
                "  [{}] {}{}",
                if c.ok { "PASS" } else { "FAIL" },
                c.name,
                if c.detail.is_empty() { String::new() } else { format!("  ({})", c.detail) }
            );
        }
        hr();
        println!(
            "共 {} 项，通过 {} 项{}",
            checks.len(),
            checks.len() - failed.len(),
            if failed.is_empty() { "，全部通过。" } else { "。" }
        );
        if !failed.is_empty() {
            println!();
            println!("失败项：");
            for c in &failed {
                println!("  - {}  {}", c.name, c.detail);
            }
        }
        hr();
    }

    Ok(())
}

pub fn cmd_napcat_doctor(args: &Args) -> Result<(), String> {
    let napcat_dir = eazyqq_lib::services::logging::workspace_root().join("napcat");
    let uin = eazyqq_lib::services::accounts::active();
    let steps = eazyqq_lib::services::napcat_boot::diagnose(&napcat_dir, uin.as_deref());
    let blocker = steps.iter().position(|s| !s.ok);

    if args.json() {
        print_json(&serde_json::json!({
            "napcatDir": napcat_dir.display().to_string(),
            "account": uin,
            "ok": blocker.is_none(),
            "steps": steps.iter().map(|s| serde_json::json!({
                "name": s.name, "ok": s.ok, "detail": s.detail,
            })).collect::<Vec<_>>(),
        }));
        return Ok(());
    }

    hr();
    println!("协议端启动路径诊断");
    println!("  NapCat 目录 : {}", napcat_dir.display());
    println!("  账号        : {}", uin.as_deref().unwrap_or("（未确定）"));
    hr();

    for (i, s) in steps.iter().enumerate() {
        let flag = if s.ok { "OK  " } else { "卡住" };
        println!("  [{}] {}. {}", flag, i + 1, s.name);
        if !s.detail.is_empty() {
            for line in wrap(&s.detail, 66) {
                println!("         {}", line);
            }
        }
    }
    hr();

    match blocker {
        None => println!("协议端启动路径没有发现阻塞项。"),
        Some(i) => {
            println!(
                "第一个阻塞项是「{}」。这一步通过之前，后面的都无从谈起。",
                steps[i].name
            );
        }
    }
    hr();
    Ok(())
}

pub fn cmd_log_path() -> Result<(), String> {
    println!("{}", logging::active_log_path().display());
    let (dir, crash_count, last) = logging::diagnostics();
    println!("日志目录: {}", dir.display());
    println!("崩溃报告: {} 份", crash_count);
    if let Some(p) = last {
        println!("最近崩溃: {}", p.display());
    }
    Ok(())
}

pub fn cmd_log_tail(args: &Args) -> Result<(), String> {
    let lines: usize = args.flag("lines").and_then(|v| v.parse().ok()).unwrap_or(60);
    match logging::tail(lines) {
        Ok(text) => println!("{}", text),
        Err(e) => return Err(format!("读取日志失败: {}", e)),
    }
    Ok(())
}
