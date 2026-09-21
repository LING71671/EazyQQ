use rusqlite::{params, Connection, Result};
use std::fs;
use std::path::Path;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};

pub struct Database {
    conn: Mutex<Connection>,
}

/// One group file tracked by the silent-sync pipeline (ROADMAP Phase 4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupFileRecord {
    pub file_id: String,
    pub group_id: String,
    pub file_name: String,
    pub file_size: i64,
    pub busid: i64,
    pub uploader_name: String,
    pub upload_time: i64,
    pub local_path: Option<String>,
    /// `remote` | `downloaded` | `failed`
    pub download_status: String,
    pub updated_at: i64,
}

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

impl Database {
    pub fn init<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        if let Some(parent) = db_path.as_ref().parent() {
            let _ = fs::create_dir_all(parent);
        }

        let conn = Connection::open(db_path)?;

        // Enable WAL mode for high concurrency
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        // Initialize schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS contact_rules (
                target_id TEXT PRIMARY KEY,
                target_type TEXT NOT NULL,
                name TEXT NOT NULL,
                avatar_url TEXT NOT NULL DEFAULT '',
                mode TEXT NOT NULL DEFAULT 'ignore',
                trigger_condition TEXT NOT NULL DEFAULT 'at_me',
                keywords TEXT NOT NULL DEFAULT '[]',
                cooldown_seconds INTEGER NOT NULL DEFAULT 5,
                enabled INTEGER NOT NULL DEFAULT 0,
                is_summary_whitelist INTEGER NOT NULL DEFAULT 0,
                summary_interval_hours INTEGER NOT NULL DEFAULT 6,
                updated_at INTEGER NOT NULL
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS group_summaries (
                id TEXT PRIMARY KEY,
                target_id TEXT NOT NULL,
                target_name TEXT NOT NULL,
                summary_text TEXT NOT NULL,
                key_points TEXT NOT NULL,
                decisions TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER NOT NULL,
                created_at INTEGER NOT NULL
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages_log (
                msg_id TEXT PRIMARY KEY,
                target_id TEXT NOT NULL,
                sender_id TEXT NOT NULL,
                sender_name TEXT NOT NULL,
                content TEXT NOT NULL,
                is_from_me INTEGER NOT NULL DEFAULT 0,
                ai_reply_status TEXT NOT NULL DEFAULT 'none',
                timestamp INTEGER NOT NULL
            );",
            [],
        )?;
        let _ = conn.execute("ALTER TABLE messages_log ADD COLUMN is_from_me INTEGER NOT NULL DEFAULT 0", []);
        let _ = conn.execute("ALTER TABLE messages_log ADD COLUMN ai_reply_status TEXT NOT NULL DEFAULT 'none'", []);

        conn.execute(
            "CREATE TABLE IF NOT EXISTS pending_drafts (
                id TEXT PRIMARY KEY,
                target_id TEXT NOT NULL,
                target_name TEXT NOT NULL,
                target_type TEXT NOT NULL,
                reply_to_msg_id TEXT NOT NULL,
                incoming_message_snippet TEXT NOT NULL,
                generated_content TEXT NOT NULL,
                thinking_content TEXT,
                model_used TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS group_files (
                file_id TEXT NOT NULL,
                group_id TEXT NOT NULL,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                busid INTEGER NOT NULL DEFAULT 0,
                uploader_name TEXT NOT NULL DEFAULT '',
                upload_time INTEGER NOT NULL DEFAULT 0,
                local_path TEXT,
                download_status TEXT NOT NULL DEFAULT 'remote',
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (group_id, file_id)
            );",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS read_state (
                target_id TEXT PRIMARY KEY,
                last_read_at INTEGER NOT NULL
            );",
            [],
        )?;

        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now_ms();
        conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now],
        )?;
        Ok(())
    }

    pub fn get_all_rules(&self) -> Result<Vec<ContactRuleRecord>> {
        let conn = self.conn.lock().unwrap();
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

    pub fn upsert_rule(&self, rule: &ContactRuleRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now_ms();
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

    pub fn batch_update_mode(&self, target_ids: &[String], mode: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now_ms();
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

    pub fn set_summary_whitelist(&self, target_id: &str, is_whitelist: bool, interval_hours: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono_now_ms();
        conn.execute(
            "UPDATE contact_rules 
             SET is_summary_whitelist = ?1, summary_interval_hours = ?2, updated_at = ?3 
             WHERE target_id = ?4",
            params![if is_whitelist { 1 } else { 0 }, interval_hours, now, target_id],
        )?;
        Ok(())
    }

    pub fn save_summary(&self, summary: &crate::models::GroupSummaryDto) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let key_points_json = serde_json::to_string(&summary.key_points).unwrap_or_else(|_| "[]".to_string());
        let decisions_json = serde_json::to_string(&summary.decisions).unwrap_or_else(|_| "[]".to_string());
        conn.execute(
            "INSERT INTO group_summaries (
                id, target_id, target_name, summary_text, key_points, decisions, start_time, end_time, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                summary_text = excluded.summary_text,
                key_points = excluded.key_points,
                decisions = excluded.decisions,
                end_time = excluded.end_time",
            params![
                summary.id,
                summary.target_id,
                summary.target_name,
                summary.summary_text,
                key_points_json,
                decisions_json,
                summary.start_time,
                summary.end_time,
                summary.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_summaries(&self, target_id: Option<&str>) -> Result<Vec<crate::models::GroupSummaryDto>> {
        let conn = self.conn.lock().unwrap();
        let mut list = Vec::new();

        if let Some(tid) = target_id {
            let mut stmt = conn.prepare(
                "SELECT id, target_id, target_name, summary_text, key_points, decisions, start_time, end_time, created_at
                 FROM group_summaries WHERE target_id = ?1 ORDER BY created_at DESC"
            )?;
            let rows = stmt.query_map(params![tid], |row| {
                let kp_raw: String = row.get(4)?;
                let dec_raw: String = row.get(5)?;
                Ok(crate::models::GroupSummaryDto {
                    id: row.get(0)?,
                    target_id: row.get(1)?,
                    target_name: row.get(2)?,
                    summary_text: row.get(3)?,
                    key_points: serde_json::from_str(&kp_raw).unwrap_or_default(),
                    decisions: serde_json::from_str(&dec_raw).unwrap_or_default(),
                    shared_files: vec![],
                    start_time: row.get(6)?,
                    end_time: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?;
            for r in rows {
                list.push(r?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, target_id, target_name, summary_text, key_points, decisions, start_time, end_time, created_at
                 FROM group_summaries ORDER BY created_at DESC"
            )?;
            let rows = stmt.query_map([], |row| {
                let kp_raw: String = row.get(4)?;
                let dec_raw: String = row.get(5)?;
                Ok(crate::models::GroupSummaryDto {
                    id: row.get(0)?,
                    target_id: row.get(1)?,
                    target_name: row.get(2)?,
                    summary_text: row.get(3)?,
                    key_points: serde_json::from_str(&kp_raw).unwrap_or_default(),
                    decisions: serde_json::from_str(&dec_raw).unwrap_or_default(),
                    shared_files: vec![],
                    start_time: row.get(6)?,
                    end_time: row.get(7)?,
                    created_at: row.get(8)?,
                })
            })?;
            for r in rows {
                list.push(r?);
            }
        }
        Ok(list)
    }

    pub fn delete_summary(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM group_summaries WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_summary_whitelist_groups(&self) -> Result<Vec<ContactRuleRecord>> {
        let conn = self.conn.lock().unwrap();
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

    pub fn save_message(&self, msg: &crate::models::MessageItemDto) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO messages_log (
                msg_id, target_id, sender_id, sender_name, content, is_from_me, ai_reply_status, timestamp
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(msg_id) DO UPDATE SET
                content = excluded.content,
                ai_reply_status = excluded.ai_reply_status",
            params![
                msg.id,
                msg.target_id,
                msg.sender_id,
                msg.sender_name,
                msg.content,
                if msg.is_from_me { 1 } else { 0 },
                msg.ai_reply_status,
                msg.timestamp,
            ],
        )?;
        Ok(())
    }

    /// The most recent `limit` messages for a target, returned **oldest-first** so the
    /// result can be fed straight into an AI prompt as conversation history.
    ///
    /// (Previously this selected the *oldest* N rows, which meant a busy conversation
    /// gave the model the very beginning of the chat instead of the latest context.)
    pub fn get_messages_by_target(&self, target_id: &str, limit: usize) -> Result<Vec<crate::models::MessageItemDto>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT msg_id, target_id, sender_id, sender_name, content, is_from_me, ai_reply_status, timestamp
             FROM messages_log 
             WHERE target_id = ?1 
             ORDER BY timestamp DESC 
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![target_id, limit as i64], |row| {
            Ok(crate::models::MessageItemDto {
                id: row.get(0)?,
                target_id: row.get(1)?,
                sender_id: row.get(2)?,
                sender_name: row.get(3)?,
                content: row.get(4)?,
                is_from_me: row.get::<_, i32>(5)? != 0,
                ai_reply_status: row.get(6)?,
                timestamp: row.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        list.reverse();
        Ok(list)
    }

    /// Messages for a target inside a sliding time window, oldest-first.
    /// Used by the group summarizer (ROADMAP Phase 5).
    pub fn get_messages_in_window(
        &self,
        target_id: &str,
        since_ms: i64,
        limit: usize,
    ) -> Result<Vec<crate::models::MessageItemDto>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT msg_id, target_id, sender_id, sender_name, content, is_from_me, ai_reply_status, timestamp
             FROM messages_log
             WHERE target_id = ?1 AND timestamp >= ?2
             ORDER BY timestamp ASC
             LIMIT ?3"
        )?;

        let rows = stmt.query_map(params![target_id, since_ms, limit as i64], |row| {
            Ok(crate::models::MessageItemDto {
                id: row.get(0)?,
                target_id: row.get(1)?,
                sender_id: row.get(2)?,
                sender_name: row.get(3)?,
                content: row.get(4)?,
                is_from_me: row.get::<_, i32>(5)? != 0,
                ai_reply_status: row.get(6)?,
                timestamp: row.get(7)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // --- Group files (ROADMAP Phase 4) ---

    pub fn upsert_group_file(&self, file: &GroupFileRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO group_files (
                file_id, group_id, file_name, file_size, busid, uploader_name,
                upload_time, local_path, download_status, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(group_id, file_id) DO UPDATE SET
                file_name = excluded.file_name,
                file_size = excluded.file_size,
                busid = excluded.busid,
                uploader_name = excluded.uploader_name,
                upload_time = excluded.upload_time,
                local_path = COALESCE(excluded.local_path, group_files.local_path),
                download_status = excluded.download_status,
                updated_at = excluded.updated_at",
            params![
                file.file_id,
                file.group_id,
                file.file_name,
                file.file_size,
                file.busid,
                file.uploader_name,
                file.upload_time,
                file.local_path,
                file.download_status,
                file.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_group_files(&self, group_id: &str) -> Result<Vec<GroupFileRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT file_id, group_id, file_name, file_size, busid, uploader_name,
                    upload_time, local_path, download_status, updated_at
             FROM group_files
             WHERE group_id = ?1
             ORDER BY upload_time DESC",
        )?;

        let rows = stmt.query_map(params![group_id], |row| {
            Ok(GroupFileRecord {
                file_id: row.get(0)?,
                group_id: row.get(1)?,
                file_name: row.get(2)?,
                file_size: row.get(3)?,
                busid: row.get(4)?,
                uploader_name: row.get(5)?,
                upload_time: row.get(6)?,
                local_path: row.get(7)?,
                download_status: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn find_group_file(&self, group_id: &str, file_id: &str) -> Result<Option<GroupFileRecord>> {
        Ok(self
            .get_group_files(group_id)?
            .into_iter()
            .find(|f| f.file_id == file_id))
    }

    // --- Read state / unread counts ---

    /// Remember that the user has seen everything up to `at_ms` for this target.
    pub fn mark_read(&self, target_id: &str, at_ms: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO read_state (target_id, last_read_at) VALUES (?1, ?2)
             ON CONFLICT(target_id) DO UPDATE SET
                last_read_at = MAX(read_state.last_read_at, excluded.last_read_at)",
            params![target_id, at_ms],
        )?;
        Ok(())
    }

    pub fn get_last_read(&self, target_id: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT last_read_at FROM read_state WHERE target_id = ?1")?;
        let mut rows = stmt.query(params![target_id])?;
        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(0)
        }
    }

    /// Incoming messages newer than the last time this target was opened.
    ///
    /// Only messages we actually store count, which means only whitelisted targets can
    /// ever have unread messages - an unlisted chat is never recorded in the first place.
    pub fn count_unread(&self, target_id: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT COUNT(*) FROM messages_log
             WHERE target_id = ?1
               AND is_from_me = 0
               AND timestamp > COALESCE(
                     (SELECT last_read_at FROM read_state WHERE target_id = ?1), 0)",
        )?;
        let mut rows = stmt.query(params![target_id])?;
        if let Some(row) = rows.next()? {
            Ok(row.get(0)?)
        } else {
            Ok(0)
        }
    }

    pub fn save_draft(&self, draft: &crate::models::PendingDraftDto) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO pending_drafts (
                id, target_id, target_name, target_type, reply_to_msg_id,
                incoming_message_snippet, generated_content, thinking_content, model_used, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                generated_content = excluded.generated_content,
                thinking_content = excluded.thinking_content",
            params![
                draft.id,
                draft.target_id,
                draft.target_name,
                draft.target_type,
                draft.reply_to_msg_id,
                draft.incoming_message_snippet,
                draft.generated_content,
                draft.thinking_content,
                draft.model_used,
                draft.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_pending_drafts(&self) -> Result<Vec<crate::models::PendingDraftDto>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, target_id, target_name, target_type, reply_to_msg_id,
                    incoming_message_snippet, generated_content, thinking_content, model_used, created_at
             FROM pending_drafts ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(crate::models::PendingDraftDto {
                id: row.get(0)?,
                target_id: row.get(1)?,
                target_name: row.get(2)?,
                target_type: row.get(3)?,
                reply_to_msg_id: row.get(4)?,
                incoming_message_snippet: row.get(5)?,
                generated_content: row.get(6)?,
                thinking_content: row.get(7)?,
                model_used: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn delete_draft(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM pending_drafts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_draft_content(&self, id: &str, content: &str, thinking: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE pending_drafts SET generated_content = ?1, thinking_content = ?2 WHERE id = ?3",
            params![content, thinking, id],
        )?;
        Ok(())
    }
}

fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MessageItemDto;

    fn temp_db(tag: &str) -> Database {
        let dir = std::env::temp_dir().join("eazyqq_db_tests");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(format!("{}_{}.db", tag, std::process::id()));
        let _ = fs::remove_file(&path);
        Database::init(&path).expect("temp db")
    }

    fn msg(id: &str, target: &str, ts: i64, from_me: bool) -> MessageItemDto {
        MessageItemDto {
            id: id.to_string(),
            target_id: target.to_string(),
            sender_id: if from_me { "me".into() } else { "10001".into() },
            sender_name: if from_me { "我".into() } else { "对方".into() },
            content: "内容".to_string(),
            is_from_me: from_me,
            ai_reply_status: "none".to_string(),
            timestamp: ts,
        }
    }

    #[test]
    fn unread_counts_only_incoming_messages() {
        let db = temp_db("unread_incoming");
        db.save_message(&msg("m1", "g1", 1000, false)).unwrap();
        db.save_message(&msg("m2", "g1", 2000, true)).unwrap();
        db.save_message(&msg("m3", "g1", 3000, false)).unwrap();

        assert_eq!(db.count_unread("g1").unwrap(), 2, "our own message must not count");
    }

    #[test]
    fn unread_is_scoped_per_target() {
        let db = temp_db("unread_scope");
        db.save_message(&msg("a1", "g1", 1000, false)).unwrap();
        db.save_message(&msg("b1", "g2", 1000, false)).unwrap();

        assert_eq!(db.count_unread("g1").unwrap(), 1);
        assert_eq!(db.count_unread("g2").unwrap(), 1);
        assert_eq!(db.count_unread("g3").unwrap(), 0);
    }

    #[test]
    fn marking_read_clears_the_badge() {
        let db = temp_db("unread_mark");
        db.save_message(&msg("m1", "g1", 1000, false)).unwrap();
        db.save_message(&msg("m2", "g1", 2000, false)).unwrap();
        assert_eq!(db.count_unread("g1").unwrap(), 2);

        db.mark_read("g1", 2000).unwrap();
        assert_eq!(db.count_unread("g1").unwrap(), 0);

        // A newer message starts counting again.
        db.save_message(&msg("m3", "g1", 3000, false)).unwrap();
        assert_eq!(db.count_unread("g1").unwrap(), 1);
    }

    #[test]
    fn mark_read_never_moves_backwards() {
        let db = temp_db("unread_monotonic");
        db.save_message(&msg("m1", "g1", 5000, false)).unwrap();

        db.mark_read("g1", 5000).unwrap();
        assert_eq!(db.count_unread("g1").unwrap(), 0);

        // An out-of-order (older) mark must not resurrect already-read messages.
        db.mark_read("g1", 1000).unwrap();
        assert_eq!(db.get_last_read("g1").unwrap(), 5000);
        assert_eq!(db.count_unread("g1").unwrap(), 0);
    }

    #[test]
    fn last_read_defaults_to_zero_for_an_unseen_target() {
        let db = temp_db("unread_default");
        assert_eq!(db.get_last_read("never-opened").unwrap(), 0);
        assert_eq!(db.count_unread("never-opened").unwrap(), 0);
    }

    #[test]
    fn messages_for_a_target_are_returned_newest_last() {
        let db = temp_db("history_order");
        for ts in [1000, 3000, 2000] {
            db.save_message(&msg(&format!("m{}", ts), "g1", ts, false)).unwrap();
        }

        let history = db.get_messages_by_target("g1", 10).unwrap();
        let stamps: Vec<i64> = history.iter().map(|m| m.timestamp).collect();
        assert_eq!(stamps, vec![1000, 2000, 3000], "must be chronological");
    }

    #[test]
    fn history_limit_keeps_the_newest_messages() {
        let db = temp_db("history_limit");
        for ts in [1000, 2000, 3000, 4000, 5000] {
            db.save_message(&msg(&format!("m{}", ts), "g1", ts, false)).unwrap();
        }

        let history = db.get_messages_by_target("g1", 2).unwrap();
        let stamps: Vec<i64> = history.iter().map(|m| m.timestamp).collect();
        assert_eq!(stamps, vec![4000, 5000], "the window must keep the newest, not the oldest");
    }

    #[test]
    fn window_query_respects_the_lower_bound() {
        let db = temp_db("window");
        for ts in [1000, 2000, 3000, 4000] {
            db.save_message(&msg(&format!("m{}", ts), "g1", ts, false)).unwrap();
        }

        let window = db.get_messages_in_window("g1", 2500, 100).unwrap();
        let stamps: Vec<i64> = window.iter().map(|m| m.timestamp).collect();
        assert_eq!(stamps, vec![3000, 4000]);
    }

    #[test]
    fn default_rule_is_not_created_for_unknown_targets() {
        let db = temp_db("rules_empty");
        assert!(db.get_all_rules().unwrap().is_empty());
        assert!(db.get_summary_whitelist_groups().unwrap().is_empty());
    }
}
