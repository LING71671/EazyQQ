//! Configuration: one source of truth, plus a check that nothing is declared and ignored.

use serde_json::{json, Value};

/// The configuration a fresh install starts with.
pub fn default_app_config() -> Value {
    json!({
        "ai": {
            "activeProvider": "opencode",
            "model": "qwen3.8-flash",
            "temperature": 0.7,
            "maxContextMessages": 10,
            "baseUrl": "https://opencode.ai/zen/v1",
            "apiKey": ""
        },
        "napcat": {
            "wsPort": 3001,
            "autoRestart": true,
            "heartbeatIntervalSec": 15
        },
        "storage": {
            "autoSyncFiles": true,
            "maxFileSizeMb": 100
        },
        "summary": {
            "enabled": true,
            "intervalType": "6h",
            "customIntervalMinutes": 360,
            "slidingWindowHours": 6,
            "autoForwardToPhone": false,
            "customPrompt": "请提炼群聊核心讨论要点、决策事项与待办行动项，结构清晰明了。"
        },
        "window": {
            "minimizeToTray": true,
            "closeToTray": true
        }
    })
}

/// Leaf paths of the default config, e.g. `ai.model`, `summary.enabled`.
pub fn declared_keys() -> Vec<String> {
    fn walk(value: &Value, prefix: &str, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (k, v) in map {
                    let path = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{}.{}", prefix, k)
                    };
                    walk(v, &path, out);
                }
            }
            _ => out.push(prefix.to_string()),
        }
    }

    let mut out = Vec::new();
    walk(&default_app_config(), "", &mut out);
    out.sort();
    out
}

/// Every backend source that may consume configuration.
const SOURCES: &[(&str, &str)] = &[
    ("ai/config.rs", include_str!("../ai/config.rs")),
    ("ai/client.rs", include_str!("../ai/client.rs")),
    ("workflows/chain.rs", include_str!("../workflows/chain.rs")),
    ("identity/contacts.rs", include_str!("../identity/contacts.rs")),
    ("infra/diagnostics.rs", include_str!("diagnostics.rs")),
    ("infra/bundle.rs", include_str!("bundle.rs")),
    ("workflows/group_files.rs", include_str!("../workflows/group_files.rs")),
    ("workflows/file_summary.rs", include_str!("../workflows/file_summary.rs")),
    ("infra/logging.rs", include_str!("logging.rs")),
    ("security/policy.rs", include_str!("../security/policy.rs")),
    ("workflows/scheduler.rs", include_str!("../workflows/scheduler.rs")),
    ("workflows/scheduler_settings.rs", include_str!("../workflows/scheduler_settings.rs")),
    ("workflows/summarizer.rs", include_str!("../workflows/summarizer.rs")),
    ("protocol/listener.rs", include_str!("../protocol/listener.rs")),
    ("protocol/events.rs", include_str!("../protocol/events.rs")),
    ("commands.rs", include_str!("../../commands.rs")),
    ("lib.rs", include_str!("../../lib.rs")),
];

/// Which source file, if any, reads a given config field.
fn consumer_of(leaf: &str) -> Option<&'static str> {
    let needle = format!("\"{}\"", leaf);
    SOURCES
        .iter()
        .find(|(_, src)| src.contains(&needle))
        .map(|(name, _)| *name)
}

/// Configuration the UI offers but the backend never reads.
pub fn unread_keys() -> Vec<String> {
    declared_keys()
        .into_iter()
        .filter(|key| {
            let leaf = key.rsplit('.').next().unwrap_or(key);
            consumer_of(leaf).is_none()
        })
        .collect()
}

/// Declared fields paired with the file that reads them, for the audit report.
pub fn audit() -> Vec<(String, Option<&'static str>)> {
    declared_keys()
        .into_iter()
        .map(|key| {
            let leaf = key.rsplit('.').next().unwrap_or(&key).to_string();
            let consumer = consumer_of(&leaf);
            (key, consumer)
        })
        .collect()
}
