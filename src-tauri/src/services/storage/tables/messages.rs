use crate::models::MessageItemDto;
use rusqlite::{params, Connection, OptionalExtension, Result};

pub fn save_message(conn: &Connection, msg: &MessageItemDto) -> Result<()> {
    conn.execute(
        "INSERT INTO messages_log (
            msg_id, target_id, sender_id, sender_name, content, is_from_me, ai_reply_status, timestamp
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(msg_id) DO UPDATE SET
            sender_id = excluded.sender_id,
            sender_name = excluded.sender_name,
            is_from_me = excluded.is_from_me,
            timestamp = excluded.timestamp,
            content = excluded.content,
            ai_reply_status = CASE 
                WHEN messages_log.ai_reply_status != 'none' THEN messages_log.ai_reply_status 
                ELSE excluded.ai_reply_status 
            END",
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

pub fn get_messages_by_target(
    conn: &Connection,
    target_id: &str,
    limit: usize,
) -> Result<Vec<MessageItemDto>> {
    let mut stmt = conn.prepare(
        "SELECT msg_id, target_id, sender_id, sender_name, content, is_from_me, ai_reply_status, timestamp
         FROM messages_log 
         WHERE target_id = ?1 
         ORDER BY timestamp DESC, rowid DESC
         LIMIT ?2"
    )?;

    let rows = stmt.query_map(params![target_id, limit as i64], |row| {
        Ok(MessageItemDto {
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

pub fn save_message_source(conn: &Connection, msg_id: &str, source: &serde_json::Value) -> Result<()> {
    conn.execute(
        "INSERT INTO message_sources (msg_id, source_json) VALUES (?1, ?2)
         ON CONFLICT(msg_id) DO UPDATE SET source_json = excluded.source_json",
        params![msg_id, source.to_string()],
    )?;
    Ok(())
}

pub fn get_message_source(conn: &Connection, msg_id: &str) -> Result<Option<serde_json::Value>> {
    let source: Option<String> = conn.query_row(
        "SELECT source_json FROM message_sources WHERE msg_id = ?1",
        [msg_id],
        |row| row.get(0),
    ).optional()?;
    source.map(|source| serde_json::from_str(&source).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })).transpose()
}

pub fn get_messages_in_window(
    conn: &Connection,
    target_id: &str,
    since_ms: i64,
    limit: usize,
) -> Result<Vec<MessageItemDto>> {
    let mut stmt = conn.prepare(
        "SELECT msg_id, target_id, sender_id, sender_name, content, is_from_me, ai_reply_status, timestamp
         FROM messages_log
         WHERE target_id = ?1 AND timestamp >= ?2
         ORDER BY timestamp ASC
         LIMIT ?3"
    )?;

    let rows = stmt.query_map(params![target_id, since_ms, limit as i64], |row| {
        Ok(MessageItemDto {
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

pub fn mark_read(conn: &Connection, target_id: &str, at_ms: i64) -> Result<()> {
    conn.execute(
        "INSERT INTO read_state (target_id, last_read_at) VALUES (?1, ?2)
         ON CONFLICT(target_id) DO UPDATE SET
            last_read_at = MAX(read_state.last_read_at, excluded.last_read_at)",
        params![target_id, at_ms],
    )?;
    Ok(())
}

pub fn get_last_read(conn: &Connection, target_id: &str) -> Result<i64> {
    let mut stmt = conn.prepare("SELECT last_read_at FROM read_state WHERE target_id = ?1")?;
    let mut rows = stmt.query(params![target_id])?;
    if let Some(row) = rows.next()? {
        Ok(row.get(0)?)
    } else {
        Ok(0)
    }
}

pub fn count_unread(conn: &Connection, target_id: &str) -> Result<i64> {
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
