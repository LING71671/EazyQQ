use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

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

pub fn upsert_group_file(conn: &Connection, file: &GroupFileRecord) -> Result<()> {
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

pub fn get_group_files(conn: &Connection, group_id: &str) -> Result<Vec<GroupFileRecord>> {
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

pub fn delete_group_file(conn: &Connection, group_id: &str, file_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM group_files WHERE group_id = ?1 AND file_id = ?2",
        params![group_id, file_id],
    )?;
    Ok(())
}
