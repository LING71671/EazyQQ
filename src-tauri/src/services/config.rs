//! Configuration: one source of truth, plus a check that nothing is declared and ignored.
//!
//! Eight separate settings have shipped that the UI let you change and the backend never
//! read - the test-mode UIN gate, trigger conditions, reply cooldown, `autoRestart`,
//! `heartbeatIntervalSec`, `autoSyncFiles`, `autoForwardToPhone`, and the summary cadence.
//! Every one of them looked like a working feature and did nothing. The cause is
//! structural: the config contract and its consumers have no link, so adding a field is
//! easy to do halfway.
//!
//! This module closes that gap. `default_app_config` is the authoritative list of fields,
//! and the test at the bottom asserts that each one is actually read somewhere in the
//! backend. Because the sources are embedded with `include_str!`, the check runs as an
//! ordinary unit test - no external tooling, no CI step to forget.

use serde_json::{json, Value};

/// The configuration a fresh install starts with.
///
/// This is the single place that defines which settings exist. Anything added here must
/// also be consumed somewhere, or `every_config_field_is_read_somewhere` fails.
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
///
/// Embedded at compile time so the check needs no filesystem access or external tool, and
/// runs identically from a unit test and from the CLI.
const SOURCES: &[(&str, &str)] = &[
    ("ai.rs", include_str!("ai.rs")),
    ("chain.rs", include_str!("chain.rs")),
    ("contacts.rs", include_str!("contacts.rs")),
    ("diagnostics.rs", include_str!("diagnostics.rs")),
    ("group_files.rs", include_str!("group_files.rs")),
    ("logging.rs", include_str!("logging.rs")),
    ("policy.rs", include_str!("policy.rs")),
    ("scheduler.rs", include_str!("scheduler.rs")),
    ("summarizer.rs", include_str!("summarizer.rs")),
    ("ws_listener.rs", include_str!("ws_listener.rs")),
    ("commands.rs", include_str!("../commands.rs")),
    ("lib.rs", include_str!("../lib.rs")),
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
///
/// A setting that silently does nothing is worse than a missing feature: it tells the user
/// their choice took effect when it did not. Eight shipped that way, which is why this
/// check exists rather than relying on someone noticing.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The check that would have caught eight shipped bugs.
    #[test]
    fn every_config_field_is_read_somewhere() {
        let unread = unread_keys();
        assert!(
            unread.is_empty(),
            "these config fields are declared in default_app_config but never read by the \
             backend, so changing them in the UI does nothing: {:?}",
            unread
        );
    }

    #[test]
    fn declared_keys_cover_the_expected_sections() {
        let keys = declared_keys();
        for expected in [
            "ai.activeProvider",
            "ai.model",
            "ai.temperature",
            "ai.maxContextMessages",
            "ai.baseUrl",
            "ai.apiKey",
            "napcat.wsPort",
            "napcat.autoRestart",
            "napcat.heartbeatIntervalSec",
            "storage.autoSyncFiles",
            "storage.maxFileSizeMb",
            "summary.enabled",
            "summary.intervalType",
            "summary.customIntervalMinutes",
            "summary.slidingWindowHours",
            "summary.autoForwardToPhone",
            "summary.customPrompt",
            "window.minimizeToTray",
            "window.closeToTray",
        ] {
            assert!(keys.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn the_removed_workspace_dir_field_is_gone() {
        // It was declared, absent from the UI, and never read - a field that implies a
        // feature which does not exist. Removed from the contract; this keeps the backend
        // from drifting back out of sync with it.
        assert!(!declared_keys().iter().any(|k| k.contains("workspaceDir")));
    }
}
