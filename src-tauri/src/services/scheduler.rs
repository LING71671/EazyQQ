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
use crate::services::onebot::OneBotClient;
use crate::services::summarizer::{self, SummaryRequest, DEFAULT_PROMPT};

const TICK_SECONDS: u64 = 60;

/// The interval every rule used to be pre-filled with. Kept only so the one-time
/// normalisation above can recognise rows written by older versions.
const LEGACY_DEFAULT_INTERVAL_HOURS: i32 = 6;

#[derive(Debug, Clone)]
pub struct SummarySettings {
    pub enabled: bool,
    pub sliding_window_hours: i32,
    pub custom_prompt: String,
    /// Cadence from `summary.intervalType` / `customIntervalMinutes`, in hours.
    ///
    /// These two settings were selectable in the UI and read by nothing - the scheduler
    /// only ever looked at each rule's own interval, so choosing "every 2 hours" changed
    /// nothing. They are the installation-wide default now; a rule whose own interval is
    /// 0 follows it, and a positive value overrides it for that group.
    pub interval_hours: i64,
}

impl Default for SummarySettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sliding_window_hours: 6,
            custom_prompt: DEFAULT_PROMPT.to_string(),
            interval_hours: 6,
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
        interval_hours: resolve_interval_hours(summary),
    }
}

/// Turn `intervalType` / `customIntervalMinutes` into an hour count.
///
/// `intervalType` is the user's choice from the settings dropdown; `custom` defers to the
/// minute field. An unrecognised or missing value falls back to 6h, matching the default
/// the UI shows, so a hand-edited config cannot silently change the cadence.
fn resolve_interval_hours(summary: &Value) -> i64 {
    let kind = summary
        .get("intervalType")
        .and_then(|v| v.as_str())
        .unwrap_or("6h");

    match kind {
        "1h" => 1,
        "2h" => 2,
        "4h" => 4,
        "6h" => 6,
        "12h" => 12,
        "24h" => 24,
        "custom" => {
            let minutes = summary
                .get("customIntervalMinutes")
                .and_then(|v| v.as_i64())
                .unwrap_or(360);
            // Round up so a sub-hour custom interval still yields a usable hour count,
            // and clamp so a typo cannot schedule something absurd.
            ((minutes + 59) / 60).clamp(1, 720)
        }
        other => {
            tracing::warn!(
                "scheduler: unrecognised intervalType {:?}, falling back to 6h",
                other
            );
            6
        }
    }
}

/// One-time normalisation of rule intervals written before the global cadence existed.
///
/// `contact_rules.summary_interval_hours` used to be pre-filled with 6 for every group and
/// was the only value the scheduler ever read, which is why the cadence dropdown on the
/// settings page did nothing. Now 0 means "follow the global cadence", but existing rows
/// still carry the old hardcoded 6 - and that reads as an explicit override, so the global
/// setting would keep being ignored on every upgraded install.
///
/// Only the exact legacy default is touched. Since the global default is also 6h, behaviour
/// is unchanged until the user picks a different cadence, at which point following it is
/// precisely what they asked for. Returns how many rules were adjusted.
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

/// `summary.autoForwardToPhone` - has always been in the settings UI and read by nothing.
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

/// Forward a finished summary to the account's own conversation.
///
/// Sending it to ourselves is what "forward to phone" actually means here: QQ syncs that
/// conversation to the mobile client, so no separate push channel is needed.
async fn forward_summary_to_self(onebot: &Arc<OneBotClient>, outcome: &summarizer::SummaryOutcome) {
    let self_id = match onebot.get_login_info().await {
        Ok(v) => {
            let data = v.get("data").cloned().unwrap_or(serde_json::Value::Null);
            // OneBot calls it `user_id`; NapCat's own API calls it `uin`.
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
    // Window covered by this summary, so the message is self-explanatory on the phone.
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

/// One scheduler pass. Returns the targets that were summarised.
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
        // A rule interval of 0 means "follow the installation-wide cadence".
        let interval_hours = if group.summary_interval_hours > 0 {
            group.summary_interval_hours as i64
        } else {
            settings.interval_hours
        }
        .max(1);
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

/// Spawn the background loop. Safe to call once during app setup.
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
    fn interval_type_maps_to_hours() {
        // These two settings were selectable in the UI and read by nothing until now, so
        // each branch needs to actually produce a cadence.
        for (kind, expected) in [("1h", 1), ("2h", 2), ("4h", 4), ("6h", 6), ("12h", 12), ("24h", 24)] {
            let v = serde_json::json!({ "intervalType": kind });
            assert_eq!(resolve_interval_hours(&v), expected, "intervalType {kind}");
        }
    }

    #[test]
    fn custom_interval_uses_the_minute_field() {
        let v = serde_json::json!({ "intervalType": "custom", "customIntervalMinutes": 180 });
        assert_eq!(resolve_interval_hours(&v), 3);

        // Sub-hour values round up rather than collapsing to zero.
        let v = serde_json::json!({ "intervalType": "custom", "customIntervalMinutes": 30 });
        assert_eq!(resolve_interval_hours(&v), 1);
    }

    #[test]
    fn custom_interval_is_clamped() {
        let v = serde_json::json!({ "intervalType": "custom", "customIntervalMinutes": 0 });
        assert_eq!(resolve_interval_hours(&v), 1, "must not schedule a zero-length cycle");

        let v = serde_json::json!({ "intervalType": "custom", "customIntervalMinutes": 999999 });
        assert_eq!(resolve_interval_hours(&v), 720, "a typo must not schedule years ahead");
    }

    #[test]
    fn unknown_or_missing_interval_type_falls_back_to_six_hours() {
        assert_eq!(resolve_interval_hours(&serde_json::json!({})), 6);
        assert_eq!(
            resolve_interval_hours(&serde_json::json!({ "intervalType": "whenever" })),
            6
        );
    }

    #[test]
    fn global_cadence_is_read_from_config() {
        let db = temp_db("interval_type");
        db.set_setting(
            "app_config",
            r#"{"summary":{"intervalType":"2h"}}"#,
        )
        .unwrap();
        assert_eq!(read_settings(&db).interval_hours, 2);

        db.set_setting(
            "app_config",
            r#"{"summary":{"intervalType":"custom","customIntervalMinutes":90}}"#,
        )
        .unwrap();
        assert_eq!(read_settings(&db).interval_hours, 2, "90 minutes rounds up to 2h");
    }

    #[test]
    fn a_rule_interval_of_zero_defers_to_the_global_cadence() {
        // The resolution rule itself: 0 = follow the installation-wide setting, anything
        // positive is an explicit override for that group.
        let resolve = |rule: i32, global: i64| -> i64 {
            if rule > 0 { rule as i64 } else { global }.max(1)
        };
        assert_eq!(resolve(0, 3), 3, "0 must follow the global cadence");
        assert_eq!(resolve(12, 3), 12, "a positive value overrides the global cadence");
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
