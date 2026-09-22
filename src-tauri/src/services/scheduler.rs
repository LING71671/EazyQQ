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
            // Report liveness to the chain monitor: a scheduler that silently stopped is
            // otherwise indistinguishable from one with nothing to do.
            crate::services::chain::record_ok(
                crate::services::chain::Link::Scheduler,
                format!("运行中，最近一轮产出 {} 份简报", outcomes.len()),
            );
            tokio::time::sleep(Duration::from_secs(TICK_SECONDS)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> std::sync::Arc<Database> {
        let dir = std::env::temp_dir().join("eazyqq_scheduler_tests");
        crate::services::logging::ensure_dir(&dir);
        let path = dir.join(format!("{}_{}.db", tag, std::process::id()));
        let _ = std::fs::remove_file(&path);
        std::sync::Arc::new(Database::init(&path).expect("temp db"))
    }

    #[test]
    fn settings_fall_back_to_defaults_without_config() {
        let db = temp_db("no_config");
        let s = read_settings(&db);
        assert!(s.enabled);
        assert_eq!(s.sliding_window_hours, 6);
        assert_eq!(s.custom_prompt, DEFAULT_PROMPT);
    }

    #[test]
    fn settings_are_read_from_app_config() {
        let db = temp_db("with_config");
        db.set_setting(
            "app_config",
            r#"{"summary":{"enabled":false,"slidingWindowHours":90,"customPrompt":"只要待办"}}"#,
        )
        .unwrap();

        let s = read_settings(&db);
        assert!(!s.enabled);
        assert_eq!(s.sliding_window_hours, 90);
        assert_eq!(s.custom_prompt, "只要待办");
    }

    #[test]
    fn sliding_window_is_clamped_to_a_sane_range() {
        let db = temp_db("clamp");
        db.set_setting("app_config", r#"{"summary":{"slidingWindowHours":99999}}"#)
            .unwrap();
        assert_eq!(read_settings(&db).sliding_window_hours, 720);

        db.set_setting("app_config", r#"{"summary":{"slidingWindowHours":0}}"#)
            .unwrap();
        assert_eq!(read_settings(&db).sliding_window_hours, 1);
    }

    #[test]
    fn blank_custom_prompt_falls_back_to_the_default() {
        let db = temp_db("blank_prompt");
        db.set_setting("app_config", r#"{"summary":{"customPrompt":"   "}}"#)
            .unwrap();
        assert_eq!(read_settings(&db).custom_prompt, DEFAULT_PROMPT);
    }

    #[test]
    fn malformed_config_does_not_panic() {
        let db = temp_db("bad_json");
        db.set_setting("app_config", "{not json").unwrap();
        let s = read_settings(&db);
        assert!(s.enabled);
        assert_eq!(s.sliding_window_hours, 6);
    }

    #[test]
    fn policy_load_denies_an_unknown_target_against_a_real_database() {
        let db = temp_db("policy_unknown");
        let p = crate::services::policy::load(&db, "999999999");
        assert!(!p.exists);
        assert!(!p.tracked());
        assert!(!p.ai_execution_allowed());
    }

    #[test]
    fn policy_load_respects_a_persisted_rule() {
        let db = temp_db("policy_rule");
        db.upsert_rule(&crate::services::db::ContactRuleRecord {
            target_id: "1104661022".to_string(),
            target_type: "group".to_string(),
            name: "测试群".to_string(),
            avatar_url: String::new(),
            mode: "copilot".to_string(),
            trigger_condition: "at_me".to_string(),
            keywords: "[]".to_string(),
            cooldown_seconds: 5,
            enabled: true,
            is_summary_whitelist: true,
            summary_interval_hours: 2,
            updated_at: 0,
        })
        .unwrap();

        let p = crate::services::policy::load(&db, "1104661022");
        assert!(p.exists);
        assert!(p.tracked());
        assert!(p.ai_execution_allowed());
        assert_eq!(p.summary_interval_hours, 2);
    }

    #[test]
    fn summary_whitelist_groups_are_queryable() {
        let db = temp_db("whitelist_query");
        db.upsert_rule(&crate::services::db::ContactRuleRecord {
            target_id: "123".to_string(),
            target_type: "group".to_string(),
            name: "g".to_string(),
            avatar_url: String::new(),
            mode: "ignore".to_string(),
            trigger_condition: "at_me".to_string(),
            keywords: "[]".to_string(),
            cooldown_seconds: 5,
            enabled: false,
            is_summary_whitelist: true,
            summary_interval_hours: 3,
            updated_at: 0,
        })
        .unwrap();

        let groups = db.get_summary_whitelist_groups().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].summary_interval_hours, 3);
    }
}
