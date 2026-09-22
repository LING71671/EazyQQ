//! `eazyqq-cli` - headless control channel for EazyQQ.
//!
//! Everything the GUI can do is reachable from here, so the backend can be driven and
//! regression-tested on a real QQ account without touching the UI (the GUI itself can
//! be blocked by machine-level WebView2 problems, see ISSUE-013).
//!
//! Usage: `cargo run --bin eazyqq-cli -- <command> [options]`
//! Run without arguments to print the full command list.

use std::collections::HashMap;
use std::process::ExitCode;
use std::sync::Arc;

use eazyqq_lib::models::MessageItemDto;
use eazyqq_lib::services::ai::AiService;
use eazyqq_lib::services::contacts;
use eazyqq_lib::services::db::{ContactRuleRecord, Database};
use eazyqq_lib::services::logging;
use eazyqq_lib::services::napcat::NapCatService;
use eazyqq_lib::services::onebot::OneBotClient;
use eazyqq_lib::services::policy;

const NAPCAT_WEBUI: &str = "http://127.0.0.1:6099";
const ONEBOT_HTTP: &str = "http://127.0.0.1:3000";

// ---------------------------------------------------------------------------
// Tiny argument parser (no clap dependency on purpose)
// ---------------------------------------------------------------------------

struct Args {
    command: String,
    flags: HashMap<String, String>,
    switches: Vec<String>,
    /// Bare positional arguments after the command, reserved for future use
    /// (e.g. `eazyqq-cli rule 1104661022 --mode copilot`).
    #[allow(dead_code)]
    positional: Vec<String>,
}

impl Args {
    fn parse() -> Self {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut command = String::new();
        let mut flags = HashMap::new();
        let mut switches = Vec::new();
        let mut positional = Vec::new();

        let mut i = 0;
        while i < raw.len() {
            let token = &raw[i];
            if let Some(name) = token.strip_prefix("--") {
                // `--key=value` form
                if let Some((k, v)) = name.split_once('=') {
                    flags.insert(k.to_string(), v.to_string());
                } else if i + 1 < raw.len() && !raw[i + 1].starts_with("--") {
                    flags.insert(name.to_string(), raw[i + 1].clone());
                    i += 1;
                } else {
                    switches.push(name.to_string());
                }
            } else if command.is_empty() {
                command = token.clone();
            } else {
                positional.push(token.clone());
            }
            i += 1;
        }

        Args {
            command,
            flags,
            switches,
            positional,
        }
    }

    fn flag(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(|s| s.as_str())
    }

    fn has(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name) || self.flags.contains_key(name)
    }

    fn json(&self) -> bool {
        self.has("json")
    }
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

fn hr() {
    println!("{}", "-".repeat(64));
}

fn print_json(value: &serde_json::Value) {
    match serde_json::to_string_pretty(value) {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("JSON encode error: {}", e),
    }
}

fn mode_label(mode: &str) -> &'static str {
    match mode {
        "auto_reply" => "自动秒回",
        "copilot" => "草稿审核",
        "ignore" => "直通忽略",
        other => Box::leak(format!("未知({})", other).into_boxed_str()),
    }
}

fn rule_to_json(r: &ContactRuleRecord) -> serde_json::Value {
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

// ---------------------------------------------------------------------------
// Service construction (mirrors lib.rs so the CLI and GUI share one data dir)
// ---------------------------------------------------------------------------

struct Services {
    db: Arc<Database>,
    napcat: Arc<NapCatService>,
    onebot: Arc<OneBotClient>,
    ai: Arc<AiService>,
}

impl Services {
    fn build() -> Result<Self, String> {
        let root = logging::workspace_root();
        let data_dir = logging::data_dir();
        logging::ensure_dir(&data_dir);

        let db = Arc::new(
            Database::init(data_dir.join("eazyqq.db"))
                .map_err(|e| format!("cannot open SQLite database: {}", e))?,
        );

        let napcat_dir = if root.join("napcat").exists() {
            root.join("napcat")
        } else {
            std::path::PathBuf::from("B:\\EazyQQ\\napcat")
        };

        // No fallback token: it is read from `napcat/config/webui.json`. A token is
        // generated per installation, so a hardcoded one would fail on any other machine
        // and would ship a credential in the source tree.
        let napcat = Arc::new(NapCatService::new(
            NAPCAT_WEBUI.to_string(),
            String::new(),
            napcat_dir.to_string_lossy().to_string(),
        ));

        let onebot = Arc::new(OneBotClient::new(ONEBOT_HTTP.to_string()));

        let api_key = std::env::var("TOKENRHYTHM_API_KEY")
            .or_else(|_| std::env::var("AI_API_KEY"))
            .ok()
            .or_else(|| db.get_setting("tokenrhythm_api_key").ok().flatten())
            .unwrap_or_default();

        // Same resolution path as the GUI, so the CLI reports what the app really uses.
        let ai_config = {
            let raw = db.get_setting("app_config").ok().flatten();
            let parsed = raw
                .as_deref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
            eazyqq_lib::services::ai::AiRuntimeConfig::from_app_config(
                parsed.as_ref().and_then(|v| v.get("ai")),
                &api_key,
            )
        };
        tracing::info!("active AI provider -> {}", ai_config.describe());
        let ai = Arc::new(AiService::new(ai_config));

        Ok(Services {
            db,
            napcat,
            onebot,
            ai,
        })
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn cmd_help() {
    println!(
        r#"EazyQQ CLI - 无界面控制通道 (v{version})

用法: eazyqq-cli <命令> [选项]

协议与登录
  status                          协议进程、登录态、二维码可用性一览
  login-info                      当前登录 QQ 号与昵称
  quick-login-list                列出可免扫码快速登录的账号
  quick-login --uin <QQ号>        对指定账号执行快速登录（免扫码）
  qr [--save <path>]              获取真实登录二维码（未登录时有效）

联系人与规则（默认全部拒绝）
  contacts [--type friend|group]  同步真实好友/群列表并列出规则
           [--search <关键词>] [--whitelist] [--summary-only]
  groups                          原始群列表（直连 OneBot）
  friends                         原始好友列表（直连 OneBot）
  rule --target <id>              修改规则
       [--mode auto_reply|copilot|ignore]
       [--summary on|off] [--interval-hours <N>]
       [--trigger all|at_me|keyword] [--keywords a,b,c]

消息与 AI
  send --target <id> --text <文本>  真实发送消息（群号自动识别为群聊）
  ask  --target <id> --text <提示>  AI 生成一条候选回复（不发送）
  drafts                          列出待审核草稿
  draft-send --id <草稿ID>        放行草稿并真实发送 [--text <覆盖内容>]
  draft-dismiss --id <草稿ID>     丢弃草稿
  draft-regenerate --id <草稿ID> --instruction <微调指令>  按指令重新生成
  history --target <id> [--limit N] 查看本地消息流水
  mark-read --target <id> [--at <毫秒时间戳>]  清除该会话未读标记

简报
  summarize --target <群号> [--hours N] [--prompt <提示词>]   立即生成真实 AI 简报
  scheduler-tick                  强制执行一次定时总结调度（与后台循环同一逻辑）
  summaries [--target <id>]       列出已生成的群简报
  whitelist-groups                列出已加入简报白名单的群

群文件
  files --target <群号>           同步并列出群文件（含本地下载状态）
  file-download --target <群号> --file-id <文件ID>   下载到工作区
  file-summarize --path <本地路径>  提取文档文本并生成结构化综述

配置与诊断
  config [--json]                 打印当前配置
  set-config --key <k> --value <v> 写入 app_settings
  ai-config                       查看实际生效的大模型供应商配置
  ai-detect                       探测本机可用的 OpenAI 兼容推理端点
  ai-set --provider <p>           切换供应商（省 token 可指向本地端点）
         [--base-url <u>] [--model <m>] [--key <k>]
         [--temperature <t>] [--max-context <n>]
  ai-test                         对当前供应商发起真实连通性测试
  chain-status                    全链路状态：逐环节体检并定位第一个断点
  health [--deep]                 依赖与链路自检（--deep 会真实调用大模型）
  export                          导出脱敏诊断包 zip
  simulate --target <id> --text <消息>  调试后门：注入模拟消息走完整处理链路
           [--type group|friend] [--sender-name <名>] [--sender-id <号>]
  log-path                        打印日志文件路径
  log-tail [--lines N]            查看日志尾部
  version                         版本信息

通用选项
  --json                          以 JSON 输出，便于脚本消费
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}

async fn cmd_status(svc: &Services, args: &Args) -> Result<(), String> {
    let napcat_alive = svc.napcat.is_alive().await;
    let login_info = svc.onebot.get_login_info().await;
    let webui_status = svc.napcat.check_login().await;

    let (logged_in, uin, nickname) = match &login_info {
        Ok(v) => {
            let data = v.get("data");
            let uin = data
                .and_then(|d| d.get("user_id"))
                .and_then(|v| v.as_i64())
                .map(|n| n.to_string());
            let nick = data
                .and_then(|d| d.get("nickname"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            (uin.is_some(), uin, nick)
        }
        Err(_) => (false, None, None),
    };

    if args.json() {
        print_json(&serde_json::json!({
            "napcatWebUiAlive": napcat_alive,
            "onebotReachable": login_info.is_ok(),
            "loggedIn": logged_in,
            "qqNumber": uin,
            "nickname": nickname,
            "webuiLoginStatus": webui_status.ok(),
        }));
        return Ok(());
    }

    hr();
    println!("EazyQQ 运行状态");
    hr();
    println!(
        "NapCat WebUI (6099) : {}",
        if napcat_alive { "在线" } else { "离线" }
    );
    println!(
        "OneBot HTTP (3000)  : {}",
        match &login_info {
            Ok(_) => "可达".to_string(),
            Err(e) => format!("不可达 ({})", e),
        }
    );
    println!(
        "登录状态            : {}",
        if logged_in { "已登录" } else { "未登录" }
    );
    if logged_in {
        println!("QQ 账号             : {}", uin.unwrap_or_default());
        println!("昵称                : {}", nickname.unwrap_or_default());
    }
    if !logged_in {
        println!();
        println!("提示: 未登录时可用 `eazyqq-cli qr` 获取二维码后手机 QQ 扫码。");
    }
    hr();
    Ok(())
}

async fn cmd_login_info(svc: &Services, args: &Args) -> Result<(), String> {
    let info = svc
        .onebot
        .get_login_info()
        .await
        .map_err(|e| format!("查询登录信息失败: {}", e))?;
    if args.json() {
        print_json(&info);
    } else {
        let data = info.get("data").cloned().unwrap_or(serde_json::json!({}));
        println!("QQ 账号 : {}", data.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0));
        println!(
            "昵称    : {}",
            data.get("nickname").and_then(|v| v.as_str()).unwrap_or("<未知>")
        );
    }
    Ok(())
}

async fn cmd_qr(svc: &Services, args: &Args) -> Result<(), String> {
    let qr = svc.napcat.get_qrcode().await?;
    if let Some(path) = args.flag("save") {
        std::fs::write(path, &qr).map_err(|e| format!("写入 {} 失败: {}", path, e))?;
        println!("二维码已保存到 {}", path);
    }
    if args.json() {
        print_json(&serde_json::json!({ "qrcode": qr }));
    } else {
        hr();
        println!("真实登录二维码 (NapCat / NTQQ 签发):");
        hr();
        println!("{}", qr);
        hr();
        println!("请用手机 QQ 扫码登录。");
    }
    Ok(())
}

async fn cmd_contacts(svc: &Services, args: &Args) -> Result<(), String> {
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

fn truncate(s: &str, width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= width {
        s.to_string()
    } else {
        format!("{}…", chars[..width.saturating_sub(1)].iter().collect::<String>())
    }
}

async fn cmd_raw_roster(svc: &Services, args: &Args, groups: bool) -> Result<(), String> {
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

async fn cmd_rule(svc: &Services, args: &Args) -> Result<(), String> {
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

async fn cmd_send(svc: &Services, args: &Args) -> Result<(), String> {
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

    let resolved_type = if target_type == "auto" {
        let groups = svc.onebot.get_group_list().await.unwrap_or_default();
        if groups.iter().any(|g| g.group_id.to_string() == target) {
            "group"
        } else {
            "friend"
        }
    } else {
        target_type
    };

    let resp = svc.onebot.send_msg(resolved_type, &target, &text).await?;
    let ok = resp.get("status").and_then(|s| s.as_str()) == Some("ok");
    if !ok {
        return Err(format!("发送失败: {}", resp));
    }

    let message_id = resp
        .get("data")
        .and_then(|d| d.get("message_id"))
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_string());

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

async fn cmd_ask(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <目标ID>".to_string())?
        .to_string();
    let prompt = args
        .flag("text")
        .ok_or_else(|| "缺少 --text <对方消息或提示>".to_string())?
        .to_string();

    let name = svc
        .db
        .get_all_rules()
        .unwrap_or_default()
        .into_iter()
        .find(|r| r.target_id == target)
        .map(|r| r.name)
        .unwrap_or_else(|| target.clone());

    let (content, thinking) = svc
        .ai
        .generate_reply(&[], &prompt, &name)
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

async fn cmd_drafts(svc: &Services, args: &Args) -> Result<(), String> {
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

async fn cmd_history(svc: &Services, args: &Args) -> Result<(), String> {
    let target = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <目标ID>".to_string())?;
    let limit: usize = args.flag("limit").and_then(|v| v.parse().ok()).unwrap_or(20);
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

async fn cmd_summaries(svc: &Services, args: &Args) -> Result<(), String> {
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
        println!("{}  [{}] {}", s.created_at, s.target_name, truncate(&s.summary_text, 80));
    }
    hr();
    Ok(())
}

async fn cmd_whitelist_groups(svc: &Services, args: &Args) -> Result<(), String> {
    let list = svc.db.get_summary_whitelist_groups().map_err(|e| e.to_string())?;
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

async fn cmd_config(svc: &Services, args: &Args) -> Result<(), String> {
    let raw = svc.db.get_setting("app_config").ok().flatten();
    let value: serde_json::Value = raw
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| serde_json::json!({ "note": "app_config 尚未写入，使用代码内默认值" }));

    if args.json() {
        print_json(&value);
    } else {
        hr();
        println!("当前配置 (app_settings.app_config)");
        hr();
        print_json(&value);
        hr();
    }
    Ok(())
}

async fn cmd_set_config(svc: &Services, args: &Args) -> Result<(), String> {
    let key = args
        .flag("key")
        .ok_or_else(|| "缺少 --key <配置键>".to_string())?;

    // `--file` is the practical path for JSON blobs, since shell quoting of a whole
    // JSON object is fragile. `--value` stays for short scalar settings.
    let value = if let Some(path) = args.flag("file") {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| format!("读取 {} 失败: {}", path, e))?;
        // Validate JSON so a broken config can never be persisted.
        serde_json::from_str::<serde_json::Value>(&raw)
            .map_err(|e| format!("{} 不是合法 JSON: {}", path, e))?;
        raw
    } else {
        args.flag("value")
            .ok_or_else(|| "缺少 --value <配置值> 或 --file <json文件>".to_string())?
            .to_string()
    };

    svc.db
        .set_setting(key, &value)
        .map_err(|e| format!("写入配置失败: {}", e))?;

    tracing::info!("config written: {} ({} bytes)", key, value.len());
    println!("已写入 {} ({} 字节)", key, value.len());
    Ok(())
}

async fn cmd_health(svc: &Services, args: &Args) -> Result<(), String> {
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
///
/// This runs the exact same code path as the live WebSocket listener (whitelist gate ->
/// persistence -> AI execution), so the whole chain can be verified without a second QQ
/// account and without any UI interaction.
async fn cmd_simulate(svc: &Services, args: &Args) -> Result<(), String> {
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

/// Approve a draft and really send it (Phase 3 acceptance step).
async fn cmd_draft_send(svc: &Services, args: &Args) -> Result<(), String> {
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
    let target_type = if draft.target_type == "group" { "group" } else { "private" };

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

    let outgoing = MessageItemDto {
        id: format!("draft_out_{}", now_ms),
        target_id: draft.target_id.clone(),
        sender_id: "me".to_string(),
        sender_name: "我 (草稿放行)".to_string(),
        content: content.clone(),
        is_from_me: true,
        ai_reply_status: "auto_replied".to_string(),
        timestamp: now_ms,
    };
    svc.db.save_message(&outgoing).map_err(|e| e.to_string())?;
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

fn cmd_draft_dismiss(svc: &Services, args: &Args) -> Result<(), String> {
    let id = args
        .flag("id")
        .ok_or_else(|| "缺少 --id <草稿ID>".to_string())?;
    svc.db.delete_draft(id).map_err(|e| e.to_string())?;
    println!("草稿 {} 已丢弃", id);
    Ok(())
}

/// Regenerate a draft with a human refinement instruction.
async fn cmd_draft_regenerate(svc: &Services, args: &Args) -> Result<(), String> {
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

    let (content, thinking) = svc
        .ai
        .regenerate_with_instruction(
            &draft.incoming_message_snippet,
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

/// Generate a real summary for one target (Phase 5 acceptance step).
async fn cmd_summarize(svc: &Services, args: &Args) -> Result<(), String> {
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

    let outcome = eazyqq_lib::services::summarizer::generate(&svc.db, &svc.ai, &req).await?;

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
async fn cmd_scheduler_tick(svc: &Services, args: &Args) -> Result<(), String> {
    let settings = eazyqq_lib::services::scheduler::read_settings(&svc.db);
    println!(
        "定时总结: {} | 滑动窗口 {}h | Prompt: {}",
        if settings.enabled { "已启用" } else { "已暂停" },
        settings.sliding_window_hours,
        truncate(&settings.custom_prompt, 40)
    );

    let outcomes = eazyqq_lib::services::scheduler::tick(&svc.db, &svc.ai, None).await;

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

/// Export a desensitised diagnostics bundle (Phase 6 acceptance step).
async fn cmd_export(svc: &Services, args: &Args) -> Result<(), String> {
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

/// List a group's files, mirroring the remote list into SQLite first (Phase 4).
async fn cmd_files(svc: &Services, args: &Args) -> Result<(), String> {
    let group = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <群号>".to_string())?;

    let records = eazyqq_lib::services::group_files::sync_group_files(&svc.db, &svc.onebot, group)
        .await
        .unwrap_or_default();

    if args.json() {
        print_json(&serde_json::to_value(&records).unwrap_or(serde_json::json!([])));
        return Ok(());
    }

    hr();
    println!("群文件 ({} 个) - 群 {}", records.len(), group);
    hr();
    if records.is_empty() {
        println!("(该群没有文件，或 OneBot 未返回文件列表)");
    }
    for f in &records {
        println!(
            "{}  {:<34} {:>8} KB  [{}]{}",
            f.file_id,
            truncate(&f.file_name, 34),
            f.file_size / 1024,
            f.download_status,
            f.local_path
                .as_ref()
                .map(|p| format!("  -> {}", p))
                .unwrap_or_default()
        );
    }
    hr();
    Ok(())
}

async fn cmd_file_download(svc: &Services, args: &Args) -> Result<(), String> {
    let group = args
        .flag("target")
        .ok_or_else(|| "缺少 --target <群号>".to_string())?;
    let file_id = args
        .flag("file-id")
        .ok_or_else(|| "缺少 --file-id <文件ID>（可用 `eazyqq-cli files` 查看）".to_string())?;

    let path =
        eazyqq_lib::services::group_files::download_group_file(&svc.db, &svc.onebot, group, file_id)
            .await?;

    if args.json() {
        print_json(&serde_json::json!({ "localSavePath": path.display().to_string() }));
    } else {
        println!("已下载 -> {}", path.display());
    }
    Ok(())
}

async fn cmd_file_summarize(svc: &Services, args: &Args) -> Result<(), String> {
    let path = args
        .flag("path")
        .ok_or_else(|| "缺少 --path <本地文件路径>".to_string())?;

    let result = eazyqq_lib::services::group_files::summarize_file(&svc.ai, path).await?;

    if args.json() {
        print_json(&serde_json::to_value(&result).unwrap_or(serde_json::json!({})));
        return Ok(());
    }

    hr();
    println!("文档综述 -> {}", result.file_name);
    println!("大小 {} KB | 提取字符 {}", result.file_size / 1024, result.total_chars);
    hr();
    println!("{}", result.summary_text);
    if !result.key_takeaways.is_empty() {
        println!();
        println!("[核心论点]");
        for t in &result.key_takeaways {
            println!("  - {}", t);
        }
    }
    if !result.action_items.is_empty() {
        println!();
        println!("[待办事项]");
        for a in &result.action_items {
            println!("  - {}", a);
        }
    }
    hr();
    Ok(())
}

/// List accounts available for password-free quick login (Phase 1).
async fn cmd_quick_login_list(svc: &Services, args: &Args) -> Result<(), String> {
    let res = svc.napcat.get_quick_login_list().await?;

    if args.json() {
        print_json(&res);
        return Ok(());
    }

    let accounts = res
        .get("data")
        .and_then(|d| d.as_array())
        .cloned()
        .unwrap_or_default();

    hr();
    println!("可快速登录的账号 ({} 个)", accounts.len());
    hr();
    if accounts.is_empty() {
        println!("(无) 说明 NapCat 未记住任何账号，需要扫码登录一次。");
    }
    for acc in accounts {
        println!(
            "{}  {:<16} {}",
            acc.get("uin").and_then(|v| v.as_str()).unwrap_or("-"),
            acc.get("nickName").and_then(|v| v.as_str()).unwrap_or("-"),
            acc.get("faceUrl").and_then(|v| v.as_str()).unwrap_or("")
        );
    }
    hr();
    Ok(())
}

/// Perform a password-free quick login.
async fn cmd_quick_login(svc: &Services, args: &Args) -> Result<(), String> {
    let uin = args
        .flag("uin")
        .ok_or_else(|| "缺少 --uin <QQ号>（可用 `eazyqq_cli quick-login-list` 查看）".to_string())?;

    tracing::info!("quick login requested for {}", uin);
    let res = svc.napcat.set_quick_login(uin).await?;

    let code = res.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 0 {
        return Err(format!(
            "快速登录失败: {}",
            res.get("message").and_then(|m| m.as_str()).unwrap_or("未知错误")
        ));
    }

    if args.json() {
        print_json(&res);
        return Ok(());
    }
    println!("已触发账号 {} 的快速登录，请稍候在 `eazyqq_cli status` 中确认登录状态。", uin);
    Ok(())
}

/// Show the AI provider configuration that is actually in effect.
fn cmd_ai_config(svc: &Services, args: &Args) -> Result<(), String> {
    let cfg = svc.ai.current();

    if args.json() {
        print_json(&serde_json::json!({
            "provider": cfg.provider,
            "baseUrl": cfg.base_url,
            "model": cfg.model,
            "temperature": cfg.temperature,
            "maxContextMessages": cfg.max_context_messages,
            "requiresApiKey": cfg.requires_api_key(),
            "apiKeyConfigured": !cfg.api_key.trim().is_empty(),
        }));
        return Ok(());
    }

    hr();
    println!("大模型供应商配置（实际生效值）");
    hr();
    println!("供应商       : {}", cfg.provider);
    println!("接口地址     : {}", cfg.base_url);
    println!("模型         : {}", cfg.model);
    println!("温度         : {}", cfg.temperature);
    println!("上下文条数   : {}", cfg.max_context_messages);
    println!(
        "是否需要 Key : {}",
        if cfg.requires_api_key() { "是" } else { "否（本地端点）" }
    );
    println!(
        "Key 已配置   : {}",
        if cfg.api_key.trim().is_empty() { "否" } else { "是" }
    );
    hr();
    println!("可用供应商（含默认端点，本地端点免 Key、省 token）：");
    for (name, url, model, local) in eazyqq_lib::services::ai::known_providers() {
        println!(
            "  {:<11} {:<34} {:<20} {}",
            name,
            url,
            model,
            if local { "本地" } else { "云端" }
        );
    }
    hr();
    println!("切换示例：");
    println!("  eazyqq_cli ai-set --provider opencode");
    println!("  eazyqq_cli ai-set --provider ollama --model qwen2.5:7b");
    println!("  eazyqq_cli ai-set --provider openai --base-url http://127.0.0.1:1234/v1 --model local-model");
    hr();
    Ok(())
}

/// Patch the `ai` block of `app_config`.
fn cmd_ai_set(svc: &Services, args: &Args) -> Result<(), String> {
    let raw = svc
        .db
        .get_setting("app_config")
        .ok()
        .flatten()
        .unwrap_or_else(|| "{}".to_string());

    let mut config: serde_json::Value =
        serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}));
    if !config.is_object() {
        config = serde_json::json!({});
    }

    let mut ai = config.get("ai").cloned().unwrap_or_else(|| serde_json::json!({}));
    if !ai.is_object() {
        ai = serde_json::json!({});
    }

    let mut changed: Vec<String> = Vec::new();
    let mut custom_provider = false;

    if let Some(v) = args.flag("provider") {
        let provider = v.trim().to_lowercase();
        ai["activeProvider"] = serde_json::json!(provider);

        // Refresh the endpoint and model from the preset unless the caller pinned them
        // in this same call. Without this, a previously stored explicit baseUrl keeps
        // winning and the provider switch silently does nothing.
        let (preset_url, preset_model) = eazyqq_lib::services::ai::provider_preset(&provider);
        match (preset_url, preset_model) {
            (Some(url), Some(model)) => {
                if args.flag("base-url").is_none() {
                    ai["baseUrl"] = serde_json::json!(url);
                }
                if args.flag("model").is_none() {
                    ai["model"] = serde_json::json!(model);
                }
            }
            _ => {
                // Custom provider: nothing to derive from, so whatever is stored stays.
                // Flag it so the user is told what still has to be filled in.
                custom_provider = true;
            }
        }
        changed.push(format!("provider={}", provider));
    }
    if let Some(v) = args.flag("base-url") {
        ai["baseUrl"] = serde_json::json!(v);
        changed.push(format!("baseUrl={}", v));
    }
    if let Some(v) = args.flag("model") {
        ai["model"] = serde_json::json!(v);
        changed.push(format!("model={}", v));
    }
    if let Some(v) = args.flag("key") {
        ai["apiKey"] = serde_json::json!(v);
        changed.push("apiKey=***".to_string());
    }
    if let Some(v) = args.flag("temperature") {
        let t: f64 = v
            .parse()
            .map_err(|_| format!("--temperature 需要数字，收到 {}", v))?;
        ai["temperature"] = serde_json::json!(t);
        changed.push(format!("temperature={}", t));
    }
    if let Some(v) = args.flag("max-context") {
        let n: i64 = v
            .parse()
            .map_err(|_| format!("--max-context 需要整数，收到 {}", v))?;
        ai["maxContextMessages"] = serde_json::json!(n);
        changed.push(format!("maxContextMessages={}", n));
    }

    if changed.is_empty() {
        return Err(
            "未指定任何变更项。可用: --provider --base-url --model --key --temperature --max-context"
                .to_string(),
        );
    }

    config["ai"] = ai;
    let serialized =
        serde_json::to_string(&config).map_err(|e| format!("序列化配置失败: {}", e))?;
    svc.db
        .set_setting("app_config", &serialized)
        .map_err(|e| format!("写入配置失败: {}", e))?;

    tracing::info!("AI config updated: {}", changed.join(", "));

    // Re-resolve so the user sees the effective endpoint (preset vs explicit).
    let fallback_key = svc
        .db
        .get_setting("tokenrhythm_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let effective = eazyqq_lib::services::ai::AiRuntimeConfig::from_app_config(
        config.get("ai"),
        &fallback_key,
    );

    if args.json() {
        print_json(&serde_json::json!({
            "changed": changed,
            "effective": {
                "provider": effective.provider,
                "baseUrl": effective.base_url,
                "model": effective.model,
                "requiresApiKey": effective.requires_api_key(),
            }
        }));
        return Ok(());
    }

    println!("已更新: {}", changed.join(", "));
    println!("实际生效: {}", effective.describe());
    if let Err(e) = effective.validate() {
        println!("⚠ 配置尚不完整: {}", e);
    }
    if custom_provider {
        println!("⚠ 自定义供应商没有内置默认端点，请用 --base-url 与 --model 指定。");
    }
    println!();
    println!("提示: 已运行的客户端需要重启，或在设置页保存一次配置以热生效。");
    Ok(())
}

/// Probe the usual local inference ports and report which ones really speak the
/// OpenAI-compatible protocol.
///
/// This exists because guessing is not acceptable here: a port that answers HTTP 200
/// with an HTML page (OpenCode's web UI, for instance) is *not* a usable endpoint, and
/// the user should not have to discover that by watching requests fail.
async fn cmd_ai_detect(svc: &Services, args: &Args) -> Result<(), String> {
    const CANDIDATES: &[(&str, &str)] = &[
        ("http://127.0.0.1:11434/v1", "Ollama"),
        ("http://127.0.0.1:1234/v1", "LM Studio"),
        ("http://127.0.0.1:8080/v1", "llama.cpp server"),
        ("http://127.0.0.1:8000/v1", "vLLM"),
        ("http://127.0.0.1:4096/v1", "OpenCode (预期不兼容)"),
    ];

    // Probe the endpoint that is actually configured first - that is the one that matters.
    let configured = svc.ai.current();
    let mut targets: Vec<(String, String)> = Vec::new();
    if !configured.base_url.trim().is_empty() {
        targets.push((
            configured.base_url.clone(),
            format!("当前配置 ({})", configured.provider),
        ));
    }
    for (url, label) in CANDIDATES {
        if !targets.iter().any(|(u, _)| u == url) {
            targets.push((url.to_string(), label.to_string()));
        }
    }

    // Two clients: loopback probes must bypass the proxy (otherwise a machine-level
    // http_proxy turns "nothing is listening" into a misleading 502 from the proxy).
    let client_direct = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;
    let client_proxied = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;

    let mut results: Vec<serde_json::Value> = Vec::new();

    if !args.json() {
        hr();
        println!("探测大模型端点是否可用（OpenAI 兼容）");
        hr();
    }

    for (url, label) in &targets {
        let probe = format!("{}/models", url.trim_end_matches('/'));
        let client = if eazyqq_lib::services::ai::is_local_endpoint(url) {
            &client_direct
        } else {
            &client_proxied
        };
        let verdict = match client.get(&probe).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let trimmed = body.trim_start();

                if status.as_u16() == 401 || status.as_u16() == 403 {
                    // The endpoint speaks the protocol but wants credentials - still a
                    // usable endpoint, just needs a key.
                    ("需鉴权", "是 OpenAI 兼容端点，但需要 API Key".to_string())
                } else if status.is_success() && trimmed.starts_with('{') {
                    match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(json) if json.get("data").is_some() => {
                            let count = json
                                .get("data")
                                .and_then(|d| d.as_array())
                                .map(|a| a.len())
                                .unwrap_or(0);
                            ("可用", format!("OpenAI 兼容，检测到 {} 个模型", count))
                        }
                        _ => ("不兼容", "返回了 JSON 但不是模型列表结构".to_string()),
                    }
                } else if status.is_success() {
                    (
                        "不兼容",
                        "返回的是 HTML 页面（很可能是 Web UI，不是推理接口）".to_string(),
                    )
                } else {
                    ("不可用", format!("HTTP {}", status))
                }
            }
            Err(e) => {
                // Distinguish "nothing there" from "there but not answering".
                let detail = if e.is_connect() {
                    "端口未监听或拒绝连接".to_string()
                } else if e.is_timeout() {
                    "端口有服务但无响应（超时）".to_string()
                } else {
                    format!("连接失败: {}", e)
                };
                ("未监听", detail)
            }
        };

        if args.json() {
            results.push(serde_json::json!({
                "label": label,
                "url": url,
                "verdict": verdict.0,
                "detail": verdict.1,
            }));
        } else {
            println!("[{}] {:<24} {:<32} {}", verdict.0, label, url, verdict.1);
        }
    }

    if args.json() {
        print_json(&serde_json::json!({ "results": results }));
    } else {
        hr();
        println!("说明: 标记为「可用」或「需鉴权」的端点才能填入 ai-set --base-url。");
        println!("本地推理推荐 Ollama / LM Studio / llama.cpp / vLLM；");
        println!("OpenCode 是编码智能体框架，其 serve 端口返回 Web UI，不提供 OpenAI 兼容接口。");
        hr();
    }
    Ok(())
}

/// Probe the configured provider with a real request.
async fn cmd_ai_test(svc: &Services, args: &Args) -> Result<(), String> {
    let cfg = svc.ai.current();
    println!("测试目标: {}", cfg.describe());

    let start = std::time::Instant::now();
    let result = svc.ai.generate_reply(&[], "ping", "连通性自检").await;
    let latency = start.elapsed().as_millis();

    match result {
        Ok((reply, _)) => {
            if args.json() {
                print_json(&serde_json::json!({
                    "isSuccess": true,
                    "provider": cfg.provider,
                    "model": cfg.model,
                    "endpoint": cfg.base_url,
                    "latencyMs": latency,
                    "reply": reply,
                }));
                return Ok(());
            }
            hr();
            println!("连通成功 ({} ms)", latency);
            println!("回复: {}", reply);
            hr();
            Ok(())
        }
        Err(e) => {
            if args.json() {
                print_json(&serde_json::json!({
                    "isSuccess": false,
                    "provider": cfg.provider,
                    "model": cfg.model,
                    "endpoint": cfg.base_url,
                    "error": e,
                }));
                return Ok(());
            }
            Err(format!("连通失败: {}", e))
        }
    }
}

/// Clear a conversation's unread badge.
fn cmd_mark_read(svc: &Services, args: &Args) -> Result<(), String> {
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

/// Report every hop of the chain and pinpoint the first break.
///
/// This is the "where is it broken" command: it walks the pipeline in order
/// (NapCat WebUI -> QQ login -> OneBot HTTP -> WebSocket -> database -> AI -> scheduler ->
/// frontend) and stops being interesting at the first failure, because everything after
/// it is a consequence rather than a separate fault.
async fn cmd_chain_status(svc: &Services, args: &Args) -> Result<(), String> {
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
///
/// Only links this process can honestly verify are touched; component-owned ones (the
/// GUI's WebSocket, its scheduler, its frontend) are left alone rather than guessed at.
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
            record_error(Link::NapcatWebUi, e);
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

    // The WebSocket and the scheduler belong to the GUI process. From here we can at least
    // say whether their port is open, and be explicit that this is not proof of health.
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

fn cmd_log_path() -> Result<(), String> {
    println!("{}", logging::active_log_path().display());
    let (dir, crash_count, last) = logging::diagnostics();
    println!("日志目录: {}", dir.display());
    println!("崩溃报告: {} 份", crash_count);
    if let Some(p) = last {
        println!("最近崩溃: {}", p.display());
    }
    Ok(())
}

fn cmd_log_tail(args: &Args) -> Result<(), String> {
    let lines: usize = args.flag("lines").and_then(|v| v.parse().ok()).unwrap_or(60);
    match logging::tail(lines) {
        Ok(text) => println!("{}", text),
        Err(e) => return Err(format!("读取日志失败: {}", e)),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main(flavor = "multi_thread")]
async fn main() -> ExitCode {
    let args = Args::parse();

    if args.command.is_empty() || args.command == "help" || args.has("help") {
        cmd_help();
        return ExitCode::SUCCESS;
    }

    if args.command == "version" {
        println!("eazyqq-cli {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    logging::init(true);

    let result = match args.command.as_str() {
        "log-path" => cmd_log_path(),
        "log-tail" => cmd_log_tail(&args),
        _ => {
            let svc = match Services::build() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("初始化失败: {}", e);
                    return ExitCode::FAILURE;
                }
            };
            match args.command.as_str() {
                "status" => cmd_status(&svc, &args).await,
                "login-info" => cmd_login_info(&svc, &args).await,
                "quick-login-list" => cmd_quick_login_list(&svc, &args).await,
                "quick-login" => cmd_quick_login(&svc, &args).await,
                "qr" => cmd_qr(&svc, &args).await,
                "contacts" => cmd_contacts(&svc, &args).await,
                "groups" => cmd_raw_roster(&svc, &args, true).await,
                "friends" => cmd_raw_roster(&svc, &args, false).await,
                "rule" => cmd_rule(&svc, &args).await,
                "send" => cmd_send(&svc, &args).await,
                "ask" => cmd_ask(&svc, &args).await,
                "drafts" => cmd_drafts(&svc, &args).await,
                "draft-send" => cmd_draft_send(&svc, &args).await,
                "draft-dismiss" => cmd_draft_dismiss(&svc, &args),
                "draft-regenerate" => cmd_draft_regenerate(&svc, &args).await,
                "history" => cmd_history(&svc, &args).await,
                "mark-read" => cmd_mark_read(&svc, &args),
                "files" => cmd_files(&svc, &args).await,
                "file-download" => cmd_file_download(&svc, &args).await,
                "file-summarize" => cmd_file_summarize(&svc, &args).await,
                "summaries" => cmd_summaries(&svc, &args).await,
                "summarize" => cmd_summarize(&svc, &args).await,
                "scheduler-tick" => cmd_scheduler_tick(&svc, &args).await,
                "whitelist-groups" => cmd_whitelist_groups(&svc, &args).await,
                "config" => cmd_config(&svc, &args).await,
                "set-config" => cmd_set_config(&svc, &args).await,
                "health" => cmd_health(&svc, &args).await,
                "chain-status" => cmd_chain_status(&svc, &args).await,
                "ai-config" => cmd_ai_config(&svc, &args),
                "ai-set" => cmd_ai_set(&svc, &args),
                "ai-test" => cmd_ai_test(&svc, &args).await,
                "ai-detect" => cmd_ai_detect(&svc, &args).await,
                "export" => cmd_export(&svc, &args).await,
                "simulate" => cmd_simulate(&svc, &args).await,
                other => {
                    eprintln!("未知命令: {}\n", other);
                    cmd_help();
                    return ExitCode::FAILURE;
                }
            }
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("错误: {}", e);
            ExitCode::FAILURE
        }
    }
}
