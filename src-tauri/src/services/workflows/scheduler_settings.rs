use serde_json::Value;

use crate::services::db::Database;
use super::summarizer::DEFAULT_PROMPT;

#[derive(Debug, Clone)]
pub struct SummarySettings {
    pub enabled: bool,
    pub sliding_window_hours: i32,
    pub custom_prompt: String,
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

pub fn resolve_interval_hours(summary: &Value) -> i64 {
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
