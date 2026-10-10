//! End-to-end chain monitoring.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

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
                detail: entry
                    .map(|e| e.detail.clone())
                    .unwrap_or_else(|| "尚未检测".into()),
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

pub fn spawn_monitor(
    db: std::sync::Arc<crate::services::db::Database>,
    napcat: std::sync::Arc<crate::services::napcat::NapCatService>,
    onebot: std::sync::Arc<crate::services::onebot::OneBotClient>,
    ai: std::sync::Arc<crate::services::ai::AiService>,
    _napcat_dir: std::path::PathBuf,
) {
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
                Ok(_) => record_ok(Link::Database, "数据库读取正常"),
                Err(e) => record_error(Link::Database, format!("{}", e)),
            }

            crate::services::infra::runtime_config::refresh_ai(&db, &ai);
            refresh_ai_status(&ai);
            crate::services::protocol::health::refresh(&napcat, &onebot).await;
            let account_enabled = crate::services::accounts::active()
                .and_then(|uin| {
                    crate::services::instances::load_registry()
                        .instances
                        .into_iter()
                        .find(|i| i.uin == uin)
                })
                .map(|i| i.auto_start)
                .unwrap_or(true);
            if account_enabled
                && crate::services::protocol::boot::auto_start_allowed()
                && !napcat.is_alive().await
                && !crate::services::protocol::session::probe(&napcat, &onebot)
                    .await
                    .logged_in
                && !crate::services::protocol::ownership::is_running(std::path::Path::new(
                    napcat.napcat_dir(),
                ))
                && napcat_setting(&db, "autoRestart")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true)
            {
                let dir = std::path::PathBuf::from(napcat.napcat_dir());
                let account = crate::services::accounts::active();
                let _ = tokio::task::spawn_blocking(move || {
                    crate::services::protocol::boot::start_with_args(&dir, None, account.as_deref())
                })
                .await;
            }


        }
    });
}


pub fn refresh_ai_status(ai: &crate::services::ai::AiService) {
    let evidence = ai.inference_health();
    let health = match evidence.verified { Some(true) => Health::Ok, Some(false) => Health::Failed, None => Health::Unknown };
    if snapshot().iter().any(|entry| entry.link == Link::AiProvider && entry.health == health && entry.detail == evidence.detail) { return; }
    match health {
        Health::Ok => record_ok(Link::AiProvider, evidence.detail),
        Health::Failed => record_error(Link::AiProvider, evidence.detail),
        Health::Unknown => record_unknown(Link::AiProvider, evidence.detail),
    }
}
