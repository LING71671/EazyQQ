//! Scheduled group summarizer (ROADMAP Phase 5).

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use crate::services::ai::AiService;
use crate::services::db::Database;
use crate::services::onebot::OneBotClient;
use crate::services::workflows::summarizer::{self, SummaryRequest};

pub use super::scheduler_settings::*;

const TICK_SECONDS: u64 = 60;
const LEGACY_DEFAULT_INTERVAL_HOURS: i32 = 6;

pub fn normalize_legacy_intervals(db: &Database) -> usize {
    let rules = match db.get_all_rules() {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("scheduler: cannot read rules for interval normalisation: {}", e);
            return 0;
        }
    };

    let mut changed = 0usize;
    for mut rule in rules {
        if rule.summary_interval_hours != LEGACY_DEFAULT_INTERVAL_HOURS {
            continue;
        }
        rule.summary_interval_hours = 0;
        match db.upsert_rule(&rule) {
            Ok(_) => changed += 1,
            Err(e) => tracing::warn!(
                "scheduler: could not normalise interval for {}: {}",
                rule.target_id,
                e
            ),
        }
    }

    if changed > 0 {
        tracing::info!(
            "scheduler: {} rule(s) now follow the global cadence instead of the old \
             hardcoded {}h default",
            changed,
            LEGACY_DEFAULT_INTERVAL_HOURS
        );
    }
    changed
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn forward_to_phone_enabled(db: &Database) -> bool {
    let raw = match db.get_setting("app_config") {
        Ok(Some(v)) => v,
        _ => return false,
    };
    serde_json::from_str::<serde_json::Value>(&raw)
        .ok()
        .and_then(|v| {
            v.get("summary")?
                .get("autoForwardToPhone")?
                .as_bool()
        })
        .unwrap_or(false)
}

async fn forward_summary_to_self(onebot: &Arc<OneBotClient>, outcome: &summarizer::SummaryOutcome) {
    let self_id = match onebot.get_login_info().await {
        Ok(v) => {
            let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);
            data.get("user_id")
                .or_else(|| data.get("uin"))
                .map(|u| match u {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default()
        }
        Err(e) => {
            tracing::warn!("forward: cannot resolve own account, skipping forward: {}", e);
            return;
        }
    };

    if self_id.is_empty() {
        tracing::warn!("forward: own account id unavailable, skipping forward");
        return;
    }

    let s = &outcome.summary;
    let window_hours = ((s.end_time - s.start_time).max(0) as f64 / 3_600_000.0).round() as i64;
    let mut text = format!(
        "【{} 简报 · 近 {} 小时】\n\n{}",
        s.target_name,
        window_hours.max(1),
        s.summary_text
    );
    if !s.key_points.is_empty() {
        text.push_str("\n\n核心要点：");
        for p in &s.key_points {
            text.push_str(&format!("\n· {}", p));
        }
    }
    if !s.decisions.is_empty() {
        text.push_str("\n\n决议与待办：");
        for d in &s.decisions {
            text.push_str(&format!("\n· {}", d));
        }
    }

    match onebot.send_msg("private", &self_id, &text).await {
        Ok(_) => tracing::info!(
            "forward: summary for {} sent to own account ({})",
            s.target_name,
            self_id
        ),
        Err(e) => tracing::warn!("forward: sending summary to self failed: {}", e),
    }
}

pub async fn tick(
    db: &Arc<Database>,
    ai: &Arc<AiService>,
    onebot: &Arc<OneBotClient>,
    app_handle: Option<&AppHandle>,
) -> Vec<summarizer::SummaryOutcome> {
    let settings = read_settings(db);
    if !settings.enabled {
        tracing::debug!("scheduler: summarization disabled in config, skipping tick");
        return Vec::new();
    }

    let groups = match db.get_summary_whitelist_groups() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("scheduler: cannot read summary whitelist: {}", e);
            return Vec::new();
        }
    };

    if groups.is_empty() {
        tracing::debug!("scheduler: no groups on the summary whitelist");
        return Vec::new();
    }

    let now = now_ms();
    let mut outcomes = Vec::new();

    for group in groups {
        let interval_hours = if group.summary_interval_hours > 0 {
            group.summary_interval_hours as i64
        } else {
            settings.interval_hours
        }
        .max(1);
        let interval_ms = interval_hours * 3600 * 1000;

        let last_run = db
            .get_summaries(Some(&group.target_id))
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.created_at)
            .max();

        let due = match last_run {
            Some(last) => now - last >= interval_ms,
            None => true,
        };

        if !due {
            tracing::debug!(
                "scheduler: {} not due yet (interval {}h, last run {:?})",
                group.name,
                interval_hours,
                last_run
            );
            continue;
        }

        tracing::info!(
            "scheduler: generating summary for {} (interval {}h, window {}h)",
            group.name,
            interval_hours,
            settings.sliding_window_hours
        );

        let req = SummaryRequest {
            target_id: group.target_id.clone(),
            sliding_window_hours: settings.sliding_window_hours,
            custom_prompt: Some(settings.custom_prompt.clone()),
            max_messages: 400,
            min_messages: 3,
        };

        match summarizer::generate(db, ai, &req).await {
            Ok(outcome) => {
                if let Some(handle) = app_handle {
                    let _ = handle.emit("new-summary", &outcome.summary);
                }
                if forward_to_phone_enabled(db) {
                    forward_summary_to_self(onebot, &outcome).await;
                }
                outcomes.push(outcome);
            }
            Err(e) => {
                tracing::error!("scheduler: summary for {} failed: {}", group.name, e);
            }
        }
    }

    outcomes
}

pub fn start_summary_scheduler(
    app_handle: AppHandle,
    db: Arc<Database>,
    ai: Arc<AiService>,
    onebot: Arc<OneBotClient>,
) {
    tauri::async_runtime::spawn(async move {
        tracing::info!("summary scheduler started (tick every {}s)", TICK_SECONDS);
        loop {
            let outcomes = tick(&db, &ai, &onebot, Some(&app_handle)).await;
            if !outcomes.is_empty() {
                tracing::info!("scheduler: produced {} summary/summaries", outcomes.len());
            }
            crate::services::chain::record_ok(
                crate::services::chain::Link::Scheduler,
                format!("运行中，最近一轮产出 {} 份简报", outcomes.len()),
            );
            tokio::time::sleep(Duration::from_secs(TICK_SECONDS)).await;
        }
    });
}
