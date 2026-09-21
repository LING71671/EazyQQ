//! Central whitelist policy.
//!
//! This is the single place that answers "may this target be processed at all?".
//! It replaces the earlier hardcoded `AUTHORIZED_TEST_UIN` gate, which contradicted the
//! rule-driven design documented in ROADMAP Phase 2 and blocked every real target.
//!
//! Two independent decisions, both strictly **default deny**:
//!
//! | decision            | condition                                        |
//! |---------------------|--------------------------------------------------|
//! | `tracked`           | `mode != ignore` **or** `is_summary_whitelist`   |
//! | `ai_execution_allowed` | `enabled == true` and `mode != ignore`        |
//!
//! `tracked` controls whether a message is even persisted / emitted to the UI. Anything
//! not tracked is bypassed completely: no message row, no draft, no AI call.

use crate::services::db::Database;

#[derive(Debug, Clone)]
pub struct TargetPolicy {
    /// The target has a persisted rule row at all.
    pub exists: bool,
    pub mode: String,
    pub enabled: bool,
    pub is_summary_whitelist: bool,
    pub summary_interval_hours: i32,
    pub name: String,
}

impl Default for TargetPolicy {
    fn default() -> Self {
        Self {
            exists: false,
            mode: "ignore".to_string(),
            enabled: false,
            is_summary_whitelist: false,
            summary_interval_hours: 6,
            name: String::new(),
        }
    }
}

impl TargetPolicy {
    /// Should messages from this target be recorded and forwarded to the UI?
    /// True only when the user explicitly opted in to message takeover or summaries.
    pub fn tracked(&self) -> bool {
        self.mode != "ignore" || self.is_summary_whitelist
    }

    /// May we run AI execution (auto reply / draft generation) for this target?
    pub fn ai_execution_allowed(&self) -> bool {
        self.enabled && self.mode != "ignore"
    }

    /// One-line description used in logs.
    pub fn describe(&self) -> String {
        format!(
            "mode={}, enabled={}, summaryWhitelist={} ({}h)",
            self.mode, self.enabled, self.is_summary_whitelist, self.summary_interval_hours
        )
    }
}

/// Load the policy for a target. Missing rule => strict default deny.
pub fn load(db: &Database, target_id: &str) -> TargetPolicy {
    match db.get_all_rules() {
        Ok(rules) => rules
            .into_iter()
            .find(|r| r.target_id == target_id)
            .map(|r| TargetPolicy {
                exists: true,
                mode: r.mode,
                enabled: r.enabled,
                is_summary_whitelist: r.is_summary_whitelist,
                summary_interval_hours: r.summary_interval_hours,
                name: r.name,
            })
            .unwrap_or_default(),
        Err(e) => {
            tracing::error!("policy: cannot read rules for {}: {} (denying by default)", target_id, e);
            TargetPolicy::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(mode: &str, enabled: bool, summary: bool) -> TargetPolicy {
        TargetPolicy {
            exists: true,
            mode: mode.to_string(),
            enabled,
            is_summary_whitelist: summary,
            summary_interval_hours: 6,
            name: "t".to_string(),
        }
    }

    #[test]
    fn default_policy_denies_everything() {
        let p = TargetPolicy::default();
        assert!(!p.exists);
        assert!(!p.tracked(), "an unknown target must never be tracked");
        assert!(
            !p.ai_execution_allowed(),
            "an unknown target must never get AI execution"
        );
    }

    #[test]
    fn ignore_without_summary_is_fully_bypassed() {
        let p = policy("ignore", false, false);
        assert!(!p.tracked());
        assert!(!p.ai_execution_allowed());
    }

    #[test]
    fn ignore_with_summary_whitelist_is_tracked_but_never_ai() {
        let p = policy("ignore", false, true);
        assert!(p.tracked(), "summary whitelist needs the message stream");
        assert!(
            !p.ai_execution_allowed(),
            "summary tracking must never trigger replies"
        );
    }

    #[test]
    fn disabled_rule_is_tracked_but_never_ai() {
        for mode in ["auto_reply", "copilot"] {
            let p = policy(mode, false, false);
            assert!(p.tracked());
            assert!(
                !p.ai_execution_allowed(),
                "mode {mode} with enabled=false must not execute AI"
            );
        }
    }

    #[test]
    fn enabled_ai_modes_execute() {
        for mode in ["auto_reply", "copilot"] {
            let p = policy(mode, true, false);
            assert!(p.tracked());
            assert!(p.ai_execution_allowed(), "mode {mode} should execute");
        }
    }

    #[test]
    fn enabled_ignore_still_never_executes() {
        let p = policy("ignore", true, false);
        assert!(!p.ai_execution_allowed());
    }
}
