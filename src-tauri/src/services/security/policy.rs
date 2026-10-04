use crate::services::db::Database;

#[derive(Debug, Clone)]
pub struct TargetPolicy {
    pub exists: bool,
    pub mode: String,
    pub enabled: bool,
    pub is_summary_whitelist: bool,
    pub summary_interval_hours: i32,
    pub name: String,
    pub trigger_condition: String,
    pub keywords: Vec<String>,
    pub cooldown_seconds: i32,
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
            trigger_condition: "all".to_string(),
            keywords: Vec::new(),
            cooldown_seconds: 0,
        }
    }
}

impl TargetPolicy {
    pub fn tracked(&self) -> bool {
        self.mode != "ignore" || self.is_summary_whitelist
    }

    pub fn ai_execution_allowed(&self) -> bool {
        self.enabled && self.mode != "ignore"
    }

    pub fn describe(&self) -> String {
        format!(
            "exists={}, mode={}, enabled={}, summary_whitelist={}, trigger={}, cooldown={}s",
            self.exists,
            self.mode,
            self.enabled,
            self.is_summary_whitelist,
            self.trigger_condition,
            self.cooldown_seconds
        )
    }
}

pub fn load(db: &Database, target_id: &str) -> TargetPolicy {
    let rules = match db.get_all_rules() {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("cannot load rules for {}: {}; using default-deny", target_id, e);
            return TargetPolicy::default();
        }
    };

    let Some(rule) = rules.into_iter().find(|r| r.target_id == target_id) else {
        return TargetPolicy::default();
    };

    let keywords: Vec<String> = serde_json::from_str(&rule.keywords)
        .unwrap_or_default();

    TargetPolicy {
        exists: true,
        mode: rule.mode,
        enabled: rule.enabled,
        is_summary_whitelist: rule.is_summary_whitelist,
        summary_interval_hours: rule.summary_interval_hours,
        name: rule.name,
        trigger_condition: if rule.trigger_condition.is_empty() {
            "at_me".to_string()
        } else {
            rule.trigger_condition
        },
        keywords,
        cooldown_seconds: rule.cooldown_seconds.max(0),
    }
}
