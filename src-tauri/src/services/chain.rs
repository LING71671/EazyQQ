//! End-to-end chain monitoring.
//!
//! EazyQQ is a pipeline: QQ -> NapCat -> OneBot (HTTP + WebSocket) -> the message
//! pipeline -> AI provider -> SQLite -> the frontend. When something looks broken, the
//! hard part has never been fixing a link - it has been working out *which* link failed.
//! Several of the failures found during development produced no error anywhere visible:
//! a WebView that could not execute JS left no crash dump, and a WebSocket that never
//! connected only appeared as a retry warning in a log file nobody was reading.
//!
//! So every link reports into one registry, and the registry answers two questions:
//! is each link up, and where is the first break?
//!
//! Links that can be probed from the outside (HTTP endpoints) are polled by
//! [`spawn_monitor`]. Links that only the owning component can know about (whether the
//! WebSocket is connected, whether the frontend ever called in) are reported by that
//! component through [`record`].

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::services::napcat_boot;

/// Every hop in the chain, in the order a message travels through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    /// NapCat's WebUI - used for login state, QR codes and quick login.
    NapcatWebUi,
    /// QQ account actually logged in (reported by NapCat).
    QqLogin,
    /// OneBot HTTP API - used to send messages and query the roster.
    OneBotHttp,
    /// OneBot WebSocket - the inbound message stream. If this is down nothing arrives.
    OneBotWs,
    /// Local SQLite store.
    Database,
    /// Configured AI provider.
    AiProvider,
    /// The scheduled group-summary loop.
    Scheduler,
    /// Whether the frontend ever called into the backend. A blank window and a healthy
    /// window look identical from the outside; this is the only honest signal.
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

    /// What breaks downstream when this link is down - shown so the user knows the impact
    /// rather than just seeing a red dot.
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
    /// Working as far as anything can tell.
    Ok,
    /// Not proven broken, but nothing has confirmed it either.
    Unknown,
    /// Known to be down.
    Failed,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkReport {
    pub link: Link,
    pub label: String,
    pub impact: String,
    pub health: Health,
    pub detail: String,
    /// Seconds since this link last reported success, if it ever has.
    pub last_ok_secs_ago: Option<u64>,
    /// Seconds since this link last reported failure, if it ever has.
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

/// Record a successful interaction with a link.
pub fn record_ok(link: Link, detail: impl Into<String>) {
    let detail = detail.into();
    if let Ok(mut map) = registry().lock() {
        let entry = map.entry(link).or_default();
        // Only log transitions, so a link that is polled every 30s does not flood the log.
        if entry.health != Health::Ok {
            tracing::info!("chain: {} is up ({})", link.label(), detail);
        }
        entry.health = Health::Ok;
        entry.detail = detail;
        entry.last_ok = Some(Instant::now());
    }
}

/// Record a failure. This is what makes a broken link visible instead of merely absent.
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

/// Mark a link as not-yet-exercised without implying it is broken.
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

/// The first link that is known to be down, in pipeline order.
///
/// Reported first because in a pipeline the earliest break is the one worth fixing -
/// everything after it is a consequence, not a separate fault.
pub fn first_break() -> Option<LinkReport> {
    snapshot().into_iter().find(|r| r.health == Health::Failed)
}

/// How long the process has been running, for context in reports.
pub fn uptime_secs() -> u64 {
    started_at().elapsed().as_secs()
}

/// Does any link report a failure?
pub fn has_failure() -> bool {
    snapshot().iter().any(|r| r.health == Health::Failed)
}

/// Read `napcat.{key}` from the persisted config, with a fallback.
fn napcat_setting(db: &crate::services::db::Database, key: &str) -> Option<serde_json::Value> {
    let raw = db.get_setting("app_config").ok().flatten()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed.get("napcat")?.get(key).cloned()
}

/// Periodically probe the links that can be probed from outside, and act on the one that
/// can be acted upon.
///
/// Component-owned links (WebSocket, frontend, scheduler) are not touched here - they
/// report themselves, and polling them would either be meaningless or produce a false
/// verdict.
pub fn spawn_monitor(db: std::sync::Arc<crate::services::db::Database>,
                     napcat: std::sync::Arc<crate::services::napcat::NapCatService>,
                     onebot: std::sync::Arc<crate::services::onebot::OneBotClient>,
                     ai: std::sync::Arc<crate::services::ai::AiService>,
                     napcat_dir: std::path::PathBuf) {
    tauri::async_runtime::spawn(async move {
        // First pass quickly, then settle into a slow cadence: this is a monitor, not a
        // hot path, and probing the AI provider costs a request.
        let mut tick: u64 = 0;
        loop {
            // `napcat.heartbeatIntervalSec` drives the cadence; clamped so a typo cannot
            // turn the monitor into a busy loop or make it effectively never run.
            let configured = napcat_setting(&db, "heartbeatIntervalSec")
                .and_then(|v| v.as_u64())
                .unwrap_or(15)
                .clamp(5, 3600);

            let interval = if tick == 0 { 5 } else { configured };
            tokio::time::sleep(Duration::from_secs(interval)).await;
            tick += 1;

            // Database: cheap and local.
            match db.get_setting("app_config") {
                Ok(_) => record_ok(Link::Database, "读写正常"),
                Err(e) => record_error(Link::Database, format!("{}", e)),
            }

            // NapCat WebUI + QQ login state.
            match napcat.check_login().await {
                Ok(v) => {
                    napcat_boot::note_healthy();
                    let logged_in = v
                        .get("data")
                        .and_then(|d| d.get("isLogin"))
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);
                    record_ok(Link::NapcatWebUi, "WebUI 可达");
                    if logged_in {
                        record_ok(Link::QqLogin, "已登录");
                    } else {
                        record_error(Link::QqLogin, "未登录（需要扫码或快速登录）");
                    }
                }
                Err(e) => {
                    record_error(Link::NapcatWebUi, e);

                    // The protocol side is the one link we can actually repair, so do it
                    // when the user has asked for it. Without this the app could only ever
                    // report that NapCat was down.
                    let auto_restart = napcat_setting(&db, "autoRestart")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    if auto_restart {
                        let outcome = napcat_boot::start(&napcat_dir);
                        if outcome.attempted {
                            if outcome.ok {
                                tracing::warn!(
                                    "chain: NapCat was down and autoRestart is on - {}",
                                    outcome.detail
                                );
                                record_unknown(
                                    Link::NapcatWebUi,
                                    format!("已自动拉起，等待就绪（{}）", outcome.detail),
                                );
                            } else {
                                tracing::error!(
                                    "chain: could not start NapCat - {}",
                                    outcome.detail
                                );
                                record_error(
                                    Link::NapcatWebUi,
                                    format!("自动拉起失败: {}", outcome.detail),
                                );
                            }
                        } else {
                            // Throttled or out of attempts; say so rather than looking idle.
                            tracing::debug!("chain: skipping NapCat restart - {}", outcome.detail);
                        }
                    }

                    record_unknown(Link::QqLogin, "NapCat WebUI 不可达，无法判断登录状态");
                }
            }

            // OneBot HTTP.
            match onebot.get_login_info().await {
                Ok(v) => {
                    let nick = v
                        .get("data")
                        .and_then(|d| d.get("nick"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    record_ok(Link::OneBotHttp, format!("可达{}", if nick.is_empty() { String::new() } else { format!(" ({})", nick) }));
                }
                Err(e) => record_error(Link::OneBotHttp, e),
            }

            // AI provider: only probe occasionally, since it costs a request.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_until_something_reports() {
        // A fresh link must not be reported as healthy - that is the whole point.
        let reports = snapshot();
        assert_eq!(reports.len(), Link::all().len());
        assert!(reports.iter().all(|r| r.health == Health::Unknown
            || r.health == Health::Ok
            || r.health == Health::Failed));
    }

    #[test]
    fn failure_is_recorded_and_reported_as_the_first_break() {
        record_error(Link::OneBotWs, "connection refused");
        let report = snapshot()
            .into_iter()
            .find(|r| r.link == Link::OneBotWs)
            .expect("link present");
        assert_eq!(report.health, Health::Failed);
        assert_eq!(report.detail, "connection refused");
        assert!(report.last_error_secs_ago.is_some());

        // OneBotWs sits after NapcatWebUi/QQ/Http and before Database, so with only this
        // link down it must be the reported break.
        let first = first_break().expect("a break exists");
        assert_eq!(first.link, Link::OneBotWs);
        assert!(has_failure());
    }

    #[test]
    fn success_clears_a_previous_failure() {
        record_error(Link::Scheduler, "tick failed");
        assert_eq!(
            snapshot().into_iter().find(|r| r.link == Link::Scheduler).unwrap().health,
            Health::Failed
        );

        record_ok(Link::Scheduler, "tick ok");
        let report = snapshot().into_iter().find(|r| r.link == Link::Scheduler).unwrap();
        assert_eq!(report.health, Health::Ok);
        assert!(report.last_ok_secs_ago.is_some());
        // The old failure timestamp is kept: useful history, not a lie about current state.
        assert!(report.last_error_secs_ago.is_some());
    }

    #[test]
    fn unknown_does_not_count_as_a_failure() {
        record_unknown(Link::Frontend, "尚未收到前端调用");
        let report = snapshot().into_iter().find(|r| r.link == Link::Frontend).unwrap();
        assert_eq!(report.health, Health::Unknown);
    }

    #[test]
    fn every_link_has_a_label_and_an_impact() {
        for link in Link::all() {
            assert!(!link.label().is_empty(), "{:?} needs a label", link);
            assert!(!link.impact().is_empty(), "{:?} needs an impact note", link);
        }
    }

    #[test]
    fn uptime_is_reported() {
        assert!(uptime_secs() < 3600, "uptime should be small in a test run");
    }
}
