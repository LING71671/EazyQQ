use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactRuleRecord {
    pub target_id: String,
    pub target_type: String,
    pub name: String,
    pub avatar_url: String,
    pub mode: String,
    pub trigger_condition: String,
    pub keywords: String,
    pub cooldown_seconds: i32,
    pub enabled: bool,
    pub is_summary_whitelist: bool,
    pub summary_interval_hours: i32,
    pub updated_at: i64,
}

pub fn get_all_rules(conn: &Connection) -> Result<Vec<ContactRuleRecord>> {
    let mut stmt = conn.prepare(
        "SELECT target_id, target_type, name, avatar_url, mode, trigger_condition,
                keywords, cooldown_seconds, enabled, is_summary_whitelist, summary_interval_hours, updated_at
         FROM contact_rules"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ContactRuleRecord {
            target_id: row.get(0)?,
            target_type: row.get(1)?,
            name: row.get(2)?,
            avatar_url: row.get(3)?,
            mode: row.get(4)?,
            trigger_condition: row.get(5)?,
            keywords: row.get(6)?,
            cooldown_seconds: row.get(7)?,
            enabled: row.get::<_, i32>(8)? != 0,
            is_summary_whitelist: row.get::<_, i32>(9)? != 0,
            summary_interval_hours: row.get(10)?,
            updated_at: row.get(11)?,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn upsert_rule(conn: &Connection, rule: &ContactRuleRecord, now: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO contact_rules (
            target_id, target_type, name, avatar_url, mode, trigger_condition,
            keywords, cooldown_seconds, enabled, is_summary_whitelist, summary_interval_hours, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        ON CONFLICT(target_id) DO UPDATE SET
            mode = excluded.mode,
            trigger_condition = excluded.trigger_condition,
            keywords = excluded.keywords,
            cooldown_seconds = excluded.cooldown_seconds,
            enabled = excluded.enabled,
            is_summary_whitelist = excluded.is_summary_whitelist,
            summary_interval_hours = excluded.summary_interval_hours,
            updated_at = excluded.updated_at",
        params![
            rule.target_id,
            rule.target_type,
            rule.name,
            rule.avatar_url,
            rule.mode,
            rule.trigger_condition,
            rule.keywords,
            rule.cooldown_seconds,
            if rule.enabled { 1 } else { 0 },
            if rule.is_summary_whitelist { 1 } else { 0 },
            rule.summary_interval_hours,
            now
        ],
    )?;
    Ok(())
}

pub fn batch_update_mode(conn: &Connection, target_ids: &[String], mode: &str, now: i64) -> Result<usize> {
    let mut affected = 0;
    for id in target_ids {
        let enabled = if mode == "ignore" { 0 } else { 1 };
        let count = conn.execute(
            "UPDATE contact_rules SET mode = ?1, enabled = ?2, updated_at = ?3 WHERE target_id = ?4",
            params![mode, enabled, now, id],
        )?;
        affected += count;
    }
    Ok(affected)
}

pub fn set_summary_whitelist(conn: &Connection, target_id: &str, is_whitelist: bool, interval_hours: i32, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE contact_rules 
         SET is_summary_whitelist = ?1, summary_interval_hours = ?2, updated_at = ?3 
         WHERE target_id = ?4",
        params![if is_whitelist { 1 } else { 0 }, interval_hours, now, target_id],
    )?;
    Ok(())
}

pub fn get_summary_whitelist_groups(conn: &Connection) -> Result<Vec<ContactRuleRecord>> {
    let mut stmt = conn.prepare(
        "SELECT target_id, target_type, name, avatar_url, mode, trigger_condition,
                keywords, cooldown_seconds, enabled, is_summary_whitelist, summary_interval_hours, updated_at
         FROM contact_rules
         WHERE target_type = 'group' AND is_summary_whitelist = 1"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(ContactRuleRecord {
            target_id: row.get(0)?,
            target_type: row.get(1)?,
            name: row.get(2)?,
            avatar_url: row.get(3)?,
            mode: row.get(4)?,
            trigger_condition: row.get(5)?,
            keywords: row.get(6)?,
            cooldown_seconds: row.get(7)?,
            enabled: row.get::<_, i32>(8)? != 0,
            is_summary_whitelist: row.get::<_, i32>(9)? != 0,
            summary_interval_hours: row.get(10)?,
            updated_at: row.get(11)?,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}
