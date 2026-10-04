//! Roster synchronisation: merge the live OneBot friend/group roster with the
//! persisted rules in SQLite.
//!
//! Invariant enforced here (see ISSUE-006 / ROADMAP Phase 2): **default deny**.
//! Every contact we have never seen before is inserted with `mode = "ignore"` and
//! `is_summary_whitelist = false`. Nothing is ever auto-enabled.

use std::collections::HashMap;

use crate::services::db::{ContactRuleRecord, Database};
use crate::services::onebot::OneBotClient;

pub struct RosterSyncReport {
    pub friends: usize,
    pub groups: usize,
    /// Newly inserted default-deny rules.
    pub created: usize,
    /// Existing rules whose display name or avatar was refreshed.
    pub refreshed: usize,
    /// True when OneBot could not be reached and only the cached rules were returned.
    pub offline: bool,
    pub rules: Vec<ContactRuleRecord>,
}

fn friend_avatar(target_id: &str) -> String {
    format!("https://q1.qlogo.cn/g?b=qq&nk={}&s=100", target_id)
}

fn group_avatar(target_id: &str) -> String {
    format!("https://p.qlogo.cn/gh/{0}/{0}/100", target_id)
}

fn default_rule(target_id: &str, target_type: &str, name: &str) -> ContactRuleRecord {
    ContactRuleRecord {
        target_id: target_id.to_string(),
        target_type: target_type.to_string(),
        name: name.to_string(),
        avatar_url: if target_type == "group" {
            group_avatar(target_id)
        } else {
            friend_avatar(target_id)
        },
        mode: "ignore".to_string(),
        trigger_condition: if target_type == "group" {
            "at_me".to_string()
        } else {
            "all".to_string()
        },
        keywords: "[]".to_string(),
        cooldown_seconds: 5,
        enabled: false,
        is_summary_whitelist: false,
        // 0 means "follow the installation-wide cadence from the settings page"; a
        // positive value is an explicit per-group override. Defaulting to a number here
        // would shadow the global setting for every group.
        summary_interval_hours: 0,
        updated_at: 0,
    }
}

/// Fetch the live roster and reconcile it with SQLite.
///
/// Never fails hard on a network problem: if OneBot is unreachable the cached rules
/// are returned with `offline = true`, so the UI/CLI can still show the configuration.
pub async fn sync_roster(db: &Database, onebot: &OneBotClient) -> RosterSyncReport {
    let mut rules = db.get_all_rules().unwrap_or_default();
    let mut index: HashMap<String, usize> = rules
        .iter()
        .enumerate()
        .map(|(i, r)| (r.target_id.clone(), i))
        .collect();

    let mut created = 0usize;
    let mut refreshed = 0usize;

    let (friends, friends_ok) = match onebot.get_friend_list().await {
        Ok(list) => (list, true),
        Err(e) => {
            tracing::warn!("roster sync: friend list unavailable ({})", e);
            (Vec::new(), false)
        }
    };

    let (groups, groups_ok) = match onebot.get_group_list().await {
        Ok(list) => (list, true),
        Err(e) => {
            tracing::warn!("roster sync: group list unavailable ({})", e);
            (Vec::new(), false)
        }
    };

    let offline = !friends_ok && !groups_ok;
    if offline {
        tracing::warn!("roster sync: OneBot unreachable, serving {} cached rules", rules.len());
        return RosterSyncReport {
            friends: 0,
            groups: 0,
            created,
            refreshed,
            offline,
            rules,
        };
    }

    let apply = |target_id: String,
                     target_type: &str,
                     name: String,
                     avatar: String,
                     rules: &mut Vec<ContactRuleRecord>,
                     index: &mut HashMap<String, usize>,
                     created: &mut usize,
                     refreshed: &mut usize| {
        if let Some(&pos) = index.get(&target_id) {
            let existing = &mut rules[pos];
            let mut dirty = false;
            if !name.is_empty() && existing.name != name {
                existing.name = name;
                dirty = true;
            }
            if !avatar.is_empty() && existing.avatar_url != avatar {
                existing.avatar_url = avatar;
                dirty = true;
            }
            if dirty {
                let snapshot = existing.clone();
                let _ = db.upsert_rule(&snapshot);
                *refreshed += 1;
            }
        } else {
            let rule = default_rule(&target_id, target_type, &name);
            if let Err(e) = db.upsert_rule(&rule) {
                tracing::error!("roster sync: cannot persist default rule for {}: {}", target_id, e);
            } else {
                tracing::info!(
                    "roster sync: new {} {} ({}) inserted as default-deny",
                    target_type,
                    target_id,
                    name
                );
            }
            index.insert(target_id.clone(), rules.len());
            rules.push(rule);
            *created += 1;
        }
    };

    for f in &friends {
        let target_id = f.user_id.to_string();
        let name = f
            .remark
            .clone()
            .filter(|r| !r.is_empty())
            .unwrap_or_else(|| f.nickname.clone());
        let avatar = friend_avatar(&target_id);
        apply(
            target_id,
            "friend",
            name,
            avatar,
            &mut rules,
            &mut index,
            &mut created,
            &mut refreshed,
        );
    }

    for g in &groups {
        let target_id = g.group_id.to_string();
        let avatar = group_avatar(&target_id);
        apply(
            target_id,
            "group",
            g.group_name.clone(),
            avatar,
            &mut rules,
            &mut index,
            &mut created,
            &mut refreshed,
        );
    }

    rules.sort_by(|a, b| {
        b.target_type
            .cmp(&a.target_type)
            .then_with(|| a.name.cmp(&b.name))
    });

    tracing::info!(
        "roster sync: {} friends, {} groups, {} created, {} refreshed",
        friends.len(),
        groups.len(),
        created,
        refreshed
    );

    RosterSyncReport {
        friends: friends.len(),
        groups: groups.len(),
        created,
        refreshed,
        offline,
        rules,
    }
}
