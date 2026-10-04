use tauri::{command, State};
use crate::commands::AppState;
use crate::models::{ApiResponse, FileSummaryResultDto, GroupFileItemDto};

#[command]
pub async fn get_group_files(
    state: State<'_, AppState>,
    group_id: String,
    _folder_id: Option<String>,
) -> Result<ApiResponse<Vec<GroupFileItemDto>>, String> {
    // Real implementation: mirror the remote list, then report local download state.
    if let Err(e) = crate::services::group_files::sync_group_files(&state.db, &state.onebot, &group_id).await {
        tracing::warn!("group file sync failed for {}: {}", group_id, e);
    }

    let records = state.db.get_group_files(&group_id).map_err(|e| e.to_string())?;
    let list: Vec<GroupFileItemDto> = records
        .into_iter()
        .map(|r| GroupFileItemDto {
            file_id: r.file_id,
            file_name: r.file_name,
            file_size: r.file_size,
            file_url: None,
            uploader_id: String::new(),
            uploader_name: r.uploader_name,
            upload_time: r.upload_time,
            download_status: r.download_status,
            local_path: r.local_path,
        })
        .collect();

    Ok(ApiResponse::ok(list))
}

#[command]
pub async fn download_file(
    state: State<'_, AppState>,
    group_id: String,
    file_id: String,
    file_name: String,
) -> Result<ApiResponse<serde_json::Value>, String> {
    tracing::info!("download requested: {} (group {}, file {})", file_name, group_id, file_id);

    let path = crate::services::group_files::download_group_file(
        &state.db,
        &state.onebot,
        &group_id,
        &file_id,
    )
    .await?;

    Ok(ApiResponse::ok(serde_json::json!({
        "taskId": format!("dl_{}", file_id),
        "localSavePath": path.display().to_string(),
    })))
}

#[command]
pub async fn summarize_file(
    state: State<'_, AppState>,
    local_file_path: String,
) -> Result<ApiResponse<FileSummaryResultDto>, String> {
    let result = crate::services::group_files::summarize_file(&state.ai, &local_file_path).await?;
    Ok(ApiResponse::ok(result))
}

#[command]
pub async fn open_folder(target_path: String) -> Result<ApiResponse<()>, String> {
    let base_data = crate::services::logging::data_dir();
    let path = if target_path.is_empty() || target_path == "." {
        base_data.join("group_files")
    } else {
        let p = std::path::Path::new(&target_path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            let rel = target_path
                .replace('/', "\\")
                .trim_start_matches("EazyQQ_Data\\")
                .trim_start_matches("EazyQQ_Data/")
                .to_string();
            base_data.join(rel)
        }
    };

    let _ = std::fs::create_dir_all(&path);

    #[cfg(target_os = "windows")]
    {
        let canonical_str = path.to_string_lossy().to_string();
        tracing::info!("open_folder: launching explorer for {}", canonical_str);
        let _ = std::process::Command::new("explorer")
            .arg(&canonical_str)
            .spawn();
    }
    Ok(ApiResponse::ok(()))
}
