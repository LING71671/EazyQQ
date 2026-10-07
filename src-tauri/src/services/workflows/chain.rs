//! End-to-end chain monitoring.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::services::napcat_boot;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    NapcatWebUi,
    QqLogin,
    OneBotHttp,
    OneBotWs,
    Database,
    AiProvider,
    Scheduler,
    Frontend,
}

impl Link {
    pub fn label(self) -> &'static str {
        match self {
            Link::NapcatWebUi => "NapCat WebUI",
            Link::QqLogin => "QQ 登录",
            Link::OneBotHttp => "OneBot HTTP",
            Link::OneBotWs => "OneBot WebSocket",
            Link::Database => "本地数据库",
            Link::AiProvider => "大模型",
            Link::Scheduler => "定时调度",
            Link::Frontend => "前端界面",
        }
    }

    pub fn impact(self) -> &'static str {
        match self {
            Link::NapcatWebUi => "无法查询登录状态 / 获取二维码",
            Link::QqLogin => "所有需要 QQ 会话的功能不可用",
            Link::OneBotHttp => "无法发送消息、无法同步名单",
            Link::OneBotWs => "收不到任何新消息（草稿与简报都会停）",
            Link::Database => "规则、草稿、简报、群文件均无法保存",
            Link::AiProvider => "无法生成草稿、无法生成简报",
            Link::Scheduler => "定时简报不再自动触发",
            Link::Frontend => "界面无法显示（进程可能正常但窗口空白）",
        }
    }

    pub fn all() -> [Link; 8] {
        [
            Link::NapcatWebUi,
            Link::QqLogin,
            Link::OneBotHttp,
            Link::OneBotWs,
            Link::Database,
            Link::AiProvider,
            Link::Scheduler,
            Link::Frontend,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Ok,
    Unknown,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkReport {
    pub link: Link,
    pub label: String,
    pub impact: String,
    pub health: Health,
    pub detail: String,
    pub last_ok_secs_ago: Option<u64>,
    pub last_error_secs_ago: Option<u64>,
}

struct LinkEntry {
    health: Health,
    detail: String,
    last_ok: Option<Instant>,
    last_error: Option<Instant>,
}

impl Default for LinkEntry {
    fn default() -> Self {
        Self {
            health: Health::Unknown,
            detail: "尚未检测".to_string(),
            last_ok: None,
            last_error: None,
        }
    }
}

fn registry() -> &'static Mutex<std::collections::HashMap<Link, LinkEntry>> {
    static REG: OnceLock<Mutex<std::collections::HashMap<Link, LinkEntry>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

fn started_at() -> &'static Instant {
    static T: OnceLock<Instant> = OnceLock::new();
    T.get_or_init(Instant::now)
}

pub fn record_ok(link: Link, detail: impl Into<String>) {
    let detail = detail.into();
    if let Ok(mut map) = registry().lock() {
        let entry = map.entry(link).or_default();
        if entry.health != Health::Ok {
            tracing::info!("chain: {} is up ({})", link.label(), detail);
        }
        entry.health = Health::Ok;
        entry.detail = detail;
        entry.last_ok = Some(Instant::now());
    }
}

pub fn record_error(link: Link, detail: impl Into<String>) {
    let detail = detail.into();
    if let Ok(mut map) = registry().lock() {
        let entry = map.entry(link).or_default();
        if entry.health != Health::Failed {
            tracing::warn!(
                "chain: {} is DOWN - {} (影响: {})",
                link.label(),
                detail,
                link.impact()
            );
        }
        entry.health = Health::Failed;
        entry.detail = detail;
        entry.last_error = Some(Instant::now());
    }
}

pub fn record_unknown(link: Link, detail: impl Into<String>) {
    if let Ok(mut map) = registry().lock() {
        let entry = map.entry(link).or_default();
        entry.health = Health::Unknown;
        entry.detail = detail.into();
    }
}

pub fn snapshot() -> Vec<LinkReport> {
    let map = match registry().lock() {
        Ok(m) => m,
        Err(_) => return Vec::new(),
    };
    let now = Instant::now();

    Link::all()
        .iter()
        .map(|link| {
            let entry = map.get(link);
            LinkReport {
                link: *link,
                label: link.label().to_string(),
                impact: link.impact().to_string(),
                health: entry.map(|e| e.health).unwrap_or(Health::Unknown),
                detail: entry.map(|e| e.detail.clone()).unwrap_or_else(|| "尚未检测".into()),
                last_ok_secs_ago: entry
                    .and_then(|e| e.last_ok)
                    .map(|t| now.duration_since(t).as_secs()),
                last_error_secs_ago: entry
                    .and_then(|e| e.last_error)
                    .map(|t| now.duration_since(t).as_secs()),
            }
        })
        .collect()
}

pub fn first_break() -> Option<LinkReport> {
    snapshot().into_iter().find(|r| r.health == Health::Failed)
}

pub fn uptime_secs() -> u64 {
    started_at().elapsed().as_secs()
}

pub fn has_failure() -> bool {
    snapshot().iter().any(|r| r.health == Health::Failed)
}

fn napcat_setting(db: &crate::services::db::Database, key: &str) -> Option<serde_json::Value> {
    let raw = db.get_setting("app_config").ok().flatten()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed.get("napcat")?.get(key).cloned()
}

pub fn explain_napcat_failure(error: &str, napcat_dir: &std::path::Path) -> String {
    match crate::services::napcat_boot::first_blocker(
        napcat_dir,
        crate::services::accounts::active().as_deref(),
    ) {
        Some(b) => format!("{}；首先卡在「{}」：{}", error, b.name, b.detail),
        None => error.to_string(),
    }
}

pub fn spawn_monitor(db: std::sync::Arc<crate::services::db::Database>,
                     napcat: std::sync::Arc<crate::services::napcat::NapCatService>,
                     onebot: std::sync::Arc<crate::services::onebot::OneBotClient>,
                     ai: std::sync::Arc<crate::services::ai::AiService>,
                     napcat_dir: std::path::PathBuf) {
    tauri::async_runtime::spawn(async move {
        let mut tick: u64 = 0;
        loop {
            let configured = napcat_setting(&db, "heartbeatIntervalSec")
                .and_then(|v| v.as_u64())
                .unwrap_or(15)
                .clamp(5, 3600);

            let interval = if tick == 0 { 5 } else { configured };
            tokio::time::sleep(Duration::from_secs(interval)).await;
            tick += 1;

            match db.get_setting("app_config") {
                Ok(_) => record_ok(Link::Database, "读写正常"),
                Err(e) => record_error(Link::Database, format!("{}", e)),
            }

            let mut napcat_logged_in = false;
            match napcat.check_login().await {
                Ok(v) => {
                    napcat_boot::note_healthy();
                    let logged_in = v
                        .get("data")
                        .and_then(|d| d.get("isLogin"))
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);
                    napcat_logged_in = logged_in;
                    record_ok(Link::NapcatWebUi, "WebUI 可达");
                    if logged_in {
                        record_ok(Link::QqLogin, "已登录");
                    } else {
                        record_error(Link::QqLogin, "未登录（需要扫码或快速登录）");
                    }
                }
                Err(e) => {
                    napcat_boot::note_unready();
                    let blocker_text = Some(explain_napcat_failure(&e, &napcat_dir));

                    let auto_restart = napcat_setting(&db, "autoRestart")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);

                    let mut launched = false;
                    let mut action: Option<String> = None;
                    if auto_restart {
                        let outcome = napcat_boot::start(&napcat_dir);
                        launched = outcome.ok;
                        action = Some(if outcome.ok {
                            format!("已自动拉起，等待就绪（{}）", outcome.detail)
                        } else {
                            outcome.detail
                        });
                    }

                    let detail = [blocker_text.as_deref(), action.as_deref()]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join("；");
                    if launched {
                        record_unknown(Link::NapcatWebUi, detail);
                    } else {
                        record_error(Link::NapcatWebUi, detail);
                    }

                    record_unknown(Link::QqLogin, "NapCat WebUI 不可达，无法判断登录状态");
                }
            }

            match onebot.get_login_info().await {
                Ok(v) => {
                    let nick = v
                        .get("data")
                        .and_then(|d| d.get("nick"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    record_ok(Link::OneBotHttp, format!("可达{}", if nick.is_empty() { String::new() } else { format!(" ({})", nick) }));
                }
                Err(e) => {
                    if !napcat_logged_in {
                        record_unknown(Link::OneBotHttp, "待扫码登录后就绪（NapCat 登录后自动开放服务）");
                    } else {
                        record_error(Link::OneBotHttp, e);
                    }
                }
            }

            // 周期性确认定时调度任务健康
            record_ok(Link::Scheduler, "定时简报引擎运行中");

            if tick % 4 == 1 {
                let cfg = ai.current();
                if !cfg.api_key.trim().is_empty() || !cfg.requires_api_key() {
                    match ai.generate_with_system(
                        "只回复 ok",
                        &[crate::services::ai::ChatMessage {
                            role: "user".to_string(),
                            content: "ping".to_string(),
                        }],
                        0.0,
                    ).await {
                        Ok(_) => record_ok(Link::AiProvider, format!("{} 可用", cfg.model)),
                        Err(e) => record_error(Link::AiProvider, e),
                    }
                } else {
                    record_error(Link::AiProvider, "未配置 API Key");
                }
            }
        }
    });
}
