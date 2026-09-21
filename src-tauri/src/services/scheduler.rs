//! Scheduled group summarizer (ROADMAP Phase 5).
//!
//! Fully configuration-driven - there is no hardcoded period anywhere:
//!
//! * which groups are summarised  -> `contact_rules.is_summary_whitelist`
//! * how often each group runs    -> `contact_rules.summary_interval_hours`
//! * the sliding window           -> `app_config.summary.slidingWindowHours`
//! * the extraction prompt        -> `app_config.summary.customPrompt`
//! * on/off                       -> `app_config.summary.enabled`
//!
//! The scheduler ticks once a minute and asks, per whitelisted group, whether
//! `now - last_summary_time >= interval`. Nothing to persist: the last run time is the
//! `created_at` of the newest stored summary.

use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::services::ai::AiService;
use crate::services::db::Database;
use crate::services::summarizer::{self, SummaryRequest, DEFAULT_PROMPT};

const TICK_SECONDS: u64 = 60;

#[derive(Debug, Clone)]
pub struct SummarySettings {
    pub enabled: bool,
    pub sliding_window_hours: i32,
    pub custom_prompt: String,
}

impl Default for SummarySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sliding_window_hours: 6,
            custom_prompt: DEFAULT_PROMPT.to_string(),
        }
    }
}

/// Read the summarizer configuration out of `app_settings.app_config`.
pub fn read_settings(db: &Database) -> SummarySettings {
    let raw = match db.get_setting("app_config") {
        Ok(Some(v)) => v,
        _ => return SummarySettings::default(),
    };
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("scheduler: app_config is not valid JSON ({}), using defaults", e);
            return SummarySettings::default();
        }
    };
    let summary = match parsed.get("summary") {
        Some(s) => s,
        None => return SummarySettings::default(),
    };

    SummarySettings {
        enabled: summary
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
        sliding_window_hours: summary
            .get("slidingWindowHours")
            .and_then(|v| v.as_i64())
            .unwrap_or(6)
            .clamp(1, 720) as i32,
        custom_prompt: summary
            .get("customPrompt")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(DEFAULT_PROMPT)
            .to_string(),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// One scheduler pass. Returns the targets that were summarised.
pub async fn tick(
    db: &Arc<Database>,
    ai: &Arc<AiService>,
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
        let interval_hours = group.summary_interval_hours.max(1) as i64;
        let interval_ms = interval_hours * 3600 * 1000;

        // Last run = created_at of the newest summary for this target.
        let last_run = db
            .get_summaries(Some(&group.target_id))
            .unwrap_or_default()
            .into_iter()
            .map(|s| s.created_at)
            .max();

        let due = match last_run {
            Some(last) => now - last >= interval_ms,
            // Never summarised before -> run on the first tick.
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
                outcomes.push(outcome);
            }
            Err(e) => {
                tracing::error!("scheduler: summary for {} failed: {}", group.name, e);
            }
        }
    }

    outcomes
}

/// Spawn the background loop. Safe to call once during app setup.
pub fn start_summary_scheduler(
    app_handle: AppHandle,
    db: Arc<Database>,
    ai: Arc<AiService>,
) {
    tauri::async_runtime::spawn(async move {
        tracing::info!("summary scheduler started (tick every {}s)", TICK_SECONDS);
        loop {
            let outcomes = tick(&db, &ai, Some(&app_handle)).await;
            if !outcomes.is_empty() {
                tracing::info!("scheduler: produced {} summary/summaries", outcomes.len());
            }
            tokio::time::sleep(Duration::from_secs(TICK_SECONDS)).await;
        }
    });
}
