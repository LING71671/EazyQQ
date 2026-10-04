use rusqlite::{Connection, Result};

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;

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

    Ok(())
}
