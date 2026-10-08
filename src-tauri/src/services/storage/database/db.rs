use rusqlite::{Connection, Result};
use std::fs;
use std::path::Path;
use std::sync::Mutex;

use super::drafts;
use super::files;
pub use super::files::GroupFileRecord;
use super::messages;
use super::rules;
pub use super::rules::ContactRuleRecord;
use super::schema::init_schema;
use super::settings;
use crate::models::{GroupSummaryDto, MessageItemDto, PendingDraftDto};

pub struct Database {
    conn: Mutex<Connection>,
}

fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

impl Database {
    pub fn init<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        if let Some(parent) = db_path.as_ref().parent() {
            let _ = fs::create_dir_all(parent);
        }

        let conn = Connection::open(db_path)?;
        init_schema(&conn)?;

        Ok(Database {
            conn: Mutex::new(conn),
        })
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        settings::get_setting(&conn, key)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        settings::set_setting(&conn, key, value, chrono_now_ms())
    }

    pub fn get_all_rules(&self) -> Result<Vec<ContactRuleRecord>> {
        let conn = self.conn.lock().unwrap();
        rules::get_all_rules(&conn)
    }

    pub fn upsert_rule(&self, rule: &ContactRuleRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        rules::upsert_rule(&conn, rule, chrono_now_ms())
    }

    pub fn batch_update_mode(&self, target_ids: &[String], mode: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        rules::batch_update_mode(&conn, target_ids, mode, chrono_now_ms())
    }

    pub fn set_summary_whitelist(
        &self,
        target_id: &str,
        is_whitelist: bool,
        interval_hours: i32,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        rules::set_summary_whitelist(
            &conn,
            target_id,
            is_whitelist,
            interval_hours,
            chrono_now_ms(),
        )
    }

    pub fn get_summary_whitelist_groups(&self) -> Result<Vec<ContactRuleRecord>> {
        let conn = self.conn.lock().unwrap();
        rules::get_summary_whitelist_groups(&conn)
    }

    pub fn save_summary(&self, summary: &GroupSummaryDto) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        super::summaries::save_summary(&conn, summary)
    }

    pub fn get_summaries(&self, target_id: Option<&str>) -> Result<Vec<GroupSummaryDto>> {
        let conn = self.conn.lock().unwrap();
        super::summaries::get_summaries(&conn, target_id)
    }

    pub fn delete_summary(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        super::summaries::delete_summary(&conn, id)
    }

    pub fn save_message(&self, msg: &MessageItemDto) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        messages::save_message(&conn, msg)
    }

    pub fn get_messages_by_target(
        &self,
        target_id: &str,
        limit: usize,
    ) -> Result<Vec<MessageItemDto>> {
        let conn = self.conn.lock().unwrap();
        messages::get_messages_by_target(&conn, target_id, limit)
    }

    pub fn get_messages_in_window(
        &self,
        target_id: &str,
        since_ms: i64,
        limit: usize,
    ) -> Result<Vec<MessageItemDto>> {
        let conn = self.conn.lock().unwrap();
        messages::get_messages_in_window(&conn, target_id, since_ms, limit)
    }

    pub fn mark_read(&self, target_id: &str, at_ms: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        messages::mark_read(&conn, target_id, at_ms)
    }

    pub fn get_last_read(&self, target_id: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        messages::get_last_read(&conn, target_id)
    }

    pub fn count_unread(&self, target_id: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        messages::count_unread(&conn, target_id)
    }

    pub fn upsert_group_file(&self, file: &GroupFileRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        files::upsert_group_file(&conn, file)
    }

    pub fn get_group_files(&self, group_id: &str) -> Result<Vec<GroupFileRecord>> {
        let conn = self.conn.lock().unwrap();
        files::get_group_files(&conn, group_id)
    }

    pub fn find_group_file(
        &self,
        group_id: &str,
        file_id: &str,
    ) -> Result<Option<GroupFileRecord>> {
        Ok(self
            .get_group_files(group_id)?
            .into_iter()
            .find(|f| f.file_id == file_id))
    }

    pub fn delete_group_file(&self, group_id: &str, file_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        files::delete_group_file(&conn, group_id, file_id)
    }

    pub fn save_draft(&self, draft: &PendingDraftDto) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        drafts::save_draft(&conn, draft)
    }

    pub fn get_pending_drafts(&self) -> Result<Vec<PendingDraftDto>> {
        let conn = self.conn.lock().unwrap();
        drafts::get_pending_drafts(&conn)
    }

    pub fn delete_draft(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        drafts::delete_draft(&conn, id)
    }

    pub fn update_draft_content(
        &self,
        id: &str,
        content: &str,
        thinking: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        drafts::update_draft_content(&conn, id, content, thinking)
    }
}
