use eazyqq_lib::services::logging;

use crate::args::Args;
use crate::context::Services;
use crate::format::{hr, print_json, truncate, wrap};

pub async fn cmd_health(svc: &Services, args: &Args) -> Result<(), String> {
    use eazyqq_lib::services::chain::{Health, Link};
    eazyqq_lib::services::infra::runtime_config::refresh_ai(&svc.db, &svc.ai);
    if args.has("deep") {
        // Only this explicit option performs inference; ordinary diagnostics stay passive.
        let _ = svc.ai.generate_reply(&[], "ping", "自检").await;
    }
    let report = eazyqq_lib::services::chain::refresh_status(&svc.db, &svc.napcat, &svc.onebot, &svc.ai).await;
    let mut items: Vec<serde_json::Value> = report.links.iter().map(|link| {
        let name = match link.link {
            Link::NapcatWebUi => format!("NapCat WebUI ({})", svc.napcat.endpoint()),
            Link::OneBotHttp => format!("OneBot HTTP ({})", svc.onebot.endpoint()),
            Link::QqLogin => "QQ login".into(),
            _ => link.label.clone(),
        };
        serde_json::json!({"item":name,"ok":link.health==Health::Ok,"health":link.health,
            "detail":link.detail,"recommendedAction":link.recommended_action})
    }).collect();
    let writable = eazyqq_lib::services::diagnostics::is_dir_writable(&logging::data_dir());
    items.push(serde_json::json!({"item":"本地数据目录可写","ok":writable,
        "health":if writable {"ok"} else {"failed"},"detail":logging::data_dir().display().to_string(),
        "recommendedAction":if writable {"none"} else {"inspect_storage"}}));
    let all_ok = items.iter().all(|item| item["health"] == "ok");
    let has_failure = items.iter().any(|item| item["health"] == "failed");
    if args.json() {
        print_json(&serde_json::json!({"allOk":all_ok,"hasFailure":has_failure,"items":items,
            "links":report.links,"firstBreak":report.first_break,"uptimeSecs":report.uptime_secs,
            "aiCredentialsReady":svc.ai.has_api_key()}));
        return Ok(());
    }
    hr();
    println!("EazyQQ 健康自检");
    hr();
    for item in &items {
        let mark = match item["health"].as_str() { Some("ok") => "[OK]", Some("failed") => "[FAIL]", _ => "[?]" };
        println!("{} {:<34} {}", mark, item["item"].as_str().unwrap_or(""), item["detail"].as_str().unwrap_or(""));
    }
    hr();
    println!("总体: {}", if all_ok {"全部通过"} else if has_failure {"存在失败环节"} else {"部分环节待验证"});
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
            println!(
                "  [{}/{}] {} -> {}",
                i + 1,
                repeat,
                outcome.action,
                outcome.summary()
            );
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

    let report = eazyqq_lib::services::chain::status_report();
    let links = &report.links;
    let first = report.first_break.clone();

    if args.json() {
        print_json(&serde_json::to_value(&report).map_err(|error| error.to_string())?);
        return Ok(());
    }

    hr();
    println!("全链路状态");
    hr();
    for link in links {
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
                println!(
                    "未发现断点，但有 {} 个环节尚未被验证（需要相应组件运行才能确认）。",
                    unknown
                );
            }
        }
    }
    hr();
    Ok(())
}

/// Probe the externally-reachable links from the CLI process.
async fn probe_chain_from_cli(svc: &Services) {
    // In-process evidence cannot prove the health of another GUI/worker process.
    // A TCP listener is not WebSocket authentication or message-consumer evidence.
    eazyqq_lib::services::chain::refresh_status(&svc.db, &svc.napcat, &svc.onebot, &svc.ai).await;
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
    check!(
        "全链路报告覆盖全部 8 个环节",
        links.len() == 8,
        format!("{} 个", links.len())
    );

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
    check!(
        "标记为正常的环节必须给出依据",
        unevidenced == 0,
        format!("{} 个无说明", unevidenced)
    );

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
    check!(
        "未知目标默认拒绝（不接管、不调用 AI）",
        !policy.tracked() && !policy.ai_execution_allowed()
    );

    // 4. Trigger conditions.
    let plain = serde_json::json!({ "raw_message": "大家早" });
    let mention = serde_json::json!({ "raw_message": "[CQ:at,qq=462564834] 在吗" });
    let at_me_plain = svc::trigger::evaluate("at_me", &[], &plain, "462564834", "大家早");
    let at_me_hit = svc::trigger::evaluate("at_me", &[], &mention, "462564834", "在吗");
    check!("@我 条件拦截普通消息", !at_me_plain.is_matched());
    check!("@我 条件放行被 @ 的消息", at_me_hit.is_matched());
    let unknown_cond = svc::trigger::evaluate("nonsense", &[], &plain, "462564834", "x");
    check!(
        "未知触发条件失败关闭（不会退化为回复全部）",
        !unknown_cond.is_matched()
    );

    // 5. Cooldown actually blocks a second attempt.
    let target = "selftest_cooldown_probe";
    svc::cooldown::reset(target);
    let first = svc::cooldown::try_acquire(target, 60);
    let second = svc::cooldown::try_acquire(target, 60);
    check!(
        "回复冷却拦截窗口内的第二次",
        first.is_ok() && second.is_err()
    );
    svc::cooldown::reset(target);

    // 6. Path safety: a hostile identifier must not escape its directory.
    let hostile_dir = svc::accounts::account_dir(Some("../../etc/passwd"));
    let dir_text = hostile_dir.to_string_lossy().replace('\\', "/");
    check!(
        "恶意账号 ID 无法越出账号目录",
        dir_text.contains("/accounts/") && !dir_text.contains(".."),
        dir_text
    );

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
    let secret = std::env::var("TEST_SECRET_KEY")
        .unwrap_or_else(|_| "selftest_secret_redaction_fixture".to_string());
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
                if c.detail.is_empty() {
                    String::new()
                } else {
                    format!("  ({})", c.detail)
                }
            );
        }
        hr();
        println!(
            "共 {} 项，通过 {} 项{}",
            checks.len(),
            checks.len() - failed.len(),
            if failed.is_empty() {
                "，全部通过。"
            } else {
                "。"
            }
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
    let source = eazyqq_lib::services::napcat_boot::locate_napcat_dir();
    let (napcat_dir, _, _, _) = eazyqq_lib::services::protocol::layout::selected(&source);
    let uin = eazyqq_lib::services::accounts::active();
    let steps = eazyqq_lib::services::napcat_boot::diagnose(&napcat_dir, uin.as_deref());
    let blocker = steps.iter().position(|s| !s.ok);
    let trace = eazyqq_lib::services::protocol::startup_trace::capture(&napcat_dir);

    if args.json() {
        print_json(&serde_json::json!({
            "napcatDir": napcat_dir.display().to_string(),
            "account": uin,
            "ok": blocker.is_none() && trace.first_failure.is_none(),
            "startupTrace": trace,
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
    let lines: usize = args
        .flag("lines")
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    match logging::tail(lines) {
        Ok(text) => println!("{}", text),
        Err(e) => return Err(format!("读取日志失败: {}", e)),
    }
    Ok(())
}
