use crate::models::ApiResponse;
use tauri::command;

#[command]
#[tracing::instrument(name = "eazyqq.ipc.telemetry_status", skip_all)]
pub async fn get_telemetry_status() -> Result<ApiResponse<serde_json::Value>, String> {
    Ok(ApiResponse::ok(crate::services::infra::telemetry::status()))
}

#[command]
pub async fn test_telemetry() -> Result<ApiResponse<serde_json::Value>, String> {
    let result = tauri::async_runtime::spawn_blocking(crate::services::infra::telemetry::test_event)
        .await.map_err(|error| error.to_string())??;
    Ok(ApiResponse::ok(result))
}
