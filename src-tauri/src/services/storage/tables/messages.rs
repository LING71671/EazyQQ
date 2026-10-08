use crate::models::MessageItemDto;
use rusqlite::{params, Connection, Result};

pub fn save_message(conn: &Connection, msg: &MessageItemDto) -> Result<()> {
    conn.execute(
        "INSERT INTO messages_log (
            msg_id, target_id, sender_id, sender_name, content, is_from_me, ai_reply_status, timestamp
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT(msg_id) DO UPDATE SET
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
         ORDER BY timestamp DESC 
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

    let mut deduped: Vec<MessageItemDto> = Vec::new();
    let mut ids_to_delete: Vec<String> = Vec::new();

    for m in list {
        if let Some(last) = deduped.last_mut() {
            let same_content = last.content == m.content;
            let same_dir = last.is_from_me == m.is_from_me;
            let close_time = (last.timestamp - m.timestamp).abs() < 60_000;
            if same_content && same_dir && close_time {
                if last.ai_reply_status == "none" && m.ai_reply_status != "none" {
                    last.ai_reply_status = m.ai_reply_status.clone();
                }
                if (last.sender_name == "好友" || last.sender_name == "我")
                    && m.sender_name != "好友"
                    && m.sender_name != "我"
                {
                    last.sender_name = m.sender_name.clone();
                }
                ids_to_delete.push(m.id);
                continue;
            }
        }
        deduped.push(m);
    }

    if !ids_to_delete.is_empty() {
        for del_id in ids_to_delete {
            let _ = conn.execute(
                "DELETE FROM messages_log WHERE msg_id = ?1",
                params![del_id],
            );
        }
    }

    Ok(deduped)
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
