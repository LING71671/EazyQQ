use crate::models::GroupSummaryDto;
use rusqlite::{params, Connection, Result};

pub fn save_summary(conn: &Connection, summary: &GroupSummaryDto) -> Result<()> {
    let key_points_json =
        serde_json::to_string(&summary.key_points).unwrap_or_else(|_| "[]".to_string());
    let decisions_json =
        serde_json::to_string(&summary.decisions).unwrap_or_else(|_| "[]".to_string());
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

pub fn get_summaries(conn: &Connection, target_id: Option<&str>) -> Result<Vec<GroupSummaryDto>> {
    let mut list = Vec::new();

    if let Some(tid) = target_id {
        let mut stmt = conn.prepare(
            "SELECT id, target_id, target_name, summary_text, key_points, decisions, start_time, end_time, created_at
             FROM group_summaries WHERE target_id = ?1 ORDER BY created_at DESC"
        )?;
        let rows = stmt.query_map(params![tid], |row| {
            let kp_raw: String = row.get(4)?;
            let dec_raw: String = row.get(5)?;
            Ok(GroupSummaryDto {
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
            Ok(GroupSummaryDto {
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

pub fn delete_summary(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM group_summaries WHERE id = ?1", params![id])?;
    Ok(())
}
