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
