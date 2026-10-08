use crate::models::PendingDraftDto;
use rusqlite::{params, Connection, Result};

pub fn save_draft(conn: &Connection, draft: &PendingDraftDto) -> Result<()> {
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

pub fn get_pending_drafts(conn: &Connection) -> Result<Vec<PendingDraftDto>> {
    let mut stmt = conn.prepare(
        "SELECT id, target_id, target_name, target_type, reply_to_msg_id,
                incoming_message_snippet, generated_content, thinking_content, model_used, created_at
         FROM pending_drafts ORDER BY created_at DESC"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(PendingDraftDto {
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

pub fn delete_draft(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM pending_drafts WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn update_draft_content(
    conn: &Connection,
    id: &str,
    content: &str,
    thinking: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE pending_drafts SET generated_content = ?1, thinking_content = ?2 WHERE id = ?3",
        params![content, thinking, id],
    )?;
    Ok(())
}
