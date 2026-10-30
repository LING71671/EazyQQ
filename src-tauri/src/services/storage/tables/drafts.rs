use crate::models::PendingDraftDto;
use rusqlite::{params, Connection, Result};

fn ensure_plan_column(conn: &Connection) -> Result<()> {
    let present = conn
        .prepare("PRAGMA table_info(pending_drafts)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>>>()?
        .iter()
        .any(|name| name == "reply_plan_json");
    if !present {
        // Another process may migrate the account database between the read and ALTER.
        if let Err(error) = conn.execute(
            "ALTER TABLE pending_drafts ADD COLUMN reply_plan_json TEXT",
            [],
        ) {
            if !error.to_string().contains("duplicate column") {
                return Err(error);
            }
        }
    }
    Ok(())
}

pub fn save_draft(conn: &Connection, draft: &PendingDraftDto) -> Result<()> {
    ensure_plan_column(conn)?;
    let plan_json = draft
        .reply_plan
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    conn.execute(
        "INSERT INTO pending_drafts (
            id, target_id, target_name, target_type, reply_to_msg_id,
            incoming_message_snippet, generated_content, thinking_content, model_used, created_at, reply_plan_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(id) DO UPDATE SET
            generated_content = excluded.generated_content,
            thinking_content = excluded.thinking_content,
            reply_plan_json = excluded.reply_plan_json",
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
            plan_json,
        ],
    )?;
    Ok(())
}

pub fn get_pending_drafts(conn: &Connection) -> Result<Vec<PendingDraftDto>> {
    ensure_plan_column(conn)?;
    let mut stmt = conn.prepare(
        "SELECT id, target_id, target_name, target_type, reply_to_msg_id,
                incoming_message_snippet, generated_content, thinking_content, model_used, created_at, reply_plan_json
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
            reply_plan: row
                .get::<_, Option<String>>(10)?
                .map(|value| {
                    serde_json::from_str(&value).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            10,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })
                })
                .transpose()?,
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
    ensure_plan_column(conn)?;
    conn.execute(
        "UPDATE pending_drafts SET generated_content = ?1, thinking_content = ?2, reply_plan_json = NULL WHERE id = ?3",
        params![content, thinking, id],
    )?;
    Ok(())
}
